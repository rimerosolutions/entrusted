use std::{env, path};

pub fn resolve_sanitizer_settings<P: AsRef<path::Path>>(exe_path: P) -> (Option<path::PathBuf>, Option<path::PathBuf>) {
    let mut office_and_tessdata_paths = (None, None);

    if let Ok(office_dir) = env::var("ENTRUSTED_LIBREOFFICE_PROGRAM_DIR") {
        if let Ok(p) = dunce::canonicalize(path::PathBuf::from(office_dir)) {
            office_and_tessdata_paths.0 = Some(p);
        }
    } else if let Ok(exe_real_path) = dunce::canonicalize(exe_path.as_ref()) {
        if let Some(exe_dir) = exe_real_path.parent() {
            if let Ok(p) = dunce::canonicalize(exe_dir.join("libreoffice").join("program")) {
                office_and_tessdata_paths.0 = Some(p);
            }
        }
    }

    if let Ok(tessdata_dir) = env::var("ENTRUSTED_TESSERACT_TESSDATA_DIR") {
        if let Ok(p) = dunce::canonicalize(path::PathBuf::from(tessdata_dir)) {
            office_and_tessdata_paths.1 = Some(p);
        }
    } else if let Ok(exe_real_path) = dunce::canonicalize(exe_path.as_ref()) {
        if let Some(exe_dir) = exe_real_path.parent() {
            if let Ok(p) = dunce::canonicalize(exe_dir.join("tesseract").join("tessdata")) {
                office_and_tessdata_paths.1 = Some(p);
            }
        }
    }

    office_and_tessdata_paths
}
