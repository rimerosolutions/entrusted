extern crate base64_lib;

use actix_web::http::header::HeaderValue;
use once_cell::sync::Lazy;

use actix_cors::Cors;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use std::collections::HashMap;
use std::env;
use std::fs;

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Context, Poll};
use std::time::Duration;

use actix_web::web::{Bytes, Data};
use actix_web::Error;
use futures::{Stream, StreamExt};
use tokio::sync::mpsc::{channel, Sender, Receiver};
use tokio::time::{interval_at, Instant};

use actix_web::{http::header, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use std::sync::Arc;
use uuid::Uuid;

use actix_multipart::Multipart;
use actix_web::http::{Method, Uri};
use futures::TryStreamExt;
use http_api_problem::{HttpApiProblem, StatusCode};
use tokio::io::AsyncWriteExt;

use bs58;

use entrusted_l10n as l10n;

use crate::config;
use crate::model;
use crate::uil10n;
use crate::process;

const SPA_INDEX_HTML: &[u8] = include_bytes!("../web-assets/index.html");

static NOTIFICATIONS_PER_REFID: Lazy<Mutex<HashMap<String, Arc<Mutex<Vec<model::Notification>>>>>> = Lazy::new(|| {
    Mutex::new(HashMap::<String, Arc<Mutex<Vec<model::Notification>>>>::new())
});

struct Broadcaster {
    clients: Vec<Sender<Bytes>>,
}

impl Broadcaster {
    fn create() -> Data<Mutex<Self>> {
        // Data ≃ Arc
        let me = Data::new(Mutex::new(Broadcaster::new()));

        // ping clients every 10 seconds to see if they are alive
        Broadcaster::spawn_ping(me.clone(), 10);

        me
    }

    fn new() -> Self {
        Broadcaster {
            clients: Vec::new(),
        }
    }

    fn spawn_ping(me: Data<Mutex<Self>>, interval_secs: u64) {
        actix_web::rt::spawn(async move {
            let mut interval = interval_at(Instant::now(), Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;
                me.lock().unwrap().remove_stale_clients();
            }
        });
    }

    fn remove_stale_clients(&mut self) {
        let msg = Bytes::from("data: ping\n\n".to_string());
        let mut ok_clients = Vec::with_capacity(self.clients.len());

        for client in self.clients.iter() {
            let result = client.try_send(msg.clone());

            if let Ok(_) = result {
                ok_clients.push(client.clone());
            }
        }

        self.clients = ok_clients;
    }

    fn new_client(&mut self, refid: String) -> Client {
        let (tx, _rx) = channel(100);
        tx.try_send(Bytes::from("data: connected\n\n")).unwrap();
        self.clients.push(tx);
        let done = false;
        let idx = 0;

        Client {
            _rx,
            refid,
            idx,
            done
        }
    }
}

// wrap Receiver in own type, with correct error type
struct Client {
    _rx: Receiver<Bytes>,
    refid: String,
    idx: usize,
    done: bool
}

impl Stream for Client {
    type Item = Result<Bytes, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.done {
            return Poll::Ready(None);
        }

        let notifications_per_refid = NOTIFICATIONS_PER_REFID.lock().unwrap().clone();

        if let Some(notifications_group) = notifications_per_refid.get(&self.refid) {
            let notifications = notifications_group.lock().unwrap();
            let notifications_count = notifications.len();
            let i = self.idx;

            if i < notifications_count {
                let notifs = notifications.as_slice();
                let n = &notifs[i];

                if n.event == "processing_failure" || n.event == "processing_success" {
                    self.done = true;
                }

                let ntext = format!("event: {}\nid: {}\ndata: {}\n\n", n.event, n.id, n.data);
                self.idx = i + 1;

                return Poll::Ready(Some(Ok(Bytes::from(ntext))));
            }
        }

        cx.waker().wake_by_ref();

        Poll::Pending
    }
}

async fn notfound(l10n: Data<Mutex<l10n::Translations>>) -> impl Responder {
    HttpResponse::NotFound().body(l10n.lock().unwrap().gettext("Resource not found"))
}

