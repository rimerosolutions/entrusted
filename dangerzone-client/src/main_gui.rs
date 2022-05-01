#![windows_subsystem = "windows"]

use std::thread;
use std::error::Error;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::cell::RefCell;
use std::cmp;
use std::process::Command;
use fltk::{
    app, button, prelude::*, group, window, dialog, input, misc, enums, text, browser, frame
};

mod common;
mod container;

pub struct ConversionParams {
    pub path_in: PathBuf,
    pub path_out: PathBuf,
    pub ocr_lang: Option<String>,
    pub oci_image: String,
}

pub fn show_progress_dialog(x: i32, y: i32, w: i32, h: i32, conversion_params: ConversionParams, open_pdf_after_conversion: bool) -> bool {
    let dw = 400;
    let dh = 400;
    let dx = cmp::max(0, (x + w / 2) - (dw / 2));
    let dy = cmp::max(0, (y + h / 2) - (dh / 2));

    do_show_progress_dialog(dx, dy, dw, dh, conversion_params, open_pdf_after_conversion)
}

fn do_show_progress_dialog(x: i32, y: i32, w: i32, h: i32, conversion_params: ConversionParams, open_pdf_after_conversion: bool) -> bool {
    let mut win = window::Window::new(x, y, w, h, "Progress dialog");
    win.set_color(enums::Color::from_rgb(240, 240, 240));
    win.make_resizable(true);

    let mut pack = group::Pack::default()
        .with_size(380, 360)
        .center_of_parent()
        .with_type(group::PackType::Vertical);
    pack.set_spacing(20);
    let mut textdisplay_cmdlog = text::TextDisplay::default_fill().with_label("Conversion output").with_size(340, 320);
    let text_buffer = text::TextBuffer::default();
    let mut text_buffer_copy = text_buffer.clone();
    textdisplay_cmdlog.set_buffer(text_buffer);
    textdisplay_cmdlog.deactivate();
    let mut textdisplay_cmdlog_copy = textdisplay_cmdlog.clone();

    let mut button_ok = button::Button::default().with_size(80, 20).with_label("Converting...");
    let mut button_ok_copy = button_ok.clone();
    pack.end();
    win.end();
    win.make_modal(true);
    win.show();
    button_ok.deactivate();
    button_ok.set_callback({
        let mut win = win.clone();
        move |_| {
            win.hide();
        }
    });

    win.set_callback({
        move |win_instance| {
            if button_ok.active() {
                win_instance.hide();
            }
        }
    });

    let (tx, rx) = mpsc::channel();

    let mut exec_handle = Some(thread::spawn(move || {
        match container::convert(conversion_params.path_in, conversion_params.path_out, Some(conversion_params.oci_image), conversion_params.ocr_lang, tx) {
            Ok(_) => None,
            Err(ex) => Some(format!("{}", ex))
        }
    }));

    let result = Arc::new(AtomicBool::new(false));

    while win.shown() {
        app::wait();

        if !button_ok_copy.active() {
            if let Ok(raw_msg) = rx.recv() {
                let msg = format!("{}\n", raw_msg);
                text_buffer_copy.append(msg.as_str());
                textdisplay_cmdlog_copy.scroll(text_buffer_copy.count_lines(0, text_buffer_copy.length()), 0);
                app::awake();
            } else {
                let mut cmdlog_label_color = enums::Color::Red;
                let mut cmdlog_label_text = "Conversion output (Failure)";
                let mut button_ok_label_text = "Close window";

                match exec_handle.take().map(thread::JoinHandle::join) {
                    Some(exec_handle_result) => {
                        match exec_handle_result {
                            Ok(None) => {
                                result.swap(true, Ordering::Relaxed);
                                cmdlog_label_color = enums::Color::DarkGreen;
                                cmdlog_label_text = "Conversion output (Success)";

                                if open_pdf_after_conversion {
                                    button_ok_label_text = "Open safe PDF";
                                }
                            },
                            Ok(err_string_opt) => {
                                if let Some(err_text) = err_string_opt {
                                    text_buffer_copy.append(err_text.as_str());
                                    textdisplay_cmdlog_copy.scroll(text_buffer_copy.count_lines(0, text_buffer_copy.length()), 0);
                                }
                            },
                            Err(ex) => {
                                let err_message = format!("{:?}", ex);
                                text_buffer_copy.append(err_message.as_str());
                                textdisplay_cmdlog_copy.scroll(text_buffer_copy.count_lines(0, text_buffer_copy.length()), 0);
                            }
                        }
                    },
                    None => {
                        cmdlog_label_color = enums::Color::Red;
                    }
                }

                button_ok_copy.set_label(button_ok_label_text);
                textdisplay_cmdlog_copy.set_label(cmdlog_label_text);
                button_ok_copy.activate();
                textdisplay_cmdlog_copy.activate();
                textdisplay_cmdlog_copy.set_label_color(cmdlog_label_color);
                app::awake();
            }
        }
    }

    result.load(Ordering::Relaxed)
}

