use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::{Child, Command};
use which;

pub fn exe_find(exe_name: &str) -> Option<PathBuf> {
    if let Ok(path_location) = which::which(exe_name) {
        Some(path_location)
    } else {
        None
    }
}

#[cfg(not(target_os = "windows"))]
pub fn spawn_cmd(
    cmd: String,
    cmd_args: Vec<String>,
    env_map: HashMap<String, String>,
) -> std::io::Result<Child> {
    Command::new(cmd)
        .envs(env_map)
        .args(cmd_args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

#[cfg(target_os = "windows")]
pub fn spawn_cmd(
    cmd: String,
    cmd_args: Vec<String>,
    env_map: HashMap<String, String>,
) -> std::io::Result<Child> {
    use std::os::windows::process::CommandExt;
    Command::new(cmd)
        .envs(env_map)
        .args(cmd_args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000)
        .spawn()
}
