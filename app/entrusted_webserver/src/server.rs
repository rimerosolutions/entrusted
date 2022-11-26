extern crate base64_lib;

use std::net::ToSocketAddrs;
use std::{convert::Infallible, time::Duration};

use std::error::Error;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;

use axum::extract::{Multipart, Path};
use axum::http::{header, HeaderMap, HeaderValue, Uri};
use axum::Extension;
use axum::{
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use once_cell::sync::Lazy;
use tower_http::cors::CorsLayer;

use std::collections::HashMap;
use std::env;
use std::fs;

use std::io::Write;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Context, Poll};

use futures::{self, Stream};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::time::{interval_at, Instant};

use serde_json;

use std::sync::Arc;
use uuid::Uuid;

use http_api_problem;
use percent_encoding::{percent_encode, NON_ALPHANUMERIC};
use bs58;

use crate::process;
use entrusted_l10n as l10n;

use crate::config;
use crate::model;
use crate::uil10n;

const SPA_INDEX_HTML: &[u8] = include_bytes!("../web-assets/index.html");

static NOTIFICATIONS_PER_REFID: Lazy<Mutex<HashMap<String, Arc<Mutex<Vec<model::Notification>>>>>> =
    Lazy::new(|| Mutex::new(HashMap::<String, Arc<Mutex<Vec<model::Notification>>>>::new()));

pub async fn serve(
    host: &str,
    port: &str,
    ci_image_name: String,
    trans: l10n::Translations,
) -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt().with_ansi(false).init();

    let state_trans = Arc::new(trans.clone());
    let state_bc = Broadcaster::create();
    let state_ci_image = Arc::new(ci_image_name.clone());

    let addr = format!("{}:{}", host, port);
    tracing::info!("{}: {}", trans.gettext("Starting server at address"), &addr);

    let app = Router::new()
        .route("/", get(index))
        .route("/api/v1/uitranslations", get(uitranslations))
        .route("/api/v1/events/:request_id", get(events))
        .route("/api/v1/downloads/:request_id", get(downloads))
        .route("/api/v1/upload", post(upload))
        .fallback(notfound)
        .layer(CorsLayer::permissive())
        .layer(Extension(state_ci_image))
        .layer(Extension(state_bc))
        .layer(Extension(state_trans));

    let mut addrs_iter = addr.to_socket_addrs()?.filter(|s| s.is_ipv4());

    match addrs_iter.next() {
        Some(socket_addr) => {
            tracing::info!("{}: {}", trans.gettext("Using address"), &socket_addr);

            match axum::Server::bind(&socket_addr).serve(app.into_make_service()).await {
                Ok(_)   => Ok(()),
                Err(ex) => Err(ex.into()),
            }
        }
        None => {
            return Err(trans.gettext("Cannot resolve server address").into());
        }
    }
}

async fn index<'a>() -> Html<&'a [u8]> {
    Html(SPA_INDEX_HTML)
}

async fn notfound(trans: Extension<Arc<l10n::Translations>>) -> impl IntoResponse {
    (StatusCode::NOT_FOUND, trans.gettext("Resource not found"))
}

async fn uitranslations(headers: HeaderMap, uri: Uri) -> Result<Json<model::TranslationResponse>, AppError> {
    let langid = if let Some(req_language) = headers.get(header::ACCEPT_LANGUAGE) {
        parse_accept_language(req_language, l10n::DEFAULT_LANGID.to_string())
    } else {
        l10n::DEFAULT_LANGID.to_string()
    };

    let json_data = uil10n::ui_translation_for(langid);

    let translation_response_ret: serde_json::Result<model::TranslationResponse> = serde_json::from_slice(&json_data);

    match translation_response_ret {
        Ok(translation_response) => {
            Ok(Json(translation_response))
        },
        Err(ex) => {
            Err(AppError::InternalServerError(problem_internal_server_error(ex.to_string(), &uri)).into())
        }
    }
}

