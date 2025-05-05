use std::error::Error;
use std::fs;
use std::io;
use std::process::Child;
use std::path::PathBuf;
use std::collections::HashMap;
use std::env;
use std::process::{Command, Stdio};
use std::io::{BufReader, BufRead, Read};
use std::thread::JoinHandle;
use std::thread;
use uuid::Uuid;
use std::time::SystemTime;

use crate::l10n;
use crate::common;
use crate::processing;

fn mkdirp(p: &PathBuf, trans: l10n::Translations) -> Result<(), Box<dyn Error>> {
    if !p.exists() {
        if let Err(ex) = fs::create_dir_all(p) {
            return Err(trans.gettext_fmt("Cannot create directory: {0}! Error: {1}", vec![&p.display().to_string(), &ex.to_string()]).into());
        }
    }

    Ok(())
}

fn cleanup_dir(dir: &PathBuf) -> Result<(), Box<dyn Error>> {
    if dir.exists() && dir.is_dir() {
        let mut files = vec![dir.to_owned()];

        while let Some(f) = files.pop() {
            if f.is_file() && f.exists() {
                fs::remove_file(f)?;
            } else {
                for p in fs::read_dir(&f)? {
                    files.push(p?.path());
                }
            }
        }

        fs::remove_dir(dir)?;
    }

    Ok(())
}

pub fn convert(input_path: PathBuf, output_path: PathBuf, convert_options: common::ConvertOptions, tx: Box<dyn common::EventSender>,  trans: l10n::Translations) -> Result<(), Box<dyn Error>> {
    if !input_path.exists() {
        return Err(trans.gettext_fmt("The selected file does not exists: {0}!", vec![&input_path.display().to_string()]).into());
    }

    let doc_uuid     = Uuid::new_v4().to_string();
    let tmp_dir      = env::temp_dir();    
    let root_tmp_dir = tmp_dir.join(&doc_uuid);
    
    let ctx = processing::ExecCtx::new(doc_uuid,
               root_tmp_dir,
               input_path,    
               output_path,
               convert_options.visual_quality,
               convert_options.opt_ocr_lang,
               convert_options.opt_passwd,     
               trans,
               tx);

    processing::execute(ctx)
}