fn request_problem(reason: String, uri: &Uri) -> HttpApiProblem {
    HttpApiProblem::with_title_and_type_from_status(StatusCode::BAD_REQUEST)
        .set_detail(reason)
        .set_instance(uri.to_string())
}

fn server_problem(reason: String, uri: &Uri) -> HttpApiProblem {
    HttpApiProblem::with_title_and_type_from_status(StatusCode::INTERNAL_SERVER_ERROR)
        .set_detail(reason)
        .set_instance(uri.to_string())
}

fn parse_accept_language(req_language: &HeaderValue, fallback_lang: String) -> String {
    if let Ok(req_language_str) = req_language.to_str() {
        let language_list = req_language_str.split(",").collect::<Vec<&str>>();

        if !language_list.is_empty() {
            let first_language = language_list[0].split(";").collect::<Vec<&str>>();
            String::from(first_language[0])
        } else {
            fallback_lang
        }
    } else {
        fallback_lang
    }
}

pub async fn serve(host: &str, port: &str, ci_image_name: String, l10n: l10n::Translations) -> std::io::Result<()> {
    let addr = format!("{}:{}", host, port);
    println!("{}: {}", l10n.gettext("Starting server at address"), &addr);

    let img = ci_image_name.clone();
    let ci_image_data = Data::new(Mutex::new(img));
    let l10n_data = Data::new(Mutex::new(l10n));

    HttpServer::new(move|| {
        let cors = Cors::permissive()
            .supports_credentials()
            .allowed_methods(vec![Method::GET, Method::POST, Method::OPTIONS, Method::HEAD, ])
            .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT, header::CONTENT_TYPE, ]);

        let data = Broadcaster::create();

        App::new()
            .wrap(cors)
            .app_data(l10n_data.clone())
            .app_data(data.clone())
            .app_data(ci_image_data.clone())
            .service(web::resource("/").route(web::get().to(index)))
            .route("/upload", web::post().to(upload))
            .route("/events/{id}", web::get().to(events))
            .route("/uitranslations", web::get().to(uitranslations))
            .route("/downloads/{id}", web::get().to(downloads))
            .default_service(web::get().to(notfound))
    })
        .bind(addr)?
        .run()
        .await
}

async fn uitranslations(req: HttpRequest) -> impl Responder {
    let langid = if let Some(req_language) = req.headers().get(header::ACCEPT_LANGUAGE) {
        parse_accept_language(req_language, l10n::DEFAULT_LANGID.to_string())
    } else {
        String::from(l10n::DEFAULT_LANGID)
    };

    let json_data = uil10n::ui_translation_for(langid);

    HttpResponse::Ok()
        .append_header((header::CONTENT_TYPE, "text/json"))
        .body(json_data)
}

async fn index() -> impl Responder {
    HttpResponse::Ok()
        .append_header((header::CONTENT_TYPE, "text/html"))
        .body(SPA_INDEX_HTML)
}

