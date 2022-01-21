use std::thread;
use std::error::Error;
use std::sync::{mpsc, Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::cell::RefCell;
use std::cmp;
use std::process::Command;
use which;
use fltk::{
    app, button, prelude::*, group, window, dialog, input, misc, enums, text, browser
};

mod common;
mod container;

pub struct ConversionParams {
    pub path_in: PathBuf,
    pub path_out: PathBuf,
    pub ocr_lang: Option<String>,
}

pub fn show_progress_dialog(x: i32, y: i32, w: i32, h: i32, conversion_params: ConversionParams) -> bool {
    let dw = 400;
    let dh = 300;
    let dx =  cmp::max(0, (x + w / 2) - (dw / 2));
    let dy =  cmp::max(0, (y + h / 2) - (dh / 2));

    do_show_progress_dialog(dx, dy, dw, dh, conversion_params)
}

fn do_show_progress_dialog(x: i32, y: i32, w: i32, h: i32, conversion_params: ConversionParams) -> bool {
    let mut win = window::Window::new(x, y, w, h, "Progress dialog");
    win.set_color(enums::Color::from_rgb(240, 240, 240));
    win.make_resizable(true);

    let mut pack = group::Pack::default()
        .with_size(380, 180)
        .center_of_parent()
        .with_type(group::PackType::Vertical);
    pack.set_spacing(20);
    let mut inp = text::TextDisplay::default_fill().with_label("Conversion output").with_size(280, 100);
    let text_buffer = text::TextBuffer::default();
    inp.set_buffer(text_buffer);
    inp.deactivate();
    pack.resizable(&inp);

    let mut ok = button::Button::default().with_size(80, 20).with_label("Converting...");
    let mut ok_clone = ok.clone();
    pack.end();
    win.end();
    win.make_modal(true);
    win.show();
    ok.deactivate();
    ok.set_callback({
        let mut win = win.clone();
        move |_| {
            win.hide();
        }
    });

    win.set_callback({
        move |win_instance| {
            if ok.active() {
                win_instance.hide();
            }
        }
    });

    let (tx, rx) = mpsc::channel();

    let mut exec_handle = Some(thread::spawn(move || {
        match container::convert(conversion_params.path_in, conversion_params.path_out, None, conversion_params.ocr_lang, tx) {
            Ok(_) => None,
            Err(ex) => Some(format!("{}", ex))
        }
    }));

    let mut inp_copy = inp.clone();
    let result = Arc::new(AtomicBool::new(false));

    while win.shown() {
        app::wait();

        if !ok_clone.active() {
            if let Ok(raw_msg) = rx.recv() {
                let msg = format!("{}\n", raw_msg);
                inp_copy.insert(msg.as_str());
                inp_copy.scroll(inp_copy.insert_position(), 0);
            } else {
                if !ok_clone.active() {
                    ok_clone.set_label("Close window");
                    ok_clone.activate();
                    inp_copy.activate();

                    match exec_handle.take().map(thread::JoinHandle::join) {
                        Some(xxx) => {
                            match xxx {
                                Ok(None) => {
                                    inp_copy.set_label_color(enums::Color::DarkGreen);
                                    result.swap(true, Ordering::Relaxed);
                                },
                                _ => {
                                    inp_copy.set_label_color(enums::Color::Red);
                                }
                            }
                        },
                        None => {
                            inp_copy.set_label_color(enums::Color::Red);
                        }
                    }
                }
            }
        }
    }

    result.load(Ordering::Relaxed)
}


fn main() -> Result<(), Box<dyn Error>> {
    let app = app::App::default().with_scheme(app::Scheme::Gleam);

    let mut wind = window::Window::default()
        .with_size(600, 300)
        .center_screen()
        .with_label("Dangerzone");
    let wind_clone = wind.clone();
    wind.make_resizable(true);

    let mut input_row = group::Pack::default().with_pos(20, 30)
        .with_size(570, 30)
        .with_type(group::PackType::Horizontal);
    input_row.set_spacing(10);
    let input_inputfile = Rc::new(RefCell::new(input::Input::default().with_size(400, 20)));
    input_inputfile.borrow_mut().set_tooltip("Path to document to convert");
    let c_input_inputfile = input_inputfile.clone();
    let mut button_inputfile = button::Button::default().with_size(140, 20).with_label("Select document...");

    button_inputfile.set_callback({
        move |_| {
            let mut dlg = dialog::FileDialog::new(dialog::FileDialogType::BrowseFile);
            dlg.set_title("Select suspicious file");
            dlg.show();

            let selected_filename = dlg.filename();

            if !selected_filename.as_os_str().is_empty() {
                let path_name = format!("{}", dlg.filename().display());
                let path_str = path_name.as_str();
                input_inputfile.borrow_mut().set_value(path_str);
            }
        }
    });
    input_row.end();

    let mut row_inputloc = group::Pack::default()
        .with_size(570, 40)
        .below_of(&input_row, 10)
        .with_type(group::PackType::Horizontal);
    row_inputloc.set_spacing(10);
    let mut checkbutton_custom_output = button::CheckButton::default().with_size(160, 20).with_label("Custom output name");
    checkbutton_custom_output.set_tooltip("The safe PDF will be named <input>-safe.pdf by default.");
    checkbutton_custom_output.set_checked(false);

    let input_outputloc = Rc::new(RefCell::new(input::Input::default().with_size(290, 20)));
    let c_input_outputloc = input_outputloc.clone();
    let cc_input_outputloc = input_outputloc.clone();
    let ccc_input_outputloc = input_outputloc.clone();
    input_outputloc.borrow_mut().deactivate();
    let mut button_saveas = button::Button::default().with_size(80, 20).with_label("Save as...");
    button_saveas.deactivate();
    let output_path_string = Rc::new(RefCell::new(String::new()));

    button_saveas.set_callback({
        move |_| {
            let mut dlg = dialog::FileDialog::new(dialog::FileDialogType::BrowseSaveFile);
            dlg.set_title("Save As");
            dlg.set_option(dialog::FileDialogOptions::SaveAsConfirm);
            dlg.show();

            let selected_filename = dlg.filename();

            if !selected_filename.as_os_str().is_empty() {
                let path_name = format!("{}", dlg.filename().display());
                let path_str = path_name.as_str();
                let _xx = &output_path_string.borrow_mut().push_str(path_str);
                input_outputloc.borrow_mut().set_value(path_str);
            }
        }
    });

    checkbutton_custom_output.set_callback({
        move |b| {
            let will_be_readonly = !b.is_checked();
            c_input_outputloc.borrow_mut().set_readonly(will_be_readonly);

            if will_be_readonly {
                cc_input_outputloc.borrow_mut().set_value("");
                c_input_outputloc.borrow_mut().deactivate();
                button_saveas.deactivate();
            } else {
                c_input_outputloc.borrow_mut().activate();
                button_saveas.activate();
            }
        }
    });

    row_inputloc.end();

    let mut row_openwith = group::Pack::default().with_size(570, 40).below_of(&row_inputloc, 10);
    row_openwith.set_type(group::PackType::Horizontal);
    row_openwith.set_spacing(10);
    let mut checkbutton_openwith = button::CheckButton::default().with_size(340, 20).with_label("Open safe document after converting, using");

    let pdf_apps_by_name = list_apps_for_pdfs();
    let pdf_viewer_list = Rc::new(RefCell::new(misc::InputChoice::default().with_size(200, 20)));
    let cc_pdf_viewer_list = pdf_viewer_list.clone();
    let mut pdf_viewer_app_names = Vec::with_capacity(pdf_apps_by_name.len());

    for (k, _v) in &pdf_apps_by_name {
        pdf_viewer_app_names.push(k.as_str());
    }

    pdf_viewer_app_names.sort();

    for k in pdf_viewer_app_names {
        pdf_viewer_list.borrow_mut().add(k);
    }

    pdf_viewer_list.borrow_mut().set_tooltip("You can also paste the path to a PDF viewer");

    if pdf_apps_by_name.len() != 0 {
        pdf_viewer_list.borrow_mut().set_value_index(0);
        checkbutton_openwith.set_checked(true);
    }

    checkbutton_openwith.set_callback({
        move |_| {
            let will_be_read_only = !pdf_viewer_list.borrow_mut().input().readonly();
            pdf_viewer_list.borrow_mut().input().set_readonly(will_be_read_only);

            if will_be_read_only {
                pdf_viewer_list.borrow_mut().deactivate();
            } else {
                pdf_viewer_list.borrow_mut().activate();
            };
        }
    });

    row_openwith.end();

    let mut row_ocr_language = group::Pack::default().with_size(570, 60).below_of(&row_openwith, 10);
    row_ocr_language.set_type(group::PackType::Horizontal);
    row_ocr_language.set_spacing(10);
    let mut checkbutton_ocr_lang = button::CheckButton::default().with_size(300, 20).with_label("OCR document, language");
    checkbutton_ocr_lang.set_tooltip("Make the PDF searchable, with a given language for OCR (Optical character recognition).");
    checkbutton_ocr_lang.set_checked(true);

    let ocr_language_list = Rc::new(RefCell::new(browser::HoldBrowser::default().with_size(240, 60)));
    let c_ocr_language_list = ocr_language_list.clone();
    let ocr_languages_by_name = common::ocr_lang_key_by_name();
    let mut ocr_languages_by_lang = HashMap::with_capacity(ocr_languages_by_name.len());
    let mut ocr_languages: Vec<&str> = Vec::with_capacity(ocr_languages_by_name.len());

    for (k, v) in ocr_languages_by_name {
        ocr_languages_by_lang.insert(v, k);
        ocr_languages.push(v);
    }

    ocr_languages.sort();

    for v in ocr_languages.iter() {
        ocr_language_list.borrow_mut().add(v);
    }

    if let Some(selected_ocr_language_idx) = ocr_languages.iter().position(|&r| r == "English") {
        ocr_language_list.borrow_mut().select( (selected_ocr_language_idx + 1) as i32);
    }

    checkbutton_ocr_lang.set_callback({
        move |b| {
            if !b.is_checked() {
                ocr_language_list.borrow_mut().deactivate();
            } else {
                ocr_language_list.borrow_mut().activate();
            }
        }
    });
    row_ocr_language.end();

    let mut row_convert_button = group::Pack::default().with_size(500, 40).below_of(&row_ocr_language, 10);
    row_convert_button.set_type(group::PackType::Horizontal);
    row_convert_button.set_spacing(10);
    let mut button_convert = button::Button::default().with_size(200, 20).with_label("Convert to Safe Document");

    button_convert.set_callback({
        move |b| {
            let str_inputloc = c_input_inputfile.borrow().value();
            let str_outputloc = ccc_input_outputloc.borrow().value();
            let ocr_language_list_ref = c_ocr_language_list.borrow_mut();

            if !str_inputloc.is_empty() {
                let mut bb = b.clone();

                // TODO the list of detected viewer apps, could be empty
                let viewer_app_name = cc_pdf_viewer_list.borrow_mut().input().value();
                let viewer_app_exec = if checkbutton_openwith.is_checked() {
                    if let Some(viewer_app_path) = pdf_apps_by_name.get(&viewer_app_name) {
                        Some(viewer_app_path.clone())
                    } else {
                        Some(String::from(viewer_app_name.trim()))
                    }
                } else {
                    None
                };

                let path_in = PathBuf::from(str_inputloc);

                if path_in.exists() {
                    let mut err_msg = "".to_string();

                    let path_out = if str_outputloc.is_empty() {
                        let path_in_clone = path_in.clone();
                        if let Ok(path_destloc) = common::default_output_path(path_in_clone) {
                            path_destloc
                        } else {
                            err_msg.push_str("ERROR: Cannot automatically set the default output location in the input directory!");
                            PathBuf::from("")
                        }
                    } else {
                        PathBuf::from(str_outputloc)
                    };
                    let path_out_clone = path_out.clone();

                    let ocr_lang = if checkbutton_ocr_lang.is_checked() {
                        if let Some(selected_lang) = ocr_language_list_ref.selected_text() {
                            ocr_languages_by_lang.get(selected_lang.as_str()).map(|i| format!("{}", i))
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    let conversion_params = ConversionParams {
                        path_in, path_out, ocr_lang,
                    };

                    if err_msg.is_empty() {
                        b.deactivate();
                        if show_progress_dialog(wind_clone.x(), wind_clone.y(), wind_clone.width(), wind_clone.height(), conversion_params) {
                            if let Some(viewer_app_executable) = viewer_app_exec {
                                if let Ok(_) = pdf_open_with(viewer_app_executable, path_out_clone) {
                                } else {
                                    dialog::alert(wind_clone.x(), wind_clone.y() + wind_clone.height() / 2, "Failed to launch PDF viewer!");
                                }
                            }
                        }
                        bb.activate();
                    } else {
                        dialog::alert(wind_clone.x(), wind_clone.y() + wind_clone.height() / 2, err_msg.as_str());
                    }
                }
            }
        }
    });

    row_convert_button.resizable(&row_convert_button);
    row_convert_button.end();

    wind.end();
    wind.show();

    match app.run() {
        Ok(_) => Ok(()),
        Err(ex) => Err(ex.into())
    }
}

#[cfg(not(any(target_os = "macos")))]
pub fn pdf_open_with(cmd: String, input: PathBuf) -> Result<(), Box<dyn Error>> {
    match Command::new(cmd).arg(input).spawn() {
        Ok(_) => Ok(()),
        Err(ex) => Err(ex.into())
    }
}

#[cfg(target_os = "macos")]
pub fn pdf_open_with(cmd: String, input: PathBuf) -> Result<(), Box<dyn Error>> {
    match which::which("open") {
        Ok(open_cmd) => {
            match Command::new(open_cmd).arg("-a").arg(cmd).arg(input).spawn() {
                Ok(mut child_proc) => {
                    if let Ok(exit_status) = child_proc.wait() {
                        if exit_status.success() {
                            Ok(())
                        } else {
                            Err("Cannot run PDF viewer".into())
                        }
                    } else {
                        Err("Cannot run PDF viewer".into())
                    }
                },
                Err(ex) => Err(ex.into())
            }
        },
        Err(ex) => Err(ex.into())
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn list_apps_for_pdfs() -> HashMap<String, String> {
    HashMap::new()
}

#[cfg(target_os="linux")]
pub fn list_apps_for_pdfs() -> HashMap<String, String> {
    use freedesktop_entry_parser::parse_entry;
    use std::env;

    // See https://wiki.archlinux.org/title/XDG_MIME_Applications for the logic

    // TODO is TryExec the best way to get a program name vs 'Exec' and stripping arguments???
    // Exec=someapp -newtab %u => where '%u' could be the file input parameter on top of other defaults '-newtab'

    fn parse_desktop_apps(apps_dir: PathBuf, mime_pdf_desktop_refs: &str) -> HashMap<String, String> {
        let desktop_entries: Vec<&str> = mime_pdf_desktop_refs.split(";").collect();
        let mut result = HashMap::with_capacity(desktop_entries.len());

        for desktop_entry in desktop_entries {
            if desktop_entry.is_empty() {
                continue;
            }

            let mut desktop_entry_path =  apps_dir.clone();
            desktop_entry_path.push(desktop_entry);

            if desktop_entry_path.exists() {
                if let Ok(desktop_entry_data) = parse_entry(desktop_entry_path) {
                    let desktop_entry_section = desktop_entry_data.section("Desktop Entry");

                    if let (Some(app_name), Some(cmd_name)) = (&desktop_entry_section.attr("Name"), &desktop_entry_section.attr("TryExec")) {
                        result.insert(app_name.to_string(), cmd_name.to_string());
                    }
                }

            }
        }

        result
    }

    let path_usr_share_applications_orig = PathBuf::from("/usr/share/applications");
    let mut ret: HashMap<String, String> = HashMap::new();
    let mut path_mimeinfo_cache = path_usr_share_applications_orig.clone();
    path_mimeinfo_cache.push("mimeinfo.cache");

    if path_mimeinfo_cache.exists() {
        if let Ok(conf) = parse_entry(path_mimeinfo_cache) {
            if let Some(mime_pdf_desktop_refs) = conf.section("MIME Cache").attr("application/pdf") {
                let tmp_result = parse_desktop_apps(path_usr_share_applications_orig.clone(), mime_pdf_desktop_refs);

                for (k, v) in &tmp_result {
                    ret.insert(k.to_string(), v.to_string());
                }
            }
        }

    }

    let mut additional_xdg_files = vec![
        PathBuf::from("/etc/xdg/mimeapps.list"),
        PathBuf::from("/usr/local/share/applications/mimeapps.list"),
        PathBuf::from("/usr/share/applications/mimeapps.list")
    ];

    if let Ok(homedir) = env::var("HOME") {
        let home_config_mimeapps: PathBuf = [homedir.as_str(), ".config/mimeapps.list"].iter().collect();
        let home_local_mimeapps: PathBuf = [homedir.as_str(), ".local/share/applications/mimeapps.list"].iter().collect();
        additional_xdg_files.push(home_config_mimeapps);
        additional_xdg_files.push(home_local_mimeapps);
    }

    for additional_xdg_file in additional_xdg_files {
        if additional_xdg_file.exists() {
            if let Ok(conf) = parse_entry(additional_xdg_file) {
                if let Some(mime_pdf_desktop_refs) = conf.section("Added Associations").attr("application/pdf") {
                    let tmp_result = parse_desktop_apps(path_usr_share_applications_orig.clone(), mime_pdf_desktop_refs);

                    for (k, v) in &tmp_result {
                        ret.insert(k.to_string(), v.to_string());
                    }
                }
            }
        }
    }

    ret
}

// TODO windows support hasn't really been tested at all for this part and more generally speaking...
#[cfg(target_os="windows")]
pub fn list_apps_for_pdfs() -> HashMap<String, String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_CURRENT_USER);
    let hkcr = RegKey::pref(HKEY_CLASSES_ROOT);
    let open_with_list = hklm.open_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts\\.pdf\\OpenWithProgIds");
    let app_name_candidates = HashSet::new();

    if let Ok(open_with_list_result) = open_with_list {
        let mru_list_name = "MRUList".to_string();

        for (name, value) in open_with_list_result.enum_values().map(|x| x.unwrap()) {
            if value.vtype == RegType::REG_SZ && name != mru_list_name {
                app_name_candidates.insert(name);
            }
        }
    }

    if let Ok(root_pdf_app_list) = hkcr.open_subkey(".pdf\\OpenWithProgids") {
        for (name, _) in root_pdf_app_list.enum_values().map(|x| x.unwrap()) {
            app_name_candidates.insert(name);
        }
    }

    let mut ret = HashMap::with_capacity(app_name_candidates.len());

    for name in app_name_candidates {
        let app_id = format!("{}\\Application", name);

        if let Ok(app_application_regkey) = hkcr.open_subkey(app_id) {
            if let (Ok(app_name), Ok(app_exe)) = (app_application_regkey.get_value::<String, String>("ApplicationName".to_string()),
                                                  app_application_regkey.get_value::<String, String>("ApplicationIcon".to_string())) {
                if !app_exe.starts_with("@") {
                    ret.insert(app_name, app_exe);
                }
            }
        }
    }

    ret
}

#[cfg(target_os="macos")]
pub fn list_apps_for_pdfs() -> HashMap<String, String> {
    use std::ffi::{ CStr, CString };
    use core_foundation::string::{CFStringCreateWithCString, CFStringGetCStringPtr, kCFStringEncodingUTF8, CFStringRef};
    use core_services::{LSCopyAllRoleHandlersForContentType, LSCopyApplicationURLsForBundleIdentifier, kLSRolesAll};
    use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex};
    use core_foundation::url::{CFURLRef, CFURLCopyPath};
    use percent_encoding::percent_decode;

    let content_type = "com.adobe.pdf";
    let mut ret = HashMap::new();

    unsafe {
        if let Ok(c_key) = CString::new(content_type) {
            let cf_key = CFStringCreateWithCString(std::ptr::null(), c_key.as_ptr(), kCFStringEncodingUTF8);
            let result = LSCopyAllRoleHandlersForContentType(cf_key, kLSRolesAll);
            let count =  CFArrayGetCount(result);

            for i in 0..count-1 {
                let bundle_id = CFArrayGetValueAtIndex(result, i) as CFStringRef;
                let err_ref = std::ptr::null_mut();
                let apps = LSCopyApplicationURLsForBundleIdentifier(bundle_id, err_ref);

                if err_ref == std::ptr::null_mut() {
                    let app_count = CFArrayGetCount(apps);

                    for j in 0..app_count {
                        let cf_ref = CFArrayGetValueAtIndex(apps, j) as CFURLRef;
                        let cf_path = CFURLCopyPath(cf_ref);
                        let cf_ptr = CFStringGetCStringPtr(cf_path, kCFStringEncodingUTF8);
                        let c_str = CStr::from_ptr(cf_ptr);

                        if let Ok(app_url) = c_str.to_str() {
                            let app_url_path = PathBuf::from(app_url);
                            let basename_path = &app_url_path.file_stem();

                            if let Some(basename_ostr) = basename_path {
                                if let Some(basename) = basename_ostr.to_str() {
                                    if let (Ok(r_app_name), Ok(r_app_url)) = (percent_decode(basename.as_bytes()).decode_utf8(),
                                                                              percent_decode(app_url.as_bytes()).decode_utf8()) {
                                        ret.insert(r_app_name.to_string(), r_app_url.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        ret
    }
}
