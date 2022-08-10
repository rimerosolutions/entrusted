use std::{error::Error, sync::mpsc::SendError};
use std::path::PathBuf;
use which;
use serde::{Deserialize, Serialize};

pub const CONTAINER_IMAGE_EXE: &str = "/usr/local/bin/entrusted-container";
pub const ENV_VAR_ENTRUSTED_DOC_PASSWD: &str = "ENTRUSTED_DOC_PASSWD";
pub const LOG_FORMAT_JSON: &str = "json";

#[macro_export]
macro_rules! incl_gettext_files {
    ( $( $x:expr ),* ) => {
        {
            let mut ret = HashMap::new();
            $(
                let data = include_bytes!(concat!("../translations/", $x, "/LC_MESSAGES/messages.mo")).as_slice();
                ret.insert($x, data);

            )*

                ret
        }
    };
}

pub trait EventSender: Send {
    fn send(&self, evt: crate::common::AppEvent) -> Result<(), SendError<crate::common::AppEvent>>;

    fn clone_box(&self) -> Box<dyn EventSender>;
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum AppEvent {
    FileOpenEvent(String),
    ConversionProgressEvent(String),
    ConversionStartEvent(usize),
    ConversionSuccessEvent(usize, Option<String>, PathBuf),
    ConversionFailureEvent(usize),
    ConversionFinishedAckEvent
}

pub fn executable_find(exe_name: &str) -> Option<PathBuf> {
    match which::which(exe_name) {
        Err(_) => None,
        Ok(path_location) => Some(path_location)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct LogMessage {
    pub data: String,
    pub percent_complete: usize,
}

#[derive(Clone)]
pub struct ConvertOptions {
    pub container_image_name: String,
    pub log_format: String,
    pub opt_ocr_lang: Option<String>,
    pub opt_passwd: Option<String>
}

impl ConvertOptions {
    pub fn new(container_image_name: String,
               log_format: String,
               opt_ocr_lang: Option<String>,
               opt_passwd: Option<String>) -> Self {
        Self {
            container_image_name,
            log_format,
            opt_ocr_lang,
            opt_passwd
        }
    }
}

#[derive(Clone)]
pub struct ContainerProgram<'a>{
    pub exec_path: PathBuf,
    pub sub_commands: Vec<&'a str>,
    pub suggested_run_args: Vec<&'a str>,
    pub suggested_tmp_dir: Option<PathBuf>,
}

impl<'a> ContainerProgram<'a> {
    pub fn new(exec_path: PathBuf, sub_commands: Vec<&'a str>, suggested_run_args: Vec<&'a str>, suggested_tmp_dir: Option<PathBuf>) -> Self {
        Self {
            exec_path,
            sub_commands,
            suggested_run_args,
            suggested_tmp_dir
        }
    }
}

enum ContainerProgramStub<'a> {
    Docker(&'a str, Vec<&'a str>, Vec<&'a str>, Option<&'a str>),
    Podman(&'a str, Vec<&'a str>, Vec<&'a str>, Option<&'a str>),
    Lima(&'a str, Vec<&'a str>, Vec<&'a str>, Option<&'a str>),
    Nerdctl(&'a str, Vec<&'a str>, Vec<&'a str>, Option<&'a str>)
}

// TODO this is not good enough, ideally subcommands should be captured at a higher level
// Especially for Lima and similar tooling, to avoid further downstream conditional blocks
pub fn container_runtime_path<'a>() -> Option<ContainerProgram<'a>> {
    let container_program_stubs = [
        ContainerProgramStub::Docker("docker", vec![], vec![], None),
        ContainerProgramStub::Podman("podman", vec![], vec!["--userns", "keep-id"], None),
        ContainerProgramStub::Lima("lima", vec!["nerdctl"], vec![], Some("/tmp/lima")),
        ContainerProgramStub::Nerdctl("nerdctl", vec![], vec![], None),
    ];

    for i in 0..container_program_stubs.len() {
        match &container_program_stubs[i] {
            ContainerProgramStub::Docker(cmd, sub_cmd_args, cmd_args, tmp_dir_opt) |
            ContainerProgramStub::Podman(cmd, sub_cmd_args, cmd_args, tmp_dir_opt) |
            ContainerProgramStub::Lima(cmd, sub_cmd_args, cmd_args, tmp_dir_opt)   |
            ContainerProgramStub::Nerdctl(cmd, sub_cmd_args, cmd_args, tmp_dir_opt) => {
                if let Some(path_container_exe) = executable_find(cmd) {
                    let suggested_tmp_dir = if let Some(tmp_dir) = tmp_dir_opt {
                        Some(PathBuf::from(tmp_dir))
                    } else {
                        None
                    };
                    return Some(ContainerProgram::new(path_container_exe, sub_cmd_args.clone(), cmd_args.clone(), suggested_tmp_dir));
                }
            }
        }
    }

    None
}

pub fn default_output_path(input: PathBuf, file_suffix: String) -> Result<PathBuf, Box<dyn Error>> {
    let input_name_opt = input.file_stem().map(|i| i.to_str()).and_then(|v| v);
    let output_filename_opt = input.parent().map(|i| i.to_path_buf());

    if let (Some(input_name), Some(mut output_filename)) = (input_name_opt, output_filename_opt) {
        let filename = format!("{}-{}.pdf", &input_name, &file_suffix);
        output_filename.push(filename);
        Ok(output_filename)
    } else {
        Err("Cannot determine resulting PDF output path based on selected input document location!".into())
    }
}