async fn downloads(info: actix_web::web::Path<String>, req: HttpRequest, l10n: Data<Mutex<l10n::Translations>>) -> impl Responder {
    let l10n_ref = l10n.lock().unwrap();
    println!("{}: {}", l10n_ref.gettext("Download from"), req.uri());

    let request_id = info.into_inner();
    let mut fileid = String::new();
    let mut filename = String::new();
    let mut request_err_found = true;
    let request_id_bytes_ret = bs58::decode(request_id.clone()).into_vec();

    match request_id_bytes_ret {
        Ok(request_id_inner_bytes) =>  {
            if let Ok(request_id_inner) = std::str::from_utf8(&request_id_inner_bytes) {
                let file_data_parts = request_id_inner.split(";").map(|i| i.to_string()).collect::<Vec<String>>();

                if file_data_parts.len() == 2 {
                    let fileid_data   = base64_lib::decode(&file_data_parts[0]);
                    let filename_data = base64_lib::decode(&file_data_parts[1]);

                    if let (Ok(fileid_value),
                            Ok(filename_value)) = (std::str::from_utf8(&fileid_data), std::str::from_utf8(&filename_data)) {
                        fileid.push_str(&output_filename_for(fileid_value.to_string()));
                        filename.push_str(&output_filename_for(filename_value.to_string()));
                        request_err_found = false;
                    } else {
                        eprintln!("An error was found: {:?}, {:?}", fileid_data, filename_data);
                    }
                }
            }
        },
        Err(ex) => {
            eprintln!("An error occurred: {:?}", ex);
        }
    }

    if request_err_found {
        return HttpResponse::BadRequest().json(request_problem(
            l10n_ref.gettext("Invalid request identifier. Is the file name atrociously long?"),
            req.uri(),
        ));
    }

    let file_loc = env::temp_dir().join(config::PROGRAM_GROUP).join(fileid.clone());

    if !file_loc.exists() {
        HttpResponse::NotFound().body(l10n_ref.gettext("Resource not found"))
    } else {
        match fs::read(file_loc.clone()) {
            Ok(data) => {
                let _ = fs::remove_file(file_loc);
                NOTIFICATIONS_PER_REFID.lock().unwrap().remove(&request_id.to_string());
                HttpResponse::Ok()
                    .append_header((header::CONTENT_TYPE, "application/pdf"))
                    .append_header((header::CONTENT_DISPOSITION, format!("attachment; filename={}", &filename)))
                    .append_header((header::CONTENT_LENGTH, data.len().to_string()))
                    .body(data)
            }
            Err(ex) => {
                eprintln!("{} {}.", l10n_ref.gettext("Could not read input file"), ex.to_string());

                if let Err(ioe) = fs::remove_file(&file_loc) {
                    eprintln!("{} {}. {}.", l10n_ref.gettext("Could not delete file"), &file_loc.display(), ioe.to_string());
                }

                NOTIFICATIONS_PER_REFID.lock().unwrap().remove(&request_id.to_string());
                HttpResponse::InternalServerError().body(l10n_ref.gettext("Internal error"))
            }
        }
    }
}

async fn events(info: actix_web::web::Path<String>, broadcaster: Data<Mutex<Broadcaster>>, l10n: Data<Mutex<l10n::Translations>>) -> impl Responder {
    let ref_id = info.into_inner();
    let notifications_per_refid = NOTIFICATIONS_PER_REFID.lock().unwrap();

    if !notifications_per_refid.contains_key(&ref_id.clone()) {
        return HttpResponse::NotFound().body(l10n.lock().unwrap().gettext("Resource not found"));
    }

    let rx = broadcaster.lock().unwrap().new_client(ref_id);

    HttpResponse::Ok()
        .append_header((header::CONTENT_TYPE, "text/event-stream"))
        .streaming(rx)
}

fn output_filename_for(request_id: String) -> String {
    let basename = std::path::Path::new(&request_id).with_extension("").display().to_string();
    [basename, "-".to_string(), config::DEFAULT_FILE_SUFFIX.to_string(), ".pdf".to_string()].concat()
}

async fn upload(req: HttpRequest, payload: Multipart, ci_image_name: Data<Mutex<String>>, l10n: Data<Mutex<l10n::Translations>>) -> impl Responder {
    let l10n_ref = l10n.lock().unwrap();

    let langid = if let Some(req_language) = req.headers().get(header::ACCEPT_LANGUAGE) {
        parse_accept_language(req_language, l10n_ref.langid())
    } else {
        l10n_ref.langid()
    };

    let tmpdir = env::temp_dir().join(config::PROGRAM_GROUP);
    let uploaded_file_ret = save_file(payload, tmpdir.clone(), l10n_ref.clone()).await;
    let mut err_msg = String::new();
    let mut uploaded_file = model::UploadedFile::default();

    match uploaded_file_ret {
        Ok(uploaded_file_value) => {
            uploaded_file = uploaded_file_value;
        }
        Err(ex) => {
            err_msg.push_str(&ex.to_string());
        }
    }

    if err_msg.is_empty() {
        NOTIFICATIONS_PER_REFID.lock().unwrap().insert(uploaded_file.id.clone(), Arc::new(Mutex::new(Vec::<model::Notification>::new())));
        let new_upload_info = uploaded_file.clone();
        let l10n_async_ref = l10n_ref.clone();
        let langid_ref = langid.clone();

        actix_web::rt::spawn(async move {
            let opt_passwd = if new_upload_info.docpassword.is_empty() {
                None
            } else {
                Some(new_upload_info.docpassword.clone())
            };

            let ocr_lang_opt = if new_upload_info.ocrlang.is_empty() {
                None
            } else {
                Some(new_upload_info.ocrlang.clone())
            };

            let request_id = new_upload_info.id.clone();
            let input_path = PathBuf::from(&new_upload_info.location);
            let output_path = PathBuf::from(tmpdir).join(output_filename_for(new_upload_info.location.clone()));
            let container_image_name = ci_image_name.lock().unwrap().to_string();
            let conversion_options = model::ConversionOptions::new(container_image_name, ocr_lang_opt, opt_passwd);

            if let Err(ex) = run_entrusted(request_id, input_path, output_path, conversion_options, l10n_async_ref.clone(), langid_ref).await {
                eprintln!("{}. {}", l10n_async_ref.gettext("Processing failure"), ex.to_string());
            }
        });
    };

    if err_msg.is_empty() {
        HttpResponse::Accepted().json(model::UploadResponse::new(uploaded_file.id.clone(), format!("/events/{}", uploaded_file.id.clone())))
    } else {
        HttpResponse::InternalServerError().json(server_problem(
            err_msg,
            req.uri(),
        ))
    }
}