fn main() -> Result<(), Box<dyn Error>> {
    let app = app::App::default().with_scheme(app::Scheme::Gleam);

    let wind_title = format!(
        "{} {}",
        option_env!("CARGO_PKG_NAME").unwrap_or("Unknown"),
        option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown")
    );
    
    let mut wind = window::Window::default()
        .with_size(600, 360)
        .center_screen()
        .with_label(&wind_title);
    let wind_clone = wind.clone();
    wind.make_resizable(true);

    let size_pack_spacing = 10;

    let mut input_row = group::Pack::default().with_pos(20, 30)
        .with_size(570, 30)
        .with_type(group::PackType::Horizontal);
    input_row.set_spacing(size_pack_spacing);
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
        .below_of(&input_row, size_pack_spacing)
        .with_type(group::PackType::Horizontal);
    row_inputloc.set_spacing(size_pack_spacing);
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

    let mut row_openwith = group::Pack::default().with_size(570, 40).below_of(&row_inputloc, size_pack_spacing);
    row_openwith.set_type(group::PackType::Horizontal);
    row_openwith.set_spacing(size_pack_spacing);
    let mut checkbutton_openwith = button::CheckButton::default().with_size(295, 20).with_label("Open safe document after converting, using");

    let pdf_apps_by_name = list_apps_for_pdfs();
    let pdf_viewer_list = Rc::new(RefCell::new(misc::InputChoice::default().with_size(200, 20)));
    let pdf_viewer_list_copy = pdf_viewer_list.clone();
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
    }

    pdf_viewer_list.borrow_mut().deactivate();

    let button_browse_for_pdf_app = Rc::new(RefCell::new(button::Button::default().with_size(35, 20).with_label("..")));
    let button_browse_for_pdf_app_copy = button_browse_for_pdf_app.clone();
    button_browse_for_pdf_app.borrow_mut().set_tooltip("Browse for PDF viewer program");
    button_browse_for_pdf_app.borrow_mut().deactivate();

    checkbutton_openwith.set_callback({
        move |b| {
            let will_be_read_only = !b.is_checked();
            pdf_viewer_list.borrow_mut().input().set_readonly(will_be_read_only);

            if will_be_read_only {
                pdf_viewer_list.borrow_mut().deactivate();
                button_browse_for_pdf_app_copy.borrow_mut().deactivate();
            } else {
                pdf_viewer_list.borrow_mut().activate();
                button_browse_for_pdf_app_copy.borrow_mut().activate();
            };
        }
    });

    button_browse_for_pdf_app.borrow_mut().set_callback({
        move |_| {
            let mut dlg = dialog::FileDialog::new(dialog::FileDialogType::BrowseFile);
            dlg.set_title("Select PDF viewer program");
            dlg.show();

            let selected_filename = dlg.filename();

            if !selected_filename.as_os_str().is_empty() {
                let path_name = format!("{}", dlg.filename().display());
                let path_str = path_name.as_str();
                pdf_viewer_list_copy.borrow_mut().set_value(path_str);
            }
        }
    });


    row_openwith.end();

    let mut row_ocr_language = group::Pack::default().with_size(570, 60).below_of(&row_openwith, size_pack_spacing);
    row_ocr_language.set_type(group::PackType::Horizontal);
    row_ocr_language.set_spacing(size_pack_spacing);
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

    let mut row_oci_image = group::Pack::default().with_size(550, 40).below_of(&row_ocr_language, size_pack_spacing);
    row_oci_image.set_type(group::PackType::Horizontal);
    row_oci_image.set_spacing(size_pack_spacing);
    let mut output_oci_image = frame::Frame::default().with_size(100, 20).with_pos(0, 0);
    output_oci_image.set_label("OCI Image");
    let mut input_oci_image = input::Input::default().with_size(440, 20);
    input_oci_image.set_value(common::CONTAINER_IMAGE_NAME);
    row_oci_image.end();

    let mut row_convert_button = group::Pack::default().with_size(500, 40).below_of(&row_oci_image, size_pack_spacing);
    row_convert_button.set_type(group::PackType::Horizontal);
    row_convert_button.set_spacing(size_pack_spacing);
    let mut button_convert = button::Button::default().with_size(200, 20).with_label("Convert to Safe Document");

    button_convert.set_callback({
        move |b| {
            let str_inputloc = c_input_inputfile.borrow().value();
            let str_outputloc = ccc_input_outputloc.borrow().value();
            let ocr_language_list_ref = c_ocr_language_list.borrow_mut();

            if !str_inputloc.is_empty() {
                let mut bb = b.clone();

                // TODO the list of detected PDF viewers could be empty, does it matter here??
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

                if path_in.exists() && path_in.is_file() {
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

                    let oci_image = input_oci_image.value();
                    let conversion_params = ConversionParams {
                        path_in, path_out, ocr_lang, oci_image
                    };

                    if err_msg.is_empty() {
                        b.deactivate();
                        if show_progress_dialog(wind_clone.x(), wind_clone.y(), wind_clone.width(), wind_clone.height(), conversion_params, viewer_app_exec.is_some()) {
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
                } else {
                    dialog::alert(wind_clone.x(), wind_clone.y() + wind_clone.height() / 2, "The selected document apparently doesn't exists!");
                }
            } else {
                dialog::alert(wind_clone.x(), wind_clone.y() + wind_clone.height() / 2, "Please select a document to convert!");
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
                    
                    if let (Some(app_name), Some(cmd_name)) = (&desktop_entry_section.attr("Name"),
                                                               &desktop_entry_section.attr("TryExec").or(desktop_entry_section.attr("Exec"))) {
                        let cmd_name_sanitized = cmd_name.to_string().replace("%u", "").replace("%U", "").replace("%f", "").replace("%F", "");
                        result.insert(app_name.to_string(), cmd_name_sanitized);
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

// TODO windows support hasn't been tested that much...
#[cfg(target_os="windows")]
pub fn list_apps_for_pdfs() -> HashMap<String, String> {
    use winreg::RegKey;
    use winreg::enums::RegType;
    use winreg::enums::HKEY_CLASSES_ROOT;
    use std::collections::HashSet;
    let mut ret = HashMap::new();
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    let open_with_list_hkcr_res = hkcr.open_subkey(".pdf\\OpenWithProgids");

    fn friendly_app_name(regkey: &RegKey, name: String) -> String {
        let app_id= format!("{}\\Application", name);

        if let Ok(app_application_regkey) = regkey.open_subkey(app_id) {
            let app_result: std::io::Result<String> = app_application_regkey.get_value("ApplicationName");

            if let Ok(ret) =  app_result {
                return ret;
            }
        }

        name
    }

    if let Ok(open_with_list_hkcr) = open_with_list_hkcr_res {
        let mut candidates = HashSet::new();

        for (name, v) in open_with_list_hkcr.enum_values().map(|x| x.unwrap()) {
            if !name.is_empty() && v.vtype != RegType::REG_NONE {
                candidates.insert(name);
            }
        }

        for name in candidates.iter() {
            let app_id = format!("{}\\shell\\Open\\command", name);
            let new_name = friendly_app_name(&hkcr, name.clone());

            if let Ok(app_application_regkey) = hkcr.open_subkey(app_id) {
                for (_, value) in app_application_regkey.enum_values().map(|x| x.unwrap()) {
                    let human_value = format!("{}", value);
                    let human_val: Vec<&str> = human_value.split("\"").collect();

                    // "C:\ Program Files\Adobe\Acrobat DC\Acrobat\Acrobat.exe" "%1"
                    if human_val.len() > 3 {
                        let human_app_path_with_trailing_backlash = human_val[2];

                        if human_app_path_with_trailing_backlash.ends_with("\\") {
                            let path_len = human_app_path_with_trailing_backlash.len() - 1;
                            let updated_path = human_app_path_with_trailing_backlash[..path_len].to_string();

                            if PathBuf::from(&updated_path).exists() {
                                ret.insert(new_name.clone(), updated_path);
                            }
                        }

                    }
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
