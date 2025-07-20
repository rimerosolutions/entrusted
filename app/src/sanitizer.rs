use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::env;

use uuid::Uuid;

use crate::l10n;
use crate::error;
use crate::common;
use crate::processing;

#[derive(Clone)]
pub struct Sanitizer {
    office_opt: Option<String>,
    tessdata_opt: Option<String>,
}

impl Sanitizer {
    pub fn new(data: (Option<PathBuf>, Option<PathBuf>)) -> Self {
        Self {
            office_opt:   init_office(data.0),
            tessdata_opt: init_tessdata(data.1),
        }
    }

    pub fn sanitize(&self, doc_uuid: Uuid, input_path: PathBuf, convert_options: common::ConvertOptions, tx: Box<dyn common::EventSender>, trans: l10n::Translations, stop_signal: Arc<AtomicBool>) -> Result<Option<PathBuf>, error::Failure> {
        if !input_path.exists() {
            let msg = trans.gettext_fmt("The selected file does not exists: {0}!", vec![&input_path.display().to_string()]);
            return Err(std::io::Error::other(msg).into());
        }

        let tmp_dir      = env::temp_dir();
        let root_tmp_dir = tmp_dir.join(common::NAMESPACE_APP).join(doc_uuid.to_string());
        let output_path  = output_file_path(&input_path, convert_options.output_folder, &convert_options.filename_suffix, &trans)?;

        mkdirp(&root_tmp_dir, &trans)?;

        let ctx = processing::ExecCtx::new(doc_uuid,
                                           root_tmp_dir.clone(),
                                           input_path.clone(),
                                           output_path.clone(),
                                           convert_options.visual_quality,
                                           convert_options.ocr_lang_code,
                                           convert_options.password_decrypt,
                                           convert_options.password_encrypt,
                                           trans,
                                           tx);

        let ret = processing::execute(ctx, &self.office_opt, &self.tessdata_opt, stop_signal);
        let _ = rmdir(&root_tmp_dir);

        match ret {
            Ok(output_path_when_not_interrupted) => Ok(output_path_when_not_interrupted),
            Err(ex)                              => Err(ex)
        }
    }
}

fn init_tessdata(tessdata_dir: Option<PathBuf>) -> Option<String> {
    tessdata_dir.map(|i| i.display().to_string())
}


fn init_office(office_dir: Option<PathBuf>) -> Option<String> {
    office_dir.map(|i| i.display().to_string())
}

fn mkdirp(p: &Path, trans: &l10n::Translations) -> Result<(), std::io::Error> {
    if !p.exists() {
        if let Err(ex) = fs::create_dir_all(p) {
            let msg = trans.gettext_fmt("Cannot create directory: {0}! Error: {1}", vec![&p.display().to_string(), &ex.to_string()]);
            return Err(std::io::Error::other(msg));
        }
    }

    Ok(())
}

fn rmdir(dir: &Path) -> Result<(), std::io::Error> {
    if dir.exists() && dir.is_dir() {
        fs::remove_dir_all(dir)?;
    }

    Ok(())
}

fn output_file_path(input_path: &Path, output_folder_opt: Option<PathBuf>, file_suffix: &str, trans: &l10n::Translations) -> Result<PathBuf, std::io::Error> {
    let input_name_opt = input_path.file_stem().map(|i| i.to_str()).and_then(|v| v);
    let mut output_dir_opt = output_folder_opt;

    if output_dir_opt.is_none() {
        output_dir_opt = input_path.parent().map(|i| i.to_path_buf());
    }

    if let (Some(input_name), Some(output_location)) = (input_name_opt, output_dir_opt) {
        let dest_path = {
            let filename = format!("{}-{}.pdf", &input_name, file_suffix);
            let mut dest_location = output_location.join(filename);
            let mut counter = 1;

            while dest_location.exists() {
                let dest_filename = format!("{}-{}.{}.pdf", &input_name, file_suffix, counter);
                dest_location = output_location.join(dest_filename);
                counter +=1;
            }

            dest_location
        };

        Ok(dest_path)
    } else {
        let msg = trans.gettext("Cannot determine resulting PDF output path based on selected input document location!");
        Err(std::io::Error::other(msg))
    }
}