pub async fn save_file(
    mut payload: Multipart,
    tmpdir: PathBuf,
    l10n: l10n::Translations
) -> Result<model::UploadedFile, Box<dyn std::error::Error>> {
    let mut buf         = Vec::<u8>::new();
    let mut filename    = String::new();
    let mut fileext     = String::new();
    let mut ocrlang     = String::new();
    let mut docpassword = String::new();

    while let Ok(Some(mut field)) = payload.try_next().await {
        let fname = field.name();
        if fname == "file" {
            // Field in turn is stream of *Bytes* object
            while let Some(chunk) = field.next().await {
                buf.extend(&chunk?.to_vec());
            }
        } else if fname == "filename" {
            while let Some(chunk) = field.next().await {
                filename.push_str(std::str::from_utf8(&chunk?.to_vec())?);
            }
        } else if fname == "ocrlang" {
            while let Some(chunk) = field.next().await {
                ocrlang.push_str(std::str::from_utf8(&chunk?.to_vec())?);
            }
        } else if fname == "docpasswd" {
            while let Some(chunk) = field.next().await {
                docpassword.push_str(std::str::from_utf8(&chunk?.to_vec())?);
            }
        }
    }

    if filename.is_empty() {
        return Err(l10n.gettext("Missing 'filename' in form data").into());
    }

    if buf.is_empty() {
        return Err(l10n.gettext("Missing 'file' in form data").into());
    }

    let file_uuid = Uuid::new_v4().to_string();
    let id_token = format!("{};{}",
                           base64_lib::encode(&file_uuid.clone().into_bytes()),
                           base64_lib::encode(&filename.clone().into_bytes()));
    let id = bs58::encode(id_token).into_string();

    let p = std::path::Path::new(&filename);

    if let Some(fext) = p.extension().map(|i| i.to_str()).and_then(|i| i) {
        fileext.push_str(fext);
    } else {
        return Err(format!("{}: {}", l10n.gettext("Mime type error! Does the input have a 'known' file extension?"), filename).into());
    }

    if !tmpdir.exists() {
        fs::create_dir(&tmpdir)?;
    }

    let filepath = tmpdir.join(format!("{}.{}", &file_uuid, fileext));
    let mut f = tokio::fs::File::create(&filepath).await?;
    f.write_all(&buf).await?;

    let location = filepath.display().to_string();

    Ok(model::UploadedFile {
        id, docpassword, location, ocrlang, fileext
    })
}

fn progress_made(refid: String, event: &str, data: String, counter: i32, err_find_notif: String, err_notif_handle: String) -> Result<(), Box<dyn std::error::Error>> {
    let nevent = event.to_string();
    let nid = format!("{}", counter);
    let n = model::Notification::new(nevent, nid, data);

    if let Ok(bc) = NOTIFICATIONS_PER_REFID.lock() {
        if let Some(v) = bc.get(&refid) {
            v.lock().unwrap().push(n);
            Ok(())
        } else {
            Err(format!("{} : {}.", err_find_notif, refid).into())
        }
    } else {
        Err(err_notif_handle.into())
    }
}