async fn upload(
    headers: HeaderMap,
    uri: Uri,
    ci_image_name: Extension<Arc<String>>,
    trans_ref: Extension<Arc<l10n::Translations>>,
    payload: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let trans = &*(trans_ref.0.clone());
    let langid = if let Some(req_language) = headers.get(header::ACCEPT_LANGUAGE) {
        parse_accept_language(req_language, trans.langid())
    } else {
        trans.langid()
    };

    let tmpdir = env::temp_dir().join(config::PROGRAM_GROUP);
    let uploaded_file_ret = save_file(payload, tmpdir.clone(), trans.clone()).await;
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
        NOTIFICATIONS_PER_REFID.lock().unwrap().insert(
            uploaded_file.id.clone(),
            Arc::new(Mutex::new(Vec::<model::Notification>::new())),
        );

        let new_upload_info = uploaded_file.clone();
        let l10n_async_ref = trans.clone();
        let langid_ref = langid.clone();

        tokio::spawn(async move {
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
            let output_path =
                PathBuf::from(tmpdir).join(output_filename_for(new_upload_info.location.clone()));
            let container_image_name = ci_image_name.to_string();
            let conversion_options =
                model::ConversionOptions::new(container_image_name, ocr_lang_opt, opt_passwd);

            if let Err(ex) = run_entrusted(
                request_id,
                input_path,
                output_path,
                conversion_options,
                l10n_async_ref.clone(),
                langid_ref,
            ).await {
                tracing::warn!(
                    "{}. {}",
                    l10n_async_ref.gettext("Processing failure"),
                    ex.to_string()
                );
            }
        });
    };

    if err_msg.is_empty() {
        Ok((
            StatusCode::ACCEPTED,
            Json(model::UploadResponse::new(
                uploaded_file.id.clone(),
                format!("/api/v1/events/{}", uploaded_file.id.clone()),
            )),
        ))
    } else {

        Err(AppError::InternalServerError(problem_internal_server_error(err_msg, &uri)).into())
    }
}

