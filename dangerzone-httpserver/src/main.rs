use once_cell::sync::Lazy;

use clap;
use std::collections::HashMap;
use std::env;
use std::fs;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

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

use serde::{Deserialize, Serialize};

use actix_multipart::Multipart;
use actix_web::http::Uri;
use futures::TryStreamExt;
use http_api_problem::{HttpApiProblem, StatusCode};
use tokio::io::AsyncWriteExt;

const SPA_INDEX_HTML: &str = include_str!("../web-assets/index.html");

static NOTIFICATIONS_PER_REFID: Lazy<Mutex<HashMap<String, Arc<Mutex<Vec<Notification>>>>>> = Lazy::new(|| {
    Mutex::new(HashMap::<String, Arc<Mutex<Vec<Notification>>>>::new())
});


#[derive(Debug, Clone)]
pub struct Notification {
    pub event: String,
    pub id: String,
    pub data: String,
}

impl Notification {
    pub fn new(event: String, id: String, data: String) -> Self {
        Self { event, id, data }
    }
}

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
        let mut ok_clients = Vec::new();

        for client in self.clients.iter() {
            let result = client.clone().try_send(msg.clone());

            if let Ok(_) = result {
                ok_clients.push(client.clone());
            }
        }

        self.clients = ok_clients;
    }

    fn new_client(&mut self, refid: String) -> Client {
        let (tx, _rx) = channel(100);

        tx.clone()
            .try_send(Bytes::from("data: connected\n\n"))
            .unwrap();

        self.clients.push(tx.clone());
        
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

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct UploadResponse {
    pub id: String,
}

impl UploadResponse {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

async fn notfound() -> impl Responder {
    HttpResponse::NotFound().body("File not found")
}


fn server_problem(reason: String, uri: &Uri) -> HttpApiProblem {
    HttpApiProblem::with_title_and_type_from_status(StatusCode::INTERNAL_SERVER_ERROR)
        .set_detail(reason)
        .set_instance(format!("{}", uri))
}

#[actix_web::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = clap::App::new(option_env!("CARGO_PKG_NAME").unwrap_or("Unknown"))
        .version(option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown"))
        .author(option_env!("CARGO_PKG_AUTHORS").unwrap_or("Unknown"))
        .about(option_env!("CARGO_PKG_DESCRIPTION").unwrap_or("Unknown"))
        .arg(
            clap::Arg::with_name("host")
                .long("host")
                .help("Server host")
                .required(true)
                .default_value("0.0.0.0")
                .takes_value(true),
        )
        .arg(
            clap::Arg::with_name("port")
                .long("port")
                .help("Server port")
                .required(true)
                .default_value("13000")
                .takes_value(true),
        );

    let run_matches = app.to_owned().get_matches();

    if let (Some(host), Some(port)) = (run_matches.value_of("host"), run_matches.value_of("port")) {
        match serve(host, port).await {
            Ok(_) => Ok(()),
            Err(ex) => Err(ex.into()),
        }
    } else {
        Err("Missing parameters?".into())
    }
}

async fn serve(host: &str, port: &str) -> std::io::Result<()> {
    let addr = format!("{}:{}", host, port);
    println!("Starting server at {}", addr.clone());

    HttpServer::new(|| {
        let data = Broadcaster::create();

        App::new()
            .app_data(data.clone())
            .service(web::resource("/").route(web::get().to(index)))
            .route("/upload", web::post().to(upload))
            .route("/events/{id}", web::get().to(events))            
            .route("/downloads/{id}", web::get().to(download))
            .default_service(web::get().to(notfound))
    })
        .bind(addr)?
        .run()
        .await
}

async fn index() -> impl Responder {
    HttpResponse::Ok()
            .append_header((header::CONTENT_TYPE, "text/html"))
            .body(SPA_INDEX_HTML)
}

async fn download(info: actix_web::web::Path<String>, req: HttpRequest) -> impl Responder {
    println!("Download from: {}", req.uri());

    let file_id = info.into_inner();
    let filename = [file_id.clone(), "-safe.pdf".to_string()].concat();
    let filepath = env::temp_dir().join(filename.clone());
    let filepath_buf = PathBuf::from(filepath);

    if !filepath_buf.exists() {
        NOTIFICATIONS_PER_REFID.lock().unwrap().remove(&file_id.to_string());
        HttpResponse::NotFound().body("File not found")
    } else {
        match fs::read(filepath_buf.clone()) {
            Ok(data) => {
                let _ = std::fs::remove_file(filepath_buf);
                NOTIFICATIONS_PER_REFID.lock().unwrap().remove(&file_id.to_string());
                HttpResponse::Ok()
                    .append_header((header::CONTENT_TYPE, "application/pdf"))
                    .append_header((header::CONTENT_DISPOSITION, format!("attachment; filename={}", filename)))
                    .append_header((header::CONTENT_LENGTH, format!("{}", data.len())))
                    .body(data)
            }
            Err(ex) => {
                eprintln!("Internal error: {}", ex.to_string());
                let _ = std::fs::remove_file(filepath_buf);
                NOTIFICATIONS_PER_REFID.lock().unwrap().remove(&file_id.to_string());
                HttpResponse::InternalServerError().body("Internal error")
            }
        }
    }
}

async fn events(info: actix_web::web::Path<String>, broadcaster: Data<Mutex<Broadcaster>>) -> impl Responder {
    let ref_id = format!("{}", info.into_inner());    
    let notifications_per_refid = NOTIFICATIONS_PER_REFID.lock().unwrap();

    if !notifications_per_refid.contains_key(&ref_id.clone()) {
        return HttpResponse::NotFound().body("Not found");
    }
    
    let rx = broadcaster.lock().unwrap().new_client(ref_id);

    HttpResponse::Ok()
        .append_header((header::CONTENT_TYPE, "text/event-stream"))
        .streaming(rx)
}

async fn upload(req: HttpRequest, payload: Multipart) -> impl Responder {
    let request_id = Uuid::new_v4().to_string();
    let request_id_clone = request_id.clone();

    println!("Starting file upload with refid: {}", request_id.clone());

    let tmpdir = env::temp_dir();
    let input_path_status = save_file(request_id.clone(), payload, tmpdir.clone()).await;
    let mut err_msg = String::new();
    let mut upload_info = (String::new(), String::new(), String::new());

    match input_path_status {
        Ok((ocr_lang, fileext, filepath)) => {
            upload_info = (ocr_lang, fileext, filepath);
        }
        Err(ex) => {
            err_msg.push_str(&ex.to_string());
        }
    }

    if err_msg.is_empty() {
        NOTIFICATIONS_PER_REFID.lock().unwrap().insert(request_id.clone(), Arc::new(Mutex::new(Vec::<Notification>::new())));

        let new_upload_info = upload_info.clone();
        actix_web::rt::spawn(async move {
            let ocr_lang_opt = if new_upload_info.0.is_empty() {
                None
            } else {
                Some(new_upload_info.0.clone())
            };
            let input_path = PathBuf::from(new_upload_info.2);
            let output_path =
                PathBuf::from(tmpdir).join([request_id.clone(), "-safe.pdf".to_string()].concat());

            if let Err(ex) = run_dangerzone(request_id, input_path, output_path, ocr_lang_opt).await {
                eprintln!("Conversion failed. {}", ex.to_string());
            }
        });
    };

    if err_msg.is_empty() {
        HttpResponse::Ok().json(UploadResponse::new(request_id_clone))
    } else {
        HttpResponse::InternalServerError().json(server_problem(
            err_msg,
            req.uri(),
        ))
    }
}

pub async fn save_file(
    request_id: String,
    mut payload: Multipart,
    tmpdir: PathBuf,
) -> Result<(String, String, String), Box<dyn std::error::Error>> {
    let mut buf = Vec::<u8>::new();
    let mut filename = String::new();
    let mut fileext = String::new();
    let mut ocr_lang  = String::new();

    while let Ok(Some(mut field)) = payload.try_next().await {
        let fname = field.name();
        if fname == "file" {
            // Field in turn is stream of *Bytes* object
            while let Some(chunk) = field.next().await {
                buf.extend(&chunk?.to_vec());
            }
        } else if fname == "filename" {
            while let Some(chunk) = field.next().await {
                filename.push_str(&String::from_utf8(chunk?.to_vec())?);
            }
        } else if fname == "ocrlang" {
            while let Some(chunk) = field.next().await {
                let data = &String::from_utf8(chunk?.to_vec())?;

                if !data.trim().is_empty() {                    
                    ocr_lang.push_str(data);
                }
            }
        }
    }

    if filename.is_empty() {
        return Err("Missing required 'filename' parameter.".into());
    }

    if buf.is_empty() {
        return Err("Missing required 'file' parameter for the document binary data.".into());
    }

    let p = std::path::Path::new(&filename);
    if let Some(fext) = p.extension().map(|i| i.to_str()) {
        if let Some(ext) = fext {
            fileext.push_str(ext);
        }
    }

    if fileext.len() == 0 {
        return Err(format!("Is the document type supported? Could not determine file extension for filename: {}", filename).into());
    }

    let filepath = tmpdir.join(format!("{}.{}", request_id, fileext));
    let mut f = tokio::fs::File::create(filepath.clone()).await?;
    f.write_all(&buf).await?;

    Ok((ocr_lang, fileext, format!("{}", filepath.display())))
}

fn progress_made(refid: String, event: &str, data: String, counter: i32) -> Result<(), Box<dyn std::error::Error>> {
    let nevent = event.to_string();
    let nid = format!("{}", counter);
    let n = Notification::new(nevent, nid, data.clone());

    if let Ok(bc) = NOTIFICATIONS_PER_REFID.lock() {
        if let Some(v) = bc.get(&refid) {
            v.lock().unwrap().push(n);
            Ok(())
        } else {
            Err(format!("Could not find notification group for : {}", refid).into())
        }
    } else {
        Err("Could not send notification".into())
    }
}

async fn run_dangerzone(
    refid: String,
    input_path: PathBuf,
    output_path: PathBuf,
    opt_ocr_lang: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmdline = vec![
        "dangerzone-cli".to_string(),
        "--output-filename".to_string(),
        format!("{}", output_path.display()),
        "--input-filename".to_string(),
        format!("{}", input_path.display()),
    ];

    if let Some(ocr_lang) = opt_ocr_lang {
        cmdline.push("--ocr-lang".to_string());
        cmdline.push(ocr_lang);
    }

    println!("Running command: {}", cmdline.join(" "));

    let mut cmd = Command::new("sh")
        .arg("-c")
        .arg(cmdline.join(" "))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let mut counter = 1;
    let stream = cmd.stdout.take().unwrap();

    let mut reader = BufReader::new(stream).lines();

    while let Some(ndata) = reader.next_line().await? {
        progress_made(refid.clone(), "processing_update", ndata, counter)?;
        counter += 1;
    }

    if let Ok(s) = cmd.try_wait() {
        if let Some(ss) = s {
            if ss.success() {
                progress_made(refid.clone(), "processing_success", "success".to_string(), counter)?;
                let _ = std::fs::remove_file(input_path);

                return Ok(());
            }
        }
    }

    progress_made(refid.clone(), "processing_failure", "failure".to_string(), counter)?;
    let _ = std::fs::remove_file(input_path);

    Err("Processing failure".into())
}
