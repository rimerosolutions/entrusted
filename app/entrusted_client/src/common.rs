use std::{error::Error, sync::mpsc::SendError};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::l10n;

pub const CONTAINER_IMAGE_EXE: &str = "/usr/local/bin/entrusted-container";
pub const ENV_VAR_ENTRUSTED_DOC_PASSWD: &str = "ENTRUSTED_DOC_PASSWD";
pub const LOG_FORMAT_JSON: &str = "json";

pub const IMAGE_QUALITY_CHOICES: [&str; 3] = ["low", "medium", "high"];
pub const IMAGE_QUALITY_CHOICE_DEFAULT_INDEX: usize = 1;
pub const DEFAULT_FILE_SUFFIX: &str  = "entrusted";

#[macro_export]
macro_rules! incl_gettext_files {
    ( $( $x:expr ),* ) => {
        {
            let mut ret = HashMap::with_capacity(2);
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

// TODO use a uuid instead of a row index (first usize parameter)
// This doesn't involve too many changes per previous tests that won't make it in 0.2.6
// One annoyance overall is performance to quickly map documents IDs to widgets and cleaning up elegantly resources
// One other detail to watch for is that in case of application crashes we should ensure that all the relevant temporary folders get deleted...
#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum AppEvent {
    FileOpenEvent(String), // file_path
    ConversionProgressEvent(String), // message in JSON format
    ConversionStartEvent(usize), // tasks_index
    ConversionSuccessEvent(usize, usize), // tasks_index, tasks_count
    ConversionFailureEvent(usize, usize), // tasks_index, tasks_count
    ConversionFinishedAckEvent,
    AllConversionEnded(usize, usize, usize) // tasks_completed, tasks_failed, tasks_count
}

pub fn executable_find(exe_name: &str) -> Option<PathBuf> {
    if let Ok(path_location) = which::which(exe_name) {
        Some(path_location)
    } else {
        None
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ReleaseInfo {
    pub html_url: String,
    pub tag_name: String,
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
    pub visual_quality: String,
    pub opt_ocr_lang: Option<String>,
    pub opt_passwd: Option<String>,
    pub seccomp_profile_enabled: bool,
}

impl ConvertOptions {
    pub fn new(container_image_name: String,
               log_format: String,
               visual_quality: String,
               opt_ocr_lang: Option<String>,
               opt_passwd: Option<String>,
               seccomp_profile_enabled: bool,
    ) -> Self {
        Self {
            container_image_name,
            log_format,
            visual_quality,
            opt_ocr_lang,
            opt_passwd,
            seccomp_profile_enabled
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
        ContainerProgramStub::Docker("docker", vec![], vec!["--security-opt=no-new-privileges:true"], None),
        ContainerProgramStub::Podman("podman", vec![], vec!["--userns", "keep-id", "--security-opt", "no-new-privileges"], None),
        ContainerProgramStub::Lima("lima", vec!["nerdctl"], vec!["--security-opt", "no-new-privileges"], Some("/tmp/lima")),
        ContainerProgramStub::Nerdctl("nerdctl", vec![], vec!["--security-opt", "no-new-privileges"], None),
    ];

    let gvisor_enabled = if let Ok(env_gvisor_enablement) = std::env::var("ENTRUSTED_AUTOMATED_GVISOR_ENABLEMENT") {
        env_gvisor_enablement.to_lowercase() == "true" || env_gvisor_enablement.to_lowercase() == "yes"
    } else {
        false
    };
    
    for item in container_program_stubs {
        match item {
            ContainerProgramStub::Docker(cmd, sub_cmd_args, cmd_args, tmp_dir_opt) |
            ContainerProgramStub::Podman(cmd, sub_cmd_args, cmd_args, tmp_dir_opt) |
            ContainerProgramStub::Lima(cmd, sub_cmd_args, cmd_args, tmp_dir_opt)   |
            ContainerProgramStub::Nerdctl(cmd, sub_cmd_args, cmd_args, tmp_dir_opt) => {
                if let Some(path_container_exe) = executable_find(cmd) {
                    let suggested_tmp_dir = tmp_dir_opt.as_ref().map(PathBuf::from);

                    let new_args = if !gvisor_enabled {
                        cmd_args.clone()
                    } else {
                        // 5mb seems enough for temporary file storage required by the LibreOffice user settings creation process...
                        vec!["--security-opt", "no-new-privileges", "--tmpfs", "/loffice:size=5m", "--env", "XDG_CONFIG_HOME=/loffice"]
                    };
                    
                    return Some(ContainerProgram::new(path_container_exe, sub_cmd_args.clone(), new_args, suggested_tmp_dir));
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

pub fn update_check(trans: &l10n::Translations) -> Result<Option<ReleaseInfo>, Box<dyn Error>> {
    let releases_url = "https://api.github.com/repos/rimerosolutions/entrusted/releases/latest";

    let response = minreq::get(releases_url)
        .with_header("User-Agent", "Entrusted Updates Checks")
        .with_header("Accept", "application/json")
        .send()?;

    let release_info: ReleaseInfo = response.json()?;
    let current_version = option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown");

    if current_version == release_info.tag_name {
        Ok(None)
    } else {
        let current_version_text = format!(">{}", current_version);
        let latest_version_text = &release_info.tag_name;

        if let Ok(version_req) = semver::VersionReq::parse(&current_version_text) {
            if let Ok(ver_latest) = semver::Version::parse(latest_version_text) {
                if version_req.matches(&ver_latest) {
                    Ok(Some(release_info))
                } else {
                    Ok(None)
                }
            } else {
                Err(trans.gettext("Could not read latest release version!").into())
            }
        } else {
            Err(trans.gettext("Could not current software version!").into())
        }
    }
}