async fn events(
    Path(request_id): Path<String>,
    uri: Uri,
    broadcaster: Extension<Arc<Mutex<Broadcaster>>>,
    l10n: Extension<Arc<l10n::Translations>>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    let ref_id = request_id.to_owned();

    if let Ok(notifications_per_refid) = NOTIFICATIONS_PER_REFID.lock() {
        if !notifications_per_refid.contains_key(&ref_id) {
            return Err(AppError::NotFound(problem_not_found(l10n.gettext("Resource not found"), &uri)).into());
        }
    }

    let stream = broadcaster.lock().unwrap().new_client(ref_id);

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn downloads(
    Path(request_id): Path<String>,
    uri: Uri,
    l10n_ref: Extension<Arc<l10n::Translations>>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!("{}: {}", l10n_ref.gettext("Download from"), uri.path());

    let mut fileid = String::new();
    let mut filename = String::new();
    let mut request_err_found = true;
    let request_id_bytes_ret = bs58::decode(request_id.clone()).into_vec();

    match request_id_bytes_ret {
        Ok(request_id_inner_bytes) => {
            if let Ok(request_id_inner) = std::str::from_utf8(&request_id_inner_bytes) {
                let file_data_parts = request_id_inner
                    .split(";")
                    .map(|i| i.to_string())
                    .collect::<Vec<String>>();

                if file_data_parts.len() == 2 {
                    let fileid_data = base64_lib::decode(&file_data_parts[0]);
                    let filename_data = base64_lib::decode(&file_data_parts[1]);

                    if let (Ok(fileid_value), Ok(filename_value)) = (
                        std::str::from_utf8(&fileid_data),
                        std::str::from_utf8(&filename_data),
                    ) {
                        fileid.push_str(&output_filename_for(fileid_value.to_string()));
                        filename.push_str(&output_filename_for(filename_value.to_string()));
                        request_err_found = false;
                    } else {
                        tracing::warn!(
                            "{}: {}",
                            l10n_ref.gettext("Could not decode request"),
                            request_id
                        );
                    }
                }
            }
        }
        Err(ex) => {
            tracing::warn!("{}:{}", l10n_ref.gettext("Internal error"), ex.to_string());
        }
    }

    if request_err_found {
        return Err(AppError::BadRequest(
            problem_bad_request(l10n_ref.gettext("Invalid request identifier or perhaps the file name atrociously long"), &uri))
                   .into());
    }

    let file_loc = env::temp_dir()
        .join(config::PROGRAM_GROUP)
        .join(fileid.clone());

    if !file_loc.exists() {
        return Err(AppError::NotFound(problem_not_found(l10n_ref.gettext("Resource not found"), &uri).into()));
    } else {
        match fs::read(file_loc.clone()) {
            Ok(data) => {
                let _ = fs::remove_file(file_loc);

                if let Ok(mut notifs_by_ref_id) = NOTIFICATIONS_PER_REFID.lock() {
                    notifs_by_ref_id.remove(&request_id.to_string());
                }

                let mut headers = HeaderMap::with_capacity(3);

                if let Ok(header_value) = HeaderValue::from_str("application/pdf") {
                    headers.insert(header::CONTENT_TYPE, header_value);
                }

                if let Ok(header_value) =
                    HeaderValue::from_str(&format!("attachment; filename*=UTF-8''{}", percent_encode(filename.as_bytes(), NON_ALPHANUMERIC).to_string()))
                {
                    headers.insert(header::CONTENT_DISPOSITION, header_value);
                }

                if let Ok(header_value) = HeaderValue::from_str(&format!("{}", data.len())) {
                    headers.insert(header::CONTENT_LENGTH, header_value);
                }

                Ok((StatusCode::OK, headers, data))
            }
            Err(ex) => {
                tracing::warn!(
                    "{} {}.",
                    l10n_ref.gettext("Could not read input file"),
                    ex.to_string()
                );

                if let Err(ioe) = fs::remove_file(&file_loc) {
                    tracing::warn!(
                        "{} {}. {}.",
                        l10n_ref.gettext("Could not delete file"),
                        &file_loc.display(),
                        ioe.to_string()
                    );
                }

                if let Ok(mut notifs_per_refid) = NOTIFICATIONS_PER_REFID.lock() {
                    notifs_per_refid.remove(&request_id.to_string());
                }

                return Err(
                    AppError::InternalServerError(problem_internal_server_error(l10n_ref.gettext("Internal error"), &uri)).into(),
                );
            }
        }
    }
}

fn output_filename_for(request_id: String) -> String {
    let basename = std::path::Path::new(&request_id)
        .with_extension("")
        .display()
        .to_string();

    [
        basename,
        "-".to_string(),
        config::DEFAULT_FILE_SUFFIX.to_string(),
        ".pdf".to_string(),
    ]
        .concat()
}

fn problem_not_found(reason: String, uri: &Uri) -> http_api_problem::HttpApiProblem {
    http_api_problem::HttpApiProblem::with_title_and_type(
        http_api_problem::StatusCode::NOT_FOUND,
    )
        .detail(reason)
        .instance(uri.to_string())
}

fn problem_bad_request(reason: String, uri: &Uri) -> http_api_problem::HttpApiProblem {
    http_api_problem::HttpApiProblem::with_title_and_type(
        http_api_problem::StatusCode::BAD_REQUEST,
    )
        .detail(reason)
        .instance(uri.to_string())
}

fn problem_internal_server_error(reason: String, uri: &Uri) -> http_api_problem::HttpApiProblem {
    http_api_problem::HttpApiProblem::with_title_and_type(
        http_api_problem::StatusCode::INTERNAL_SERVER_ERROR,
    )
        .detail(reason)
        .instance(uri.to_string())
}

fn parse_accept_language(req_language: &HeaderValue, fallback_lang: String) -> String {
    if let Ok(req_language_str) = req_language.to_str() {
        let language_list = req_language_str.split(",").collect::<Vec<&str>>();

        if !language_list.is_empty() {
            let first_language = language_list[0].split(";").collect::<Vec<&str>>();
            first_language[0].to_string()
        } else {
            fallback_lang
        }
    } else {
        fallback_lang
    }
}
struct Broadcaster {
    clients: Vec<Sender<Event>>,
}

impl Broadcaster {
    fn create() -> Arc<Mutex<Self>> {
        let me = Arc::new(Mutex::new(Broadcaster::new()));

        // ping clients every 10 seconds to see if they are alive
        Broadcaster::spawn_ping(me.clone(), 10);

        me
    }

    fn new() -> Self {
        Broadcaster {
            clients: Vec::new(),
        }
    }

    fn spawn_ping(me: Arc<Mutex<Self>>, interval_secs: u64) {
        tokio::spawn(async move {
            let mut interval = interval_at(Instant::now(), Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;
                me.lock().unwrap().remove_stale_clients();
            }
        });
    }

    fn remove_stale_clients(&mut self) {
        let msg = Event::default().data("ping");
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
        let _ = tx.try_send(Event::default().data("connected"));
        self.clients.push(tx);
        let done = false;
        let idx = 0;

        Client {
            _rx,
            refid,
            idx,
            done,
        }
    }
}

// wrap Receiver in own type, with correct error type
struct Client {
    _rx: Receiver<Event>,
    refid: String,
    idx: usize,
    done: bool,
}

impl Stream for Client {
    type Item = Result<Event, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.done {
            return Poll::Ready(None);
        }

        if let Ok(notifications_per_refid) = NOTIFICATIONS_PER_REFID.lock() {
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

                    let evt = Event::default()
                        .data(n.data.clone())
                        .id(n.id.clone())
                        .event(n.event.clone());

                    self.idx = i + 1;

                    return Poll::Ready(Some(Ok(evt)));
                }
            }
        }

        cx.waker().wake_by_ref();

        Poll::Pending
    }
}