async fn run_entrusted(
    refid: String,
    input_path: PathBuf,
    output_path: PathBuf,
    conversion_options: model::ConversionOptions,
    l10n: l10n::Translations,
    langid: String
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(proposed_ocr_lang) = conversion_options.opt_ocr_lang.clone() {
        let ocr_lang_by_code = l10n::ocr_lang_key_by_name(&l10n);

        if !ocr_lang_by_code.contains_key(&*proposed_ocr_lang) {
            return Err(l10n.gettext_fmt("Unknown language code for the ocr-lang parameter: {0}. Hint: Try 'eng' for English.", vec![&proposed_ocr_lang]).into());
        }
    }

    let cmd = if let Some(app_path) = process::exe_find("entrusted-cli") {
        app_path.display().to_string()
    } else {
        return Err(l10n.gettext("Please ensure that the entrusted-cli binary is available in your PATH environment variable.").into());
    };

    let mut cmd_args = vec![
        "--log-format".to_string(),
        "json".to_string(),
        "--container-image-name".to_string(),
        conversion_options.ci_image_name,
        "--output-filename".to_string(),
        output_path.display().to_string(),
        "--input-filename".to_string(),
        input_path.display().to_string()
    ];

    if let Some(ocr_lang) = conversion_options.opt_ocr_lang {
        cmd_args.push("--ocr-lang".to_string());
        cmd_args.push(ocr_lang);
    }

    if conversion_options.opt_passwd.is_some() {
        cmd_args.push("--passwd-prompt".to_string());
    }

    println!("{}: {}", l10n.gettext("Running command"), cmd_args.join(" "));

    let err_find_notif = l10n.gettext("Could not find notification for");
    let err_notif_handle = l10n.gettext("Could not read notifications data");

    let mut env_map: HashMap<String, String> = HashMap::new();
    env_map.insert(l10n::ENV_VAR_ENTRUSTED_LANGID.to_string(), langid.clone());

    if let Some(doc_passwd) = conversion_options.opt_passwd {
        env_map.insert("ENTRUSTED_AUTOMATED_PASSWORD_ENTRY".to_string(), doc_passwd);
    }

    let mut child = process::spawn_cmd(cmd, cmd_args, env_map)?;
    let mut counter = 1;

    let stdout = child.stdout.take().expect("child is missing stdout handle");
    let stderr = child.stderr.take().expect("child is missing stderr handle");
    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let mut success = false;

    loop {
        tokio::select! {
            result = stdout_reader.next_line() => {
                match result {
                    Ok(Some(line)) => {
                        progress_made(refid.clone(), "processing_update", line, counter, err_find_notif.clone(), err_notif_handle.clone())?;
                        counter += 1;
                    }, Err(_) => break,
                    _ => (),
                }
            }
            result = stderr_reader.next_line() => {
                match result {
                    Ok(Some(line)) => {
                        progress_made(refid.clone(), "processing_update", line, counter, err_find_notif.clone(), err_notif_handle.clone())?;
                        counter += 1;
                    }, Err(_) => break,
                    _ => (),
                }
            }
            result = child.wait() => {
                match result {
                    Ok(exit_code) => {
                        success = exit_code.success();
                    },
                    _ => (),
                }
                break // child process exited
            }
        };

    };

    if success {
        let msg = model::CompletionMessage::new(format!("/downloads/{}", refid.clone()));
        let msg_json = serde_json::to_string(&msg).unwrap();
        progress_made(refid.clone(), "processing_success", msg_json, counter, err_find_notif, err_notif_handle)?;
        let _ = fs::remove_file(input_path);

        Ok(())
    } else {
        let msg = model::CompletionMessage::new("failure".to_string());
        let msg_json = serde_json::to_string(&msg).unwrap();
        progress_made(refid.clone(), "processing_failure", msg_json, counter, err_find_notif, err_notif_handle)?;
        let _ = fs::remove_file(input_path);

        Err(l10n.gettext("Processing failure").into())
    }
}