#[derive(Debug, Clone)]
enum AppError {
    NotFound(http_api_problem::HttpApiProblem),
    BadRequest(http_api_problem::HttpApiProblem),
    InternalServerError(http_api_problem::HttpApiProblem),
}

impl IntoResponse for AppError {
    // TODO response format based on accept headers: text or json
    fn into_response(self) -> axum::response::Response {
        let (status, problem) = match self {
            AppError::NotFound(reason)            => (StatusCode::NOT_FOUND, reason),
            AppError::BadRequest(reason)          => (StatusCode::BAD_REQUEST, reason),
            AppError::InternalServerError(reason) => (StatusCode::INTERNAL_SERVER_ERROR, reason),
        };

        let json = problem.json_bytes();
        let length = json.len() as u64;

        let mut response = (status, json).into_response();

        *response.status_mut() = status;

        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(http_api_problem::PROBLEM_JSON_MEDIA_TYPE),
        );

        if let Ok(header_value) = HeaderValue::from_str(&length.to_string()) {
            response.headers_mut().insert(header::CONTENT_LENGTH, header_value);
        }

        response
    }
}

pub async fn save_file(
    mut payload: Multipart,
    tmpdir: PathBuf,
    l10n: l10n::Translations,
) -> Result<model::UploadedFile, Box<dyn std::error::Error>> {
    let mut buf         = Vec::<u8>::new();
    let mut filename    = String::new();
    let mut fileext     = String::new();
    let mut ocrlang     = String::new();
    let mut docpassword = String::new();

    while let Ok(Some(field)) = payload.next_field().await {
        if let Some(fname) = field.name() {
            if fname == "file" {
                // Field in turn is stream of *Bytes* object
                if let Ok(chunk) = field.bytes().await {
                    buf.extend(&chunk);
                }
            } else if fname == "filename" {
                if let Ok(chunk) = field.text().await {
                    filename.push_str(&chunk);
                }
            } else if fname == "ocrlang" {
                if let Ok(chunk) = field.text().await {
                    ocrlang.push_str(&chunk);
                }
            } else if fname == "docpasswd" {
                if let Ok(chunk) = field.text().await {
                    docpassword.push_str(&chunk);
                }
            } else {
                tracing::warn!(
                    "{}: {}",
                    l10n.gettext("Unknown upload request field"),
                    fname
                );
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

    let id_token = format!(
        "{};{}",
        base64_lib::encode(&file_uuid.clone().into_bytes()),
        base64_lib::encode(&filename.clone().into_bytes())
    );

    let id = bs58::encode(id_token).into_string();

    let p = std::path::Path::new(&filename);

    if let Some(fext) = p.extension().map(|i| i.to_str()).and_then(|i| i) {
        fileext.push_str(fext);
    } else {
        return Err(format!(
            "{}: {}",
            l10n.gettext("Mime type error! Does the input have a 'known' file extension?"),
            filename
        ).into());
    }

    if !tmpdir.exists() {
        fs::create_dir_all(&tmpdir)?;
    }

    let filepath = tmpdir.join(format!("{}.{}", &file_uuid, fileext));
    let mut f = fs::File::create(&filepath)?;
    f.write_all(&buf)?;

    let location = filepath.display().to_string();

    Ok(model::UploadedFile {
        id,
        docpassword,
        location,
        ocrlang,
        fileext,
    })
}

fn progress_made(
    refid: String,
    event: &str,
    data: String,
    counter: i32,
    err_find_notif: String,
    err_notif_handle: String,
) -> Result<(), Box<dyn std::error::Error>> {
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
    langid: String,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(proposed_ocr_lang) = conversion_options.opt_ocr_lang.clone() {
        let selected_langcodes: Vec<&str> = proposed_ocr_lang.split("+").collect();
        let ocr_lang_by_code = l10n::ocr_lang_key_by_name(&l10n);

        for selected_langcode in selected_langcodes {
            if !ocr_lang_by_code.contains_key(&selected_langcode) {
                return Err(l10n.gettext_fmt("Unknown language code for the ocr-lang parameter: {0}. Hint: Try 'eng' for English.", vec![&proposed_ocr_lang]).into());
            }
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
        input_path.display().to_string(),
    ];

    if let Some(ocr_lang) = conversion_options.opt_ocr_lang {
        cmd_args.push("--ocr-lang".to_string());
        cmd_args.push(ocr_lang);
    }

    if conversion_options.opt_passwd.is_some() {
        cmd_args.push("--passwd-prompt".to_string());
    }

    tracing::info!(
        "{}: {} {}",
        l10n.gettext("Running command"),
        &cmd,
        cmd_args.join(" ")
    );

    let err_find_notif = l10n.gettext("Could not find notification for");
    let err_notif_handle = l10n.gettext("Could not read notifications data");

    let mut env_map: HashMap<String, String> = HashMap::with_capacity(2);
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
    }

    if success {
        let msg = model::CompletionMessage::new(format!("/api/v1/downloads/{}", refid.clone()));

        if let Ok(msg_json) = serde_json::to_string(&msg) {
            progress_made(
                refid.clone(),
                "processing_success",
                msg_json,
                counter,
                err_find_notif,
                err_notif_handle,
            )?;

            if let Err(ex) = fs::remove_file(&input_path) {
                tracing::warn!("{} {}: {}", l10n.gettext("Could not delete file"), input_path.display(), ex.to_string());
            }
        }

        Ok(())
    } else {
        let msg = model::CompletionMessage::new("failure".to_string());

        if let Ok(msg_json) = serde_json::to_string(&msg) {
            progress_made(
                refid.clone(),
                "processing_failure",
                msg_json,
                counter,
                err_find_notif,
                err_notif_handle,
            )?;

            if let Err(ex) = fs::remove_file(&input_path) {
                tracing::warn!("{} {}: {}", l10n.gettext("Could not delete file"), input_path.display(), ex.to_string());
            }
        }

        Err(l10n.gettext("Processing failure").into())
    }
}
