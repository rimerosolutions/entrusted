#![windows_subsystem = "windows"]

use serde_json;
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use fltk::{
    app, browser, button, dialog, draw, enums, frame, group, input, misc, prelude::*, text, window, image
};

use entrusted_l10n as l10n;
mod common;
mod config;
mod container;

const WIDGET_GAP: i32 = 20;
const ELLIPSIS: &str  = "...";

const ICON_HELP_TEXT: &str = "?";
const ICON_SAVE: &[u8]     = include_bytes!("../../images/Save_icon.png");
const ICON_FRAME: &[u8]    = include_bytes!("../../images/Entrusted_icon.png");
const ICON_PASSWORD: &[u8] = include_bytes!("../../images/Password_icon.png");

const FILELIST_ROW_STATUS_PENDING    :&str = "Pending";
const FILELIST_ROW_STATUS_INPROGRESS :&str = "InProgress";
const FILELIST_ROW_STATUS_SUCCEEDED  :&str = "Succeeded";
const FILELIST_ROW_STATUS_FAILED     :&str = "Failed";

struct FileListWidgetEvent;

impl FileListWidgetEvent {
    const SELECTION_CHANGED: i32 = 50;
    const ALL_SELECTED: i32      = 51;
    const ALL_DESELECTED: i32    = 52;
}

#[derive(Clone)]
struct FileListRow {
    file: PathBuf,
    opt_output_file: Rc<RefCell<Option<String>>>,
    password_button: button::Button,
    output_file_button: button::Button,
    checkbox: button::CheckButton,
    progressbar: misc::Progress,
    status: frame::Frame,
    log_link: button::Button,
    logs: Rc<RefCell<Vec<String>>>,
    opt_passwd: Rc<RefCell<Option<String>>>,
}

impl FileListRow {
    pub fn reset_ui_state(&mut self) {
        self.status.set_label(FILELIST_ROW_STATUS_PENDING);
        self.status.set_label_color(enums::Color::Magenta);

        self.progressbar.set_label("0%");
        self.progressbar.set_value(0.0);
        self.progressbar.redraw();

        self.log_link.set_label("    ");
        self.log_link.set_frame(enums::FrameType::NoBox);
        self.log_link.set_down_frame(enums::FrameType::NoBox);
        self.log_link.deactivate();
        self.log_link.redraw();

        self.logs.borrow_mut().clear();
    }
}

#[derive(Clone)]
struct FileListWidget {
    container: group::Pack,
    selected_indices: Rc<RefCell<Vec<usize>>>,
    rows: Rc<RefCell<Vec<FileListRow>>>,
    trans: l10n::Translations
}

impl Deref for FileListWidget {
    type Target = group::Pack;

    fn deref(&self) -> &Self::Target {
        &self.container
    }
}

impl  DerefMut for FileListWidget {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.container
    }
}

fn clip_text<S: Into<String>>(txt: S, max_width: i32) -> String {
    let text = txt.into();
    let (width, _) = draw::measure(&text, true);

    if width > max_width {
        let (mut total_width, _) = draw::measure(ELLIPSIS, true);
        let mut tmp = [0u8; 4];
        let mut ret = String::with_capacity(text.len());

        for ch in text.chars() {
            tmp.fill(0);
            let ch_str = ch.encode_utf8(&mut tmp);
            let (ch_w, _) = draw::measure(ch_str, true);
            ret.push(ch);
            total_width += ch_w;

            if total_width > max_width {
                ret.push_str(ELLIPSIS);
                return ret;
            }
        }
    }

    text
}

fn show_info_dialog(parent_window_bounds: (i32, i32, i32, i32), trans: l10n::Translations) {
    let wind_w = 450;
    let wind_h = 300;
    let wind_x = parent_window_bounds.0 + (parent_window_bounds.2 / 2) - (wind_w / 2);
    let wind_y = parent_window_bounds.1 + (parent_window_bounds.3 / 2) - (wind_h / 2);

    let mut win = window::Window::default()
        .with_size(wind_w, wind_h)
        .with_pos(wind_x, wind_y)
        .with_label(&trans.gettext("Help"));

    win.begin();
    win.make_resizable(true);

    let mut grp = group::Pack::default()
        .with_pos(WIDGET_GAP, 10)
        .with_size(wind_w - (WIDGET_GAP * 2), wind_h - (WIDGET_GAP * 2))
        .center_of(&win)
        .with_type(group::PackType::Vertical);
    grp.set_spacing(5);

    let label_container_solution = format!("{}{}{}{}",
                                           "The program requires a container solution: \n",
                                           "- Docker (Windows, Linux, Mac OS)\n",
                                           "- Podman (Linux)\n",
                                           "- Lima (Mac OS)");

    frame::Frame::default()
        .with_size(350, 80)
        .center_of(&win)
        .with_label(&trans.gettext(&label_container_solution))
        .with_align(enums::Align::Inside | enums::Align::Left);

    let label_supported_docs = format!("Supported document types: \n- {}\n- {}\n- {}\n- {}\n- {}",
                                       "Images (.jpg, .jpeg, .gif, .png, .tif, .tiff)",
                                       "Document Graphics (.odg)",
                                       "Text Documents (.rtf, .doc, .docx, .odt)",
                                       "Spreadsheets (.xls, .xlsx)",
                                       "Presentations (.ppt, .pptx, .odp)");


    frame::Frame::default()
        .with_size(350, 130)
        .with_label(&trans.gettext(&label_supported_docs))
        .with_align(enums::Align::Inside | enums::Align::Left);

    let label_website = "https://github.com/rimerosolutions/entrusted";

    frame::Frame::default()
        .with_size(350, 30)
        .with_label(&trans.gettext_fmt("For more information, please visit:\n{0}", vec![&label_website]))
        .with_align(enums::Align::Inside | enums::Align::Left);

    grp.end();

    win.show();
    while win.shown() {
        app::wait();
    }
}

fn filelist_column_widths(w: i32) -> (i32, i32, i32, i32, i32, i32) {
    let width_password    = 40;
    let width_output_file = 40;
    let width_checkbox    = (w as f64 * 0.35) as i32;
    let width_progressbar = (w as f64 * 0.15) as i32;
    let width_status      = (w as f64 * 0.15) as i32;
    let width_logs        = (w as f64 * 0.2)  as i32;

    (width_output_file, width_password, width_checkbox, width_progressbar, width_status, width_logs)
}

impl <'a> FileListWidget {
    pub fn new(translations: l10n::Translations) -> Self {
        let mut container = group::Pack::default().with_type(group::PackType::Vertical).with_size(300, 300);
        container.set_spacing(WIDGET_GAP);
        container.end();
        container.auto_layout();

        Self {
            container,
            trans: translations,
            selected_indices: Rc::new(RefCell::new(vec![])),
            rows: Rc::new(RefCell::new(vec![])),
        }
    }

    pub fn resize(&mut self, x: i32, y: i32, w: i32, _: i32) {
        self.container.resize(x, y, w, self.container.h());
        let (width_password, width_output_file, width_checkbox, width_progressbar, width_status, width_logs) = filelist_column_widths(w);
        let col_widths =[
            width_password, width_output_file, width_checkbox, width_progressbar, width_status, width_logs
        ];

        if let Ok(rows) = self.rows.try_borrow() {
            for row in rows.iter() {
                let mut active_row = row.clone();

                let mut col_widgets: Vec<Box<dyn WidgetExt>> = vec![
                    Box::new(row.password_button.clone()),
                    Box::new(row.output_file_button.clone()),
                    Box::new(row.checkbox.clone()),
                    Box::new(row.progressbar.clone()),
                    Box::new(row.status.clone()),
                    Box::new(row.log_link.clone())
                ];

                let mut xx = x;

                for i in 0..col_widths.len() {
                    let wid = &mut col_widgets[i];
                    wid.resize(xx, wid.y(), col_widths[i], wid.h());
                    xx = xx + col_widths[i] + WIDGET_GAP;

                    if i == 2 {
                        if let Some(path_name) = row.file.file_name().and_then(|x| x.to_str()) {
                            wid.set_label(&clip_text(path_name, width_checkbox));
                        }
                    }
                }

                active_row.log_link.resize(active_row.log_link.x(), active_row.log_link.x(), 60, active_row.log_link.h());
            }
        }
    }

    pub fn contains_path(&self, p: &PathBuf) -> bool {
        self.rows
            .borrow()
            .iter()
            .find(|row| *row.file == *p)
            .is_some()
    }

    pub fn has_files(&self) -> bool {
        !self.rows.borrow().is_empty()
    }

    fn toggle_selection(&mut self, select: bool) -> bool {
        let mut selection_changed = false;

        for row in self.rows.borrow().iter() {
            if row.checkbox.active() {
                if row.checkbox.is_checked() != select {
                    row.checkbox.set_checked(select);
                    selection_changed = true;
                }
            }
        }

        selection_changed
    }

    pub fn selected_indices(&self) -> Vec<usize> {
        self.selected_indices
            .borrow()
            .iter()
            .map(|i| i.clone())
            .collect()
    }

    pub fn select_all(&mut self) {
        if self.toggle_selection(true) {
            let row_count = self.rows.borrow().len();
            self.selected_indices.borrow_mut().extend(0..row_count);
            let _ = app::handle_main(FileListWidgetEvent::SELECTION_CHANGED);
        }
    }

    pub fn deselect_all(&mut self) {
        if self.toggle_selection(false) {
            self.selected_indices.borrow_mut().clear();
            let _ = app::handle_main(FileListWidgetEvent::SELECTION_CHANGED);
        }
    }

    pub fn delete_all(&mut self) {
        self.selected_indices.borrow_mut().clear();

        while !self.rows.borrow().is_empty() {
            if let Some(row) = self.rows.borrow_mut().pop() {
                if let Some(row_parent) = row.checkbox.parent() {
                    self.container.remove(&row_parent);
                }
            }
        }

        self.container.redraw();

        if let Some(container_parent) = self.container.parent() {
            let mut container_parent = container_parent;
            container_parent.resize(container_parent.x(), container_parent.y(), container_parent.w(), container_parent.h());
            container_parent.redraw();
        }

        let _ = app::handle_main(FileListWidgetEvent::SELECTION_CHANGED);
    }

    pub fn delete_selection(&mut self) {
        self.selected_indices.borrow_mut().sort_by(|a, b| a.cmp(b));

        while !self.selected_indices.borrow().is_empty() {
            if let Some(idx) = self.selected_indices.borrow_mut().pop() {
                let row = self.rows.borrow_mut().remove(idx);
                if let Some(row_parent) = row.checkbox.parent() {
                    self.container.remove(&row_parent);
                }
            }
        }

        self.container.redraw();

        if let Some(container_parent) = self.container.parent() {
            let mut container_parent = container_parent;
            container_parent.resize(container_parent.x(), container_parent.y(), container_parent.w(), container_parent.h());
            container_parent.redraw();
        }

        let _ = app::handle_main(FileListWidgetEvent::SELECTION_CHANGED);
    }

    pub fn add_file(&mut self, path: PathBuf) {
        let trans = &self.trans;

        let ww = self.container.w();

        let (width_password, width_output_file, width_checkbox, width_progressbar, width_status, width_logs) = filelist_column_widths(ww);

        let mut row = group::Pack::default()
            .with_type(group::PackType::Horizontal)
            .with_size(ww, 40);
        row.set_spacing(WIDGET_GAP);

        let row_height: i32 = 30;

        let mut password_frame =  button::Button::default().with_size(width_password, row_height);
        if let Ok(mut img) = image::PngImage::from_data(ICON_PASSWORD) {
            img.scale(width_password - 2, row_height - 2, true, true);
            password_frame.set_image(Some(img));
        }
        password_frame.set_color(enums::Color::White);
        password_frame.set_label_color(enums::Color::Red);
        password_frame.set_tooltip(&trans.gettext("Set document password (empty for none)"));

        let mut output_file_button = button::Button::default()
            .with_size(width_output_file, row_height);
        output_file_button.set_tooltip(&trans.gettext("Custom output file"));
        if let Ok(mut img) = image::PngImage::from_data(ICON_SAVE) {
            img.scale(width_output_file- 2, row_height - 2, true, true);
            output_file_button.set_image(Some(img));
        }

        let path_name = format!("{}", path.file_name().and_then(|x| x.to_str()).unwrap());
        let path_tooltip = path.display().to_string();
        let mut selectrow_checkbutton = button::CheckButton::default()
            .with_size(width_checkbox, row_height)
            .with_label(&clip_text(path_name, width_checkbox));
        selectrow_checkbutton.set_tooltip(&path_tooltip);

        let check_buttonx2 = selectrow_checkbutton.clone();
        let progressbar = misc::Progress::default().with_size(width_progressbar, 20).with_label("0%");

        let mut status_frame = frame::Frame::default()
            .with_size(width_status, row_height)
            .with_label(FILELIST_ROW_STATUS_PENDING)
            .with_align(enums::Align::Inside | enums::Align::Left);
        status_frame.set_label_color(enums::Color::Magenta);

        let mut logs_button = button::Button::default()
            .with_size(width_logs, row_height)
            .with_label("   ");
        logs_button.set_frame(enums::FrameType::NoBox);
        logs_button.set_down_frame(enums::FrameType::NoBox);
        logs_button.deactivate();
        logs_button.set_label_color(enums::Color::Blue);

        row.end();

        let file_list_row = FileListRow {
            password_button: password_frame.clone(),
            checkbox: check_buttonx2,
            progressbar,
            status: status_frame,
            log_link: logs_button.clone(),
            logs: Rc::new(RefCell::new(vec![])),
            file: path.clone(),
            output_file_button: output_file_button.clone(),
            opt_output_file: Rc::new(RefCell::new(None)),
            opt_passwd: Rc::new(RefCell::new(None))
        };

        output_file_button.set_callback({
            let opt_output_file = file_list_row.opt_output_file.clone();
            let trans = trans.clone();

            move|_| {
                if let Some(current_wind) = app::first_window() {
                    let dialog_width  = 450;
                    let dialog_height = 200;
                    let dialog_xpos   = current_wind.x() + (current_wind.w() / 2) - (dialog_width  / 2);
                    let dialog_ypos   = current_wind.y() + (current_wind.h() / 2) - (dialog_height / 2);
                    let (button_width, button_height) = (100, 40);

                    let mut win = window::Window::default()
                        .with_size(dialog_width, dialog_height)
                        .with_pos(dialog_xpos, dialog_ypos)
                        .with_label(&trans.gettext("Custom output file"));

                    let mut container_pack = group::Pack::default()
                        .with_pos(WIDGET_GAP, WIDGET_GAP)
                        .with_type(group::PackType::Vertical)
                        .with_size(dialog_width - (WIDGET_GAP * 2), dialog_height - (WIDGET_GAP * 2));
                    container_pack.set_spacing(WIDGET_GAP);

                    let mut buttonsfile_pack = group::Pack::default()
                        .with_type(group::PackType::Horizontal)
                        .with_size(dialog_width - (WIDGET_GAP * 2), button_height);
                    buttonsfile_pack.set_spacing(WIDGET_GAP);

                    let outputfile_input_rc = Rc::new(RefCell::new(input::Input::default().with_size(290, button_height)));
                    outputfile_input_rc.borrow_mut().set_tooltip(&trans.gettext("Optional output filename defaulting to <filename>-entrusted.pdf."));
                    if let Some(v) = opt_output_file.borrow_mut().take() {
                        outputfile_input_rc.borrow_mut().set_value(&v);
                    }

                    let mut select_button = button::Button::default()
                        .with_size(button_width, button_height)
                        .with_label(&trans.gettext("Select"));
                    buttonsfile_pack.end();

                    let mut buttons_pack = group::Pack::default()
                        .with_type(group::PackType::Horizontal)
                        .with_size(dialog_width - (WIDGET_GAP * 2), button_height);
                    buttons_pack.set_spacing(WIDGET_GAP);

                    let mut reset_button = button::Button::default()
                        .with_size(button_width, button_height)
                        .with_label(&trans.gettext("Reset"));

                    let mut accept_button = button::Button::default()
                        .with_size(button_width, button_height)
                        .with_label(&trans.gettext("Accept"));

                    let mut cancel_button = button::Button::default()
                        .with_size(button_width, button_height)
                        .with_label(&trans.gettext("Cancel"));

                    let select_pdffile_msg = trans.gettext("Custom output file");

                    select_button.set_callback({
                        let outputfile_input_rc = outputfile_input_rc.clone();
                        let select_pdffile_msg = select_pdffile_msg.clone();

                        move |_| {
                            let mut dlg = dialog::FileDialog::new(dialog::FileDialogType::BrowseSaveFile);
                            dlg.set_title(&select_pdffile_msg);
                            dlg.show();
                            let selected_filename = dlg.filename();

                            if !selected_filename.as_os_str().is_empty() {
                                let path_name = dlg.filename().display().to_string();
                                outputfile_input_rc.borrow_mut().set_value(&path_name);
                            }
                        }
                    });

                    reset_button.set_callback({
                        let mut win = win.clone();
                        let opt_output_file = opt_output_file.clone();

                        move |_| {
                            opt_output_file.replace(None);
                            win.hide();
                        }
                    });

                    accept_button.set_callback({
                        let mut win = win.clone();
                        let outputfile_input_rc = outputfile_input_rc.clone();
                        let opt_output_file = opt_output_file.clone();

                        move |_| {
                            let output_filename = outputfile_input_rc.borrow().value();
                            if output_filename.trim().len() != 0 {
                                opt_output_file.replace(Some(output_filename));
                            }
                            win.hide();
                        }
                    });

                    cancel_button.set_callback({
                        let mut win = win.clone();

                        move |_| {
                            win.hide();
                        }
                    });

                    buttons_pack.end();
                    container_pack.end();

                    win.end();
                    win.make_modal(true);
                    win.make_resizable(true);
                    win.show();

                    while win.shown() {
                        app::wait();
                    }
                }
            }
        });

        output_file_button.draw({
            let opt_current = file_list_row.opt_output_file.clone();

            move |wid| {
                if wid.active() {
                    let old_color = draw::get_color();
                    let current_color = if opt_current.borrow().is_some() {
                        enums::Color::DarkRed
                    } else {
                        enums::Color::Yellow
                    };
                    draw::set_draw_color(current_color);

                    let stroke = 2;
                    for i in 1..(stroke + 1) {
                        draw::draw_rect(wid.x() + i, wid.y() + i, wid.w() - i - i, wid.h() - i - i);
                    }

                    draw::set_draw_color(old_color);
                }
            }
        });

        let dialog_title = trans.gettext("Logs");
        let close_button_label = trans.gettext("Close");

        password_frame.draw({
            let opt_current = file_list_row.opt_passwd.clone();

            move |wid| {
                if wid.active() {
                    let old_color = draw::get_color();
                    let current_color = if opt_current.borrow().is_some() {
                        enums::Color::DarkRed
                    } else {
                        enums::Color::Yellow
                    };
                    draw::set_draw_color(current_color);

                    let stroke = 2;
                    for i in 1..(stroke + 1) {
                        draw::draw_rect(wid.x() + i, wid.y() + i, wid.w() - i - i, wid.h() - i - i);
                    }
                    draw::set_draw_color(old_color);
                }
            }
        });

        password_frame.set_callback({
            let active_row = file_list_row.clone();
            let trans = trans.clone();

            move |_| {
                if let Some(current_wind) = app::first_window() {
                    let dialog_width  = 350;
                    let dialog_height = 200;
                    let dialog_xpos   = current_wind.x() + (current_wind.w() / 2) - (dialog_width  / 2);
                    let dialog_ypos   = current_wind.y() + (current_wind.h() / 2) - (dialog_height / 2);

                    let (button_width, button_height) = (100, 40);

                    let mut win = window::Window::default()
                        .with_size(dialog_width, dialog_height)
                        .with_pos(dialog_xpos, dialog_ypos)
                        .with_label(&trans.gettext("Set document password"));

                    let mut container_pack = group::Pack::default()
                        .with_pos(WIDGET_GAP, WIDGET_GAP)
                        .with_type(group::PackType::Vertical)
                        .with_size(dialog_width - (WIDGET_GAP * 2), dialog_height - (WIDGET_GAP * 2));
                    container_pack.set_spacing(WIDGET_GAP);

                    let mut secret_input = input::SecretInput::default()
                        .with_size(dialog_width - WIDGET_GAP * 2, 40);
                    secret_input.set_tooltip(&trans.gettext("Set document password (empty for none)"));

                    let opt_current_password = active_row.opt_passwd.borrow().clone();
                    if let Some(current_password) = opt_current_password {
                        secret_input.set_value(&current_password);
                    }

                    let mut buttons_pack = group::Pack::default()
                        .with_size(dialog_width, WIDGET_GAP * 2)
                        .below_of(&secret_input, WIDGET_GAP)
                        .with_type(group::PackType::Horizontal)
                        .with_align(enums::Align::Inside | enums::Align::Right);
                    buttons_pack.set_spacing(WIDGET_GAP);

                    let mut ok_button = button::Button::default()
                        .with_size(button_width, button_height)
                        .with_label(&trans.gettext("Accept"));

                    let mut cancel_button = button::Button::default()
                        .with_size(button_width, button_height)
                        .with_label(&trans.gettext("Cancel"));

                    ok_button.set_callback({
                        let mut win = win.clone();
                        let secret_input = secret_input.clone();
                        let active_row = active_row.clone();

                        move |_| {
                            let input_value = secret_input.value();
                            let new_passwd = if !input_value.is_empty() {
                                Some(input_value.clone())
                            } else {
                                None
                            };
                            let mut passwd_holder = active_row.opt_passwd.borrow_mut();
                            let _ = std::mem::replace(&mut *passwd_holder, new_passwd);
                            win.hide();
                        }
                    });

                    cancel_button.set_callback({
                        let mut win = win.clone();

                        move |_| {
                            win.hide();
                        }
                    });

                    buttons_pack.end();
                    container_pack.end();

                    win.end();
                    win.make_modal(true);
                    win.make_resizable(true);
                    win.show();

                    while win.shown() {
                        app::wait();
                    }
                }
            }
        });
        logs_button.set_callback({
            let active_row = file_list_row.clone();

            move |_| {
                if let Some(top_level_wind) = app::first_window() {
                    let wind_w = 400;
                    let wind_h = 400;
                    let button_width = 50;
                    let button_height = 30;
                    let wind_x = top_level_wind.x() + (top_level_wind.w() / 2) - (wind_w / 2);
                    let wind_y = top_level_wind.y() + (top_level_wind.h() / 2) - (wind_h / 2);

                    let mut dialog = window::Window::default()
                        .with_size(wind_w, wind_h)
                        .with_pos(wind_x, wind_y)
                        .with_label(&dialog_title);

                    dialog.begin();

                    let mut textdisplay_cmdlog = text::TextDisplay::default()
                        .with_type(group::PackType::Vertical)
                        .with_size(wind_w, 350);
                    let mut text_buffer = text::TextBuffer::default();
                    let logs = active_row.logs.borrow().join("\n") + "\n";

                    let mut log_close_button = button::Button::default()
                        .with_pos((wind_w / 2) - (button_width / 2), 400 - button_height - (WIDGET_GAP / 2))
                        .with_size(button_width, button_height)
                        .with_label(&close_button_label);

                    log_close_button.set_callback({
                        let mut dialog_window = dialog.clone();
                        move |_| {
                            dialog_window.hide();
                            app::awake();
                        }
                    });

                    text_buffer.set_text(&logs);
                    textdisplay_cmdlog.set_buffer(text_buffer);

                    dialog.handle({
                        move |wid, ev| match ev {
                            enums::Event::Resize => {
                                let x = (wid.w() / 2) - (button_width / 2);
                                let y = wid.h() - button_height - (WIDGET_GAP / 2);
                                log_close_button.resize(x, y, button_width, button_height);
                                true
                            },
                            _ => false
                        }
                    });

                    dialog.end();
                    dialog.make_modal(true);
                    dialog.make_resizable(true);
                    dialog.show();

                    while dialog.shown() {
                        app::wait();
                    }
                }
            }
        });

        selectrow_checkbutton.set_callback({
            let selfie = self.clone();
            let current_path = path.clone();

            move |b| {
                let idx = selfie.row_index(&current_path);

                if idx != -1 {
                    if b.is_checked() {
                        selfie.selected_indices.borrow_mut().push(idx as usize);
                    } else {
                        selfie.selected_indices.borrow_mut().retain( |x| *x != idx as usize);
                    }

                    let _ = app::handle_main(FileListWidgetEvent::SELECTION_CHANGED);
                }
            }
        });

        self.container.add(&row);
        self.rows.borrow_mut().push(file_list_row);
        self.resize(self.container.x(), self.container.y(), ww, self.container.h());
    }

    fn row_index(&self, file: &PathBuf) -> i32 {
        if let Some(pos) = self.rows.borrow().iter().position(|r| r.file == *file) {
            pos as i32
        } else {
            -1
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    l10n::load_translations(incl_gettext_files!("en", "fr"));

    let locale = match env::var(l10n::ENV_VAR_ENTRUSTED_LANGID) {
        Ok(selected_locale) => selected_locale,
        Err(_) => l10n::sys_locale()
    };

    let trans = l10n::new_translations(locale);
    let trans_ref = trans.clone();

    let selectfiles_dialog_title = trans.gettext("Select 'potentially suspicious' file(s)").clone();
    let appconfig_ret = config::load_config();
    let appconfig = appconfig_ret.unwrap_or(config::AppConfig::default());

    let is_converting = Arc::new(AtomicBool::new(false));
    let app = app::App::default().with_scheme(app::Scheme::Gleam);
    let (_, r) = app::channel::<String>();

    let wind_title = format!(
        "{} {}",
        option_env!("CARGO_PKG_NAME").unwrap_or("Unknown"),
        option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown")
    );

    let mut wind = window::Window::default()
        .with_size(739, 630)
        .center_screen()
        .with_label(&wind_title);

    wind.set_xclass("entrusted");

    if let Ok(frame_icon) = image::PngImage::from_data(ICON_FRAME) {
        wind.set_icon(Some(frame_icon));
    }

    wind.make_resizable(true);

    let mut top_group = group::Pack::default()
        .with_pos(20, 20)
        .with_size(680, 25)
        .with_type(group::PackType::Horizontal)
        .with_align(enums::Align::Inside | enums::Align::Right);

    top_group.set_spacing(WIDGET_GAP);

    let mut tabsettings_button = button::Button::default()
        .with_size(120, 20)
        .with_label(&trans.gettext("Settings"));

    let mut tabconvert_button = button::Button::default()
        .with_size(120, 20)
        .with_label(&trans.gettext("Convert"));

    let mut helpinfo_frame = frame::Frame::default_fill();

    helpinfo_frame.draw({
        move |wid| {
            let (w, h) = draw::measure(ICON_HELP_TEXT, true);
            let widx = wid.x() + wid.w() - WIDGET_GAP;
            let widy = wid.y() + 2;
            let old_color = draw::get_color();
            draw::draw_rect_fill(widx, widy, WIDGET_GAP, WIDGET_GAP, enums::Color::Blue);
            draw::set_draw_color(enums::Color::White);
            draw::draw_text(ICON_HELP_TEXT, (widx + WIDGET_GAP/2) - w/2, wid.y() + h);
            draw::set_draw_color(old_color);
        }
    });

    helpinfo_frame.handle({
        let wind = wind.clone();
        let trans_ref = trans_ref.clone();

        move |wid, ev| match ev {
            enums::Event::Push => {
                let (x, y) = app::event_coords();
                let widx = wid.x() + wid.w() - WIDGET_GAP;
                let widy = wid.y() + 2;

                if x >= widx && x <= widx + WIDGET_GAP && y >= widy && y <= widy + WIDGET_GAP {
                    show_info_dialog((wind.x(), wind.y(), wind.w(), wind.h()), trans_ref.clone());
                }

                true
            }
            _ => false,
        }
    });

    top_group.end();

    let settings_pack_rc = Rc::new(RefCell::new(
        group::Pack::default()
            .with_pos(20, 20)
            .with_size(600, 580)
            .below_of(&top_group, WIDGET_GAP)
            .with_type(group::PackType::Vertical),
    ));
    settings_pack_rc.borrow_mut().set_spacing(WIDGET_GAP);

    let mut filesuffix_pack = group::Pack::default()
        .with_size(570, 40)
        .with_type(group::PackType::Horizontal);

    filesuffix_pack.set_spacing(WIDGET_GAP);
    let mut filesuffix_checkbutton = button::CheckButton::default()
        .with_size(160, 20)
        .with_label(&trans.gettext("Custom file suffix"));
    filesuffix_checkbutton
        .set_tooltip(&trans.gettext("The safe PDF will be named <input>-<suffix>.pdf by default."));

    if &appconfig.file_suffix != config::DEFAULT_FILE_SUFFIX {
        filesuffix_checkbutton.set_checked(true);
    }

    let filesuffix_input_rc = Rc::new(RefCell::new(input::Input::default().with_size(290, 20)));
    filesuffix_input_rc.borrow_mut().set_value(&appconfig.file_suffix);

    if &appconfig.file_suffix == config::DEFAULT_FILE_SUFFIX {
        filesuffix_input_rc.borrow_mut().deactivate();
    }

    filesuffix_checkbutton.set_callback({
        let filesuffix_input_rc_ref = filesuffix_input_rc.clone();

        move|b| {
            if b.is_checked() {
                filesuffix_input_rc_ref.borrow_mut().activate();
            } else {
                filesuffix_input_rc_ref.borrow_mut().set_value(config::DEFAULT_FILE_SUFFIX);
                filesuffix_input_rc_ref.borrow_mut().deactivate();
            }
        }
    });

    filesuffix_pack.end();

    let mut ocrlang_pack = group::Pack::default()
        .with_size(570, 60)
        .below_of(&filesuffix_pack, WIDGET_GAP)
        .with_type(group::PackType::Horizontal);
    ocrlang_pack.set_spacing(WIDGET_GAP);
    let mut ocrlang_checkbutton = button::CheckButton::default()
        .with_size(300, 20)
        .with_label(&trans.gettext("Enable full-text search? In:"));
    ocrlang_checkbutton.set_tooltip(
        &trans.gettext("OCR (Optical character recognition) will be applied."),
    );

    if appconfig.ocr_lang.is_some() {
        ocrlang_checkbutton.set_checked(true);
    }

    let ocrlang_holdbrowser_rc = Rc::new(RefCell::new(
        browser::HoldBrowser::default().with_size(240, 60),
    ));
    let ocr_languages_by_name = l10n::ocr_lang_key_by_name(&trans_ref);
    let ocr_languages_by_name_ref = ocr_languages_by_name.clone();
    let mut ocr_languages_by_lang = HashMap::with_capacity(ocr_languages_by_name.len());
    let mut ocr_languages: Vec<String> = Vec::with_capacity(ocr_languages_by_name.len());

    for (k, v) in ocr_languages_by_name {
        ocr_languages_by_lang.insert(v.clone(), k);
        ocr_languages.push(v.clone());
    }

    ocr_languages.sort();

    for v in ocr_languages.iter() {
        ocrlang_holdbrowser_rc.borrow_mut().add(v);
    }

    let selected_ocrlang = if let Some(cur_ocrlangcode) = appconfig.ocr_lang.clone() {
        let cur_ocrlangcode_str = cur_ocrlangcode.as_str();

        if let Some(cur_ocrlangname) = ocr_languages_by_name_ref.get(cur_ocrlangcode_str) {
            cur_ocrlangname.to_string()
        } else {
            String::from(&trans.gettext("English"))
        }
    } else {
        String::from(&trans.gettext("English"))
    };

    if let Some(selected_ocr_language_idx) = ocr_languages.iter().position(|r| r == &selected_ocrlang) {
        ocrlang_holdbrowser_rc
            .borrow_mut()
            .select((selected_ocr_language_idx + 1) as i32);
    }

    if appconfig.ocr_lang.is_none() {
        ocrlang_holdbrowser_rc.borrow_mut().deactivate();
    }

    ocrlang_checkbutton.set_callback({
        let ocrlang_holdbrowser_rc_ref = ocrlang_holdbrowser_rc.clone();

        move |b| {
            if !b.is_checked() {
                ocrlang_holdbrowser_rc_ref.borrow_mut().deactivate();
            } else {
                ocrlang_holdbrowser_rc_ref.borrow_mut().activate();
            }
        }
    });
    ocrlang_pack.end();

    let mut openwith_pack = group::Pack::default()
        .with_size(570, 40)
        .with_type(group::PackType::Horizontal);
    openwith_pack.set_spacing(WIDGET_GAP);
    let mut openwith_checkbutton = button::CheckButton::default().with_size(295, 20).with_label(&trans.gettext("Open resulting PDF with"));
    openwith_checkbutton.set_tooltip(&trans.gettext("Automatically open resulting PDFs with a given program."));

    let pdf_apps_by_name = list_apps_for_pdfs();
    let openwith_inputchoice_rc = Rc::new(RefCell::new(misc::InputChoice::default().with_size(240, 20)));
    let mut pdf_viewer_app_names = Vec::with_capacity(pdf_apps_by_name.len());

    for (k, _v) in &pdf_apps_by_name {
        pdf_viewer_app_names.push(k.as_str());
    }

    pdf_viewer_app_names.sort();

    for k in pdf_viewer_app_names.iter() {
        openwith_inputchoice_rc.borrow_mut().add(k);
    }

    openwith_inputchoice_rc.borrow_mut().set_tooltip(&trans.gettext("You can also paste the path to a PDF viewer"));

    if pdf_apps_by_name.len() != 0 {
        let idx = if let Some(viewer_appname) = appconfig.openwith_appname.clone() {
            if let Some(pos) = pdf_viewer_app_names.iter().position(|r| r == &viewer_appname) {
                pos
            } else {
                0
            }
        } else {
            0
        };
        openwith_inputchoice_rc.borrow_mut().set_value_index(idx as i32);
    }

    if appconfig.openwith_appname.is_none() {
        openwith_inputchoice_rc.borrow_mut().deactivate();
    }

    let openwith_button_rc = Rc::new(RefCell::new(button::Button::default().with_size(35, 20).with_label("..")));
    let openwith_button_tooltip = trans.gettext("Browse for PDF viewer program");
    openwith_button_rc.borrow_mut().set_tooltip(&openwith_button_tooltip);

    openwith_button_rc.borrow_mut().deactivate();

    if appconfig.openwith_appname.is_some() {
        openwith_checkbutton.set_checked(true);
    }

    openwith_checkbutton.set_callback({
        let pdf_viewer_list_ref = openwith_inputchoice_rc.clone();
        let openwith_button_rc_ref = openwith_button_rc.clone();

        move |b| {
            let will_be_read_only = !b.is_checked();
            pdf_viewer_list_ref.borrow_mut().input().set_readonly(will_be_read_only);

            if will_be_read_only {
                pdf_viewer_list_ref.borrow_mut().deactivate();
                openwith_button_rc_ref.borrow_mut().deactivate();
            } else {
                pdf_viewer_list_ref.borrow_mut().activate();
                openwith_button_rc_ref.borrow_mut().activate();
            };
        }
    });

    openwith_button_rc.borrow_mut().set_callback({
        let pdf_viewer_list_ref = openwith_inputchoice_rc.clone();

        move |_| {
            let mut selectpdfviewer_dialog = dialog::FileDialog::new(dialog::FileDialogType::BrowseFile);
            selectpdfviewer_dialog.set_title(&openwith_button_tooltip);
            selectpdfviewer_dialog.show();

            let selected_filename = selectpdfviewer_dialog.filename();

            if !selected_filename.as_os_str().is_empty() {
                let path_name = selectpdfviewer_dialog.filename().display().to_string();
                pdf_viewer_list_ref.borrow_mut().set_value(&path_name);
            }
        }
    });

    openwith_pack.end();

    let mut ociimage_pack = group::Pack::default()
        .with_size(550, 40)
        .below_of(&ocrlang_pack, WIDGET_GAP);
    ociimage_pack.set_type(group::PackType::Horizontal);
    ociimage_pack.set_spacing(WIDGET_GAP);
    let mut ociimage_checkbutton = button::CheckButton::default()
        .with_size(100, 20)
        .with_pos(0, 0)
        .with_align(enums::Align::Inside | enums::Align::Left);
    ociimage_checkbutton.set_label(&trans.gettext("Custom container image"));
    ociimage_checkbutton.set_tooltip(&trans.gettext("Expert option for sandbox solution"));

    let ociimage_text = if let Some(custom_container_image_name) = appconfig.container_image_name.clone() {
        custom_container_image_name
    } else {
        config::default_container_image_name()
    };

    if ociimage_text != config::default_container_image_name() {
        ociimage_checkbutton.set_checked(true);
    }

    let ociimage_input_rc = Rc::new(RefCell::new(input::Input::default().with_size(440, 20)));
    ociimage_input_rc.borrow_mut().set_value(&ociimage_text);

    if appconfig.container_image_name.is_none() {
        ociimage_input_rc.borrow_mut().deactivate();
    } else if ociimage_text == config::default_container_image_name() {
        ociimage_input_rc.borrow_mut().deactivate();
    }

    ociimage_pack.end();

    ociimage_checkbutton.set_callback({
        let ociimage_input_rc_ref = ociimage_input_rc.clone();

        move|b| {
            if !b.is_checked() {
                ociimage_input_rc_ref.borrow_mut().deactivate();
                ociimage_input_rc_ref.borrow_mut().set_value(&config::default_container_image_name());
            } else {
                ociimage_input_rc_ref.borrow_mut().activate();
            }
        }
    });

    let savesettings_pack = group::Pack::default()
        .with_size(150, 30)
        .below_of(&ociimage_pack, WIDGET_GAP);
    ociimage_pack.set_type(group::PackType::Horizontal);
    ociimage_pack.set_spacing(WIDGET_GAP);

    let mut savesettings_button = button::Button::default()
        .with_size(100, 20)
        .with_label(&trans.gettext("Save current settings as defaults"))
        .with_align(enums::Align::Inside | enums::Align::Center);

    savesettings_button.set_callback({
        let ocrlang_checkbutton_ref = ocrlang_checkbutton.clone();
        let ocrlang_holdbrowser_rc_ref = ocrlang_holdbrowser_rc.clone();
        let filesuffix_input_rc_ref = filesuffix_input_rc.clone();
        let filesuffix_checkbutton_ref = filesuffix_checkbutton.clone();
        let openwith_checkbutton_ref = openwith_checkbutton.clone();
        let openwith_inputchoice_rc_ref = openwith_inputchoice_rc.clone();
        let ociimage_checkbutton_ref = ociimage_checkbutton.clone();
        let ociimage_input_rc_ref = ociimage_input_rc.clone();
        let ocr_languages_by_lang_ref = ocr_languages_by_lang.clone();
        let wind_ref = wind.clone();

        move|_| {
            let mut new_appconfig = config::AppConfig::default();

            if ocrlang_checkbutton_ref.is_checked() {
                if let Some(language_name) = ocrlang_holdbrowser_rc_ref.borrow().selected_text() {
                    if let Some(langcode) = ocr_languages_by_lang_ref.get(&language_name) {
                        new_appconfig.ocr_lang = Some(langcode.to_string());
                    }
                }
            }

            if ociimage_checkbutton_ref.is_checked() {
                let mut ociimage_text = ociimage_input_rc_ref.borrow().value();
                ociimage_text = ociimage_text.trim().to_string();
                if !ociimage_text.is_empty() && ociimage_text != config::default_container_image_name() {
                    new_appconfig.container_image_name = Some(ociimage_text.trim().to_string());
                }
            }

            if filesuffix_checkbutton_ref.is_checked() {
                let selected_filesuffix = filesuffix_input_rc_ref.borrow().value();

                if selected_filesuffix != String::from(config::DEFAULT_FILE_SUFFIX) {
                    new_appconfig.file_suffix = selected_filesuffix;
                }
            }

            if openwith_checkbutton_ref.is_checked() {
                new_appconfig.openwith_appname = openwith_inputchoice_rc_ref.borrow().value();
            }

            if let Err(ex) = config::save_config(new_appconfig) {
                let err_text = ex.to_string();
                dialog::alert(wind_ref.x(), wind_ref.y() + wind_ref.height() / 2, &err_text);
            }
        }
    });

    savesettings_pack.end();


    settings_pack_rc.borrow_mut().end();

    let convert_pack_rc = Rc::new(RefCell::new(
        group::Pack::default()
            .with_pos(20, 20)
            .with_size(680, 680)
            .below_of(&top_group, WIDGET_GAP)
            .with_type(group::PackType::Vertical),
    ));
    convert_pack_rc.borrow_mut().set_spacing(WIDGET_GAP);

    let mut convert_frame = frame::Frame::default().with_size(680, 80).with_pos(10, 10);
    convert_frame.set_frame(enums::FrameType::RFlatBox);
    convert_frame.set_label_color(enums::Color::White);
    convert_frame.set_label(&format!("{}\n{}\n{}",
                                     trans.gettext("Drop 'potentially suspicious' file(s) here"),
                                     trans.gettext("or"),
                                     trans.gettext("Click here to select file(s)")));
    convert_frame.set_color(enums::Color::Red);

    let mut row_convert_button = group::Pack::default()
        .with_size(wind.w(), 40)
        .below_of(&convert_frame, 30);
    row_convert_button.set_type(group::PackType::Horizontal);
    row_convert_button.set_spacing(WIDGET_GAP/2);

    let mut selection_pack = group::Pack::default()
        .with_size(150, 40)
        .with_type(group::PackType::Vertical)
        .below_of(&convert_frame, 30);
    selection_pack.set_spacing(5);

    let selectall_frame_rc = Rc::new(RefCell::new(
        frame::Frame::default()
            .with_size(150, 10)
            .with_label(&trans.gettext("Select all"))
            .with_align(enums::Align::Inside | enums::Align::Left),
    ));

    let deselectall_frame_rc = Rc::new(RefCell::new(
        frame::Frame::default()
            .with_size(150, 10)
            .with_label(&trans.gettext("Deselect all"))
            .with_align(enums::Align::Inside | enums::Align::Left),
    ));

    selectall_frame_rc
        .borrow_mut()
        .set_label_color(enums::Color::Blue);

    selectall_frame_rc.borrow_mut().draw({
        move |wid| {
            if wid.visible() && wid.active() {
                let (lw, _) = draw::measure(&wid.label(), true);
                let old_color = draw::get_color();
                draw::set_draw_color(wid.label_color());
                draw::draw_line(wid.x() + 3, wid.y() + wid.h(), wid.x() + lw, wid.y() + wid.h());
                draw::set_draw_color(old_color);
            }
        }
    });

    deselectall_frame_rc.borrow_mut().draw({
        move |wid| {
            if wid.visible() && wid.active() {
                let (lw, _) = draw::measure(&wid.label(), true);
                let old_color = draw::get_color();
                draw::set_draw_color(wid.label_color());
                draw::draw_line(wid.x() + 3, wid.y() + wid.h(), wid.x() + lw, wid.y() + wid.h());
                draw::set_draw_color(old_color);
            }
        }
    });

    deselectall_frame_rc
        .borrow_mut()
        .set_label_color(enums::Color::Blue);

    selectall_frame_rc.borrow_mut().handle({
        move |_, ev| match ev {
            enums::Event::Push => {
                let _ = app::handle_main(FileListWidgetEvent::ALL_SELECTED);
                true
            }
            _ => false,
        }
    });

    deselectall_frame_rc.borrow_mut().handle({
        move |_, ev| match ev {
            enums::Event::Push => {
                let _ = app::handle_main(FileListWidgetEvent::ALL_DESELECTED);
                true
            }
            _ => false,
        }
    });

    selectall_frame_rc.borrow_mut().hide();
    deselectall_frame_rc.borrow_mut().hide();

    selection_pack.end();

    let mut delete_button = button::Button::default()
        .with_size(260, 20)
        .with_label(&trans.gettext("Remove selected file(s)"));
    delete_button.set_label_color(enums::Color::Black);
    delete_button.set_color(enums::Color::White);
    delete_button.deactivate();

    let mut convert_button = button::Button::default()
        .with_size(260, 20)
        .with_label(&trans.gettext("Convert document(s)"));

    convert_button.set_label_color(enums::Color::Black);
    convert_button.set_color(enums::Color::White);
    convert_button.deactivate();

    row_convert_button.end();

    let mut columns_frame = frame::Frame::default().with_size(500, 40).with_pos(10, 10);
    columns_frame.set_frame(enums::FrameType::NoBox);

    let filelist_scroll = group::Scroll::default().with_size(580, 200);
    let mut filelist_widget = FileListWidget::new(trans.clone());

    let col_label_password   = String::new();
    let col_label_outputfile = String::new();
    let col_label_filename   = trans.gettext("File name");
    let col_label_progress   = trans.gettext("Progress(%)");
    let col_label_status     = trans.gettext("Status");
    let col_label_message    = trans.gettext("Message");

    columns_frame.draw({
        let filelist_widget_ref      = filelist_widget.clone();
        let col_label_password_ref   = col_label_password.to_owned();
        let col_label_outputfile_ref = col_label_outputfile.to_owned();
        let col_label_filename_ref   = col_label_filename.to_owned();
        let col_label_progress_ref   = col_label_progress.to_owned();
        let col_label_status_ref     = col_label_status.to_owned();
        let col_label_message_ref    = col_label_message.to_owned();

        move |wid| {
            let file_count = filelist_widget_ref.children();
            if file_count != 0 {
                let w = filelist_widget_ref.container.w();

                let (width_output_file, width_password, width_checkbox, width_progressbar, width_status, width_logs) = filelist_column_widths(w);
                let column_widths = [
                    width_output_file, width_password, width_checkbox, width_progressbar, width_status, width_logs
                ];

                let column_names = vec![
                    col_label_password_ref.clone(),
                    col_label_outputfile_ref.clone(),
                    format!("{} [{}]", &col_label_filename_ref, file_count),
                    col_label_progress_ref.clone(),
                    col_label_status_ref.clone(),
                    col_label_message_ref.clone()
                ];

                let old_color = draw::get_color();
                let old_font = draw::font();
                let old_font_size = app::font_size();

                draw::set_font(enums::Font::HelveticaBold, old_font_size);
                draw::set_draw_color(enums::Color::Black);

                let mut column_x = column_widths[0] + column_widths[1];
                let y = wid.y() + wid.h() / 2;

                for i in 2..column_names.len() {
                    column_x = column_x + WIDGET_GAP + column_widths[i - 1];
                    draw::draw_text(&column_names[i], column_x, y);
                }

                draw::set_draw_color(old_color);
                draw::set_font(old_font, old_font_size);
            }
        }
    });

    delete_button.set_callback({
        let mut filelist_widget_ref = filelist_widget.clone();
        let mut filelist_scroll_ref = filelist_scroll.clone();

        move |_| {
            filelist_widget_ref.delete_selection();
            filelist_scroll_ref.scroll_to(0, 0);
        }
    });

    filelist_scroll.end();

    let messages_frame = frame::Frame::default()
        .with_size(580, 80)
        .with_label(" ")
        .with_align(enums::Align::Left | enums::Align::Inside);

    convert_button.set_callback({
        let wind_ref = wind.clone();
        let filelist_widget_ref = filelist_widget.clone();
        let mut convert_frame_ref = convert_frame.clone();
        let mut messages_frame_ref = messages_frame.clone();
        let ocrlang_holdbrowser_rc_ref = ocrlang_holdbrowser_rc.clone();
        let mut tabsettings_button_ref =  tabsettings_button.clone();
        let ocrlang_checkbutton_ref = ocrlang_checkbutton.clone();
        let ociimage_input_rc_ref = ociimage_input_rc.clone();
        let pdf_viewer_list_ref = openwith_inputchoice_rc.clone();
        let filesuffix_input_rc_ref = filesuffix_input_rc.clone();
        let is_converting_ref = is_converting.clone();
        let openwith_checkbutton_ref = openwith_checkbutton.clone();
        let selectall_frame_rc_ref = selectall_frame_rc.clone();
        let deselectall_frame_rc_ref = deselectall_frame_rc.clone();
        let mut filelist_scroll_ref = filelist_scroll.clone();
        let trans_ref = trans_ref.clone();
        let app_config_ref = appconfig.clone();

        move |b| {
            tabsettings_button_ref.deactivate();
            selectall_frame_rc_ref.borrow_mut().deactivate();
            deselectall_frame_rc_ref.borrow_mut().deactivate();
            selectall_frame_rc_ref.borrow_mut().set_label_color(enums::Color::from_rgb(82, 82, 82));
            deselectall_frame_rc_ref.borrow_mut().set_label_color(enums::Color::from_rgb(82, 82, 82));
            convert_frame_ref.deactivate();

            is_converting_ref.store(true, Ordering::Relaxed);
            let file_suffix = filesuffix_input_rc_ref.borrow().value();
            let mut file_suffix = file_suffix.clone().trim().to_owned();

            if file_suffix.is_empty() {
                file_suffix = app_config_ref.file_suffix.to_owned();
            }

            let viewer_app_name = pdf_viewer_list_ref.borrow_mut().input().value();
            let viewer_app_exec = if openwith_checkbutton_ref.is_checked() {
                if let Some(viewer_app_path) = pdf_apps_by_name.get(&viewer_app_name) {
                    Some(viewer_app_path.clone())
                } else {
                    Some(viewer_app_name.trim().to_owned())
                }
            } else {
                None
            };

            b.deactivate();

            for current_row in filelist_widget_ref.rows.borrow_mut().iter() {
                let mut active_row = current_row.clone();
                active_row.reset_ui_state();
                active_row.password_button.deactivate();
                active_row.output_file_button.deactivate();
                active_row.checkbox.deactivate();
            }

            let ocr_lang_setting = if ocrlang_checkbutton_ref.is_checked() {
                if let Some(selected_lang) = ocrlang_holdbrowser_rc_ref.borrow().selected_text() {
                    ocr_languages_by_lang
                        .get(&selected_lang)
                        .map(|i| i.to_string())
                } else {
                    None
                }
            } else {
                None
            };

            let oci_image_text = ociimage_input_rc_ref.borrow().value();

            let ociimage_option  = if oci_image_text.trim().is_empty() {
                config::default_container_image_name()
            } else {
                String::from(oci_image_text.trim())
            };

            let viewer_app_option = viewer_app_exec.clone();
            let failure_message = &trans_ref.gettext("Conversion failed!");
            let logs_title_button_label = &trans_ref.gettext("Logs");

            filelist_scroll_ref.scroll_to(0, 0);
            filelist_scroll_ref.redraw();

            for current_row in filelist_widget_ref.rows.borrow_mut().iter() {
                let result = Arc::new(AtomicBool::new(false));
                let mut active_row = current_row.clone();
                let input_path = active_row.file.clone();
                let active_ocrlang_option = ocr_lang_setting.clone();
                let active_ociimage_option = ociimage_option.clone();
                let active_viewer_app_option = viewer_app_option.clone();
                let active_file_suffix = file_suffix.clone();

                filelist_scroll_ref.scroll_to(0, active_row.checkbox.y() - filelist_scroll_ref.y());
                filelist_scroll_ref.redraw();

                let (tx, rx) = mpsc::channel();

                let output_path = if let Some(custom_output_path) = active_row.opt_output_file.borrow().clone() {
                    PathBuf::from(custom_output_path)
                } else {
                    common::default_output_path(input_path.clone(), active_file_suffix.clone()).unwrap()
                };

                let current_input_path = input_path.clone();
                let current_output_path = output_path.clone();
                active_row.status.set_label(FILELIST_ROW_STATUS_INPROGRESS);
                active_row.checkbox.deactivate();
                active_row.status.set_label_color(enums::Color::DarkYellow);
                let trans_ref = trans_ref.clone();
                let opt_row_passwd = active_row.opt_passwd.clone();
                let opt_passwd = opt_row_passwd.borrow().clone();

                let mut exec_handle = Some(thread::spawn(move || {
                    let opt_passwd_value  = opt_passwd.to_owned();
                    let convert_options = common::ConvertOptions::new(
                        active_ociimage_option,
                        common::LOG_FORMAT_JSON.to_string(),
                        active_ocrlang_option,
                        opt_passwd_value);

                    match container::convert(
                        current_input_path.clone(),
                        output_path.clone(),
                        convert_options,
                        tx,
                        trans_ref
                    ) {
                        Ok(_) => None,
                        Err(ex) => Some(ex.to_string())
                    }
                }));

                while let Ok(raw_msg) = rx.recv() {
                    app::wait();

                    let log_msg_ret: serde_json::Result<common::LogMessage> =
                        serde_json::from_slice(raw_msg.as_bytes());

                    if let Ok(log_msg) = log_msg_ret {
                        let progress_text = format!("{} %", log_msg.percent_complete);
                        active_row.progressbar.set_label(&progress_text);
                        active_row.progressbar.set_value(log_msg.percent_complete as f64);
                        messages_frame_ref.set_label(&clip_text(&log_msg.data, messages_frame_ref.w()));
                        active_row.logs.borrow_mut().push(log_msg.data);
                        active_row.progressbar.parent().unwrap().redraw();
                    }

                    app::awake();
                }

                let mut status_color = enums::Color::Red;
                let mut row_status = FILELIST_ROW_STATUS_FAILED;

                match exec_handle.take().map(thread::JoinHandle::join) {
                    Some(exec_handle_result) => match exec_handle_result {
                        Ok(None) => {
                            result.swap(true, Ordering::Relaxed);
                            active_row.progressbar.set_label("100%");
                            active_row.progressbar.set_value(100.0);
                            status_color = enums::Color::DarkGreen;
                            row_status = FILELIST_ROW_STATUS_SUCCEEDED;
                        }
                        Ok(err_string_opt) => {
                            if let Some(err_text) = err_string_opt {
                                active_row.logs.borrow_mut().push(err_text.clone());
                                active_row.log_link.set_label(&err_text);
                            }
                        }
                        Err(ex) => {
                            let err_text = format!("{:?}", ex);
                            active_row.logs.borrow_mut().push(err_text.clone());
                            active_row.log_link.set_label(&err_text);
                        }
                    },
                    None => {
                        let label_text = failure_message;
                        active_row.log_link.set_label(label_text);
                        active_row.logs.borrow_mut().push(String::from(label_text));
                    }
                }

                active_row.status.set_label(row_status);
                active_row.status.set_label_color(status_color);
                active_row.progressbar.set_label("100%");
                active_row.progressbar.set_value(100.0);
                active_row.log_link.set_label(logs_title_button_label);
                active_row.log_link.set_frame(enums::FrameType::ThinUpBox);
                active_row.log_link.set_down_frame(enums::FrameType::ThinDownBox);
                active_row.log_link.activate();

                messages_frame_ref.set_label("");

                if result.load(Ordering::Relaxed) && active_viewer_app_option.is_some() {
                    if let Some(viewer_exe) = active_viewer_app_option {
                        if let Err(exe) = pdf_open_with(viewer_exe, current_output_path.clone()) {
                            let err_text = format!("{}\n{}.", trans.gettext("Could not open PDF result!"), exe.to_string());
                            dialog::alert(wind_ref.x(), wind_ref.y() + wind_ref.height() / 2, &err_text);
                        }
                    }
                }

                app::sleep(0.05);
            }

            tabsettings_button_ref.activate();
            convert_frame_ref.activate();

            filelist_scroll_ref.scroll_to(0, 0);
            filelist_scroll_ref.redraw();
        }
    });

    #[cfg(target_os = "macos")] {
        use fltk::menu;

        app::raw_open_callback(Some(|s| {
            let input_path: String = {
                let ret = unsafe { std::ffi::CStr::from_ptr(s).to_string_lossy().to_string() };
                ret.to_owned()
            };
            let s = app::Sender::<String>::get();
            s.send(input_path);
        }));

        menu::mac_set_about({
            let current_wind = wind.clone();
            let trans_ref = trans_ref.clone();

            move || {
                let logo_image_bytes = include_bytes!("../../images/Entrusted.png");
                let dialog_width = 350;
                let dialog_height = 200;
                let dialog_xpos = current_wind.x() + (current_wind.w() / 2) - (dialog_width / 2);
                let dialog_ypos = current_wind.y() + (current_wind.h() / 2) - (dialog_height / 2);
                let win_title = format!("{} {}", &trans_ref.gettext("About"),
                                        option_env!("CARGO_PKG_NAME").unwrap_or("Unknown"));

                let mut win = window::Window::default()
                    .with_size(dialog_width, dialog_height)
                    .with_pos(dialog_xpos, dialog_ypos)
                    .with_label(&win_title);

                let dialog_text = format!(
                    "{}\n{} {}\n\n{}",
                    option_env!("CARGO_PKG_DESCRIPTION").unwrap_or("Unknown"),
                    &trans_ref.gettext("Version"),
                    option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown"),
                    &trans_ref.gettext_fmt("Copyright {0} {1}", vec!["2021-2022", "Rimero Solutions Inc."])
                );

                let mut logo_frame = frame::Frame::default()
                    .with_size(200, 50)
                    .with_pos(dialog_width/2 - 100, WIDGET_GAP);

                if let Ok(img) = image::PngImage::from_data(logo_image_bytes) {
                    let mut img = img;
                    img.scale(50, 50, true, true);
                    logo_frame.set_image(Some(img));
                }

                frame::Frame::default()
                    .with_size(200, 60)
                    .below_of(&logo_frame, WIDGET_GAP)
                    .with_label(&dialog_text)
                    .with_align(enums::Align::Center | enums::Align::Inside);

                win.end();
                win.make_modal(true);
                win.show();

                while win.shown() {
                    app::wait();
                }
            }
        });
    }

    convert_pack_rc.borrow_mut().end();
    tabconvert_button.set_frame(enums::FrameType::DownBox);
    settings_pack_rc.borrow_mut().hide();

    tabsettings_button.set_callback({
        let convert_pack_rc_ref = convert_pack_rc.clone();
        let settings_pack_rc_ref = settings_pack_rc.clone();
        let mut tabconvert_button_ref = tabconvert_button.clone();
        let mut filelist_scroll_ref = filelist_scroll.clone();
        let mut wind_ref = wind.clone();

        move |b| {
            if !settings_pack_rc_ref.borrow().visible() {
                tabconvert_button_ref.set_frame(enums::FrameType::UpBox);
                b.set_frame(enums::FrameType::DownBox);
                convert_pack_rc_ref.borrow_mut().hide();
                settings_pack_rc_ref.borrow_mut().show();
                filelist_scroll_ref.redraw();
                wind_ref.redraw();
            }
        }
    });

    tabconvert_button.set_callback({
        let convert_pack_rc_ref = convert_pack_rc.clone();
        let mut tabsettings_button_ref = tabsettings_button.clone();
        let settings_pack_rc_ref = settings_pack_rc.clone();
        let mut wind_ref = wind.clone();

        move |b| {
            if !convert_pack_rc_ref.borrow().visible() {
                tabsettings_button_ref.set_frame(enums::FrameType::UpBox);
                b.set_frame(enums::FrameType::DownBox);
                settings_pack_rc_ref.borrow_mut().hide();
                convert_pack_rc_ref.borrow_mut().show();
                wind_ref.redraw();
            }
        }
    });

    fn add_to_conversion_queue(
        paths: Vec<PathBuf>,
        row_pack: &mut FileListWidget,
        group_scroll: &mut group::Scroll,
    ) -> bool {
        let mut added = false;

        for p in paths.iter() {
            let path = PathBuf::from(p);

            if path.exists() && !row_pack.contains_path(&path) {
                row_pack.add_file(path);
                added = true;
            }
        }

        if added {
            group_scroll.redraw();
        }

        added
    }

    convert_frame.handle({
        let mut dnd = false;
        let mut released = false;
        let mut filelist_scroll_ref = filelist_scroll.clone();
        let mut filelist_widget_ref = filelist_widget.clone();
        let mut selection_pack_ref = selection_pack.clone();
        let is_converting_ref = is_converting.clone();
        let selectall_frame_rc_ref = selectall_frame_rc.clone();
        let deselectall_frame_rc_ref = deselectall_frame_rc.clone();
        let mut convert_button_ref = convert_button.clone();
        let mut columns_frame_ref = columns_frame.clone();
        let dialog_title = selectfiles_dialog_title.clone();
        let mut row_convert_button_ref = row_convert_button.clone();

        move |_, ev| match ev {
            enums::Event::DndEnter => {
                dnd = true;
                true
            }
            enums::Event::DndDrag => true,
            enums::Event::DndRelease => {
                released = true;
                true
            }
            enums::Event::Paste => {
                if dnd && released {
                    let path  = app::event_text();
                    let path  = path.trim();
                    let path  = path.replace("file://", "");
                    let paths = path.split("\n");

                    let file_paths: Vec<PathBuf> = paths
                        .map(|p| PathBuf::from(p))
                        .filter(|p| p.exists())
                        .collect();

                    if is_converting_ref.load(Ordering::Relaxed) && !file_paths.is_empty() {
                        is_converting_ref.store(false, Ordering::Relaxed);
                        filelist_widget_ref.delete_all();
                        filelist_scroll_ref.scroll_to(0, 0);
                        filelist_scroll_ref.redraw();
                    }

                    if add_to_conversion_queue(file_paths, &mut filelist_widget_ref, &mut filelist_scroll_ref) {
                        if !selectall_frame_rc_ref.borrow().active() {
                            selectall_frame_rc_ref.borrow_mut().activate();
                            selectall_frame_rc_ref.borrow_mut().set_label_color(enums::Color::Blue);
                        }

                        if !deselectall_frame_rc_ref.borrow().active() {
                            deselectall_frame_rc_ref.borrow_mut().activate();
                            deselectall_frame_rc_ref.borrow_mut().set_label_color(enums::Color::Blue);
                        }

                        if !convert_button_ref.active() {
                            convert_button_ref.activate();
                            selection_pack_ref.set_damage(true);
                            selectall_frame_rc_ref.borrow_mut().show();
                            deselectall_frame_rc_ref.borrow_mut().show();

                            selection_pack_ref.resize(
                                selection_pack_ref.x(),
                                selection_pack_ref.y(),
                                150,
                                40,
                            );

                            selection_pack_ref.set_damage(true);
                            selection_pack_ref.redraw();
                            columns_frame_ref.redraw();
                            row_convert_button_ref.redraw();
                        }
                    }
                }

                true
            }
            enums::Event::Push => {
                let mut selectfiles_filedialog = dialog::FileDialog::new(dialog::FileDialogType::BrowseMultiFile);
                selectfiles_filedialog.set_title(&dialog_title);
                selectfiles_filedialog.show();

                let file_paths: Vec<PathBuf> = selectfiles_filedialog
                    .filenames()
                    .iter()
                    .map(|p| p.clone())
                    .filter(|p| p.exists())
                    .collect();

                if is_converting_ref.load(Ordering::Relaxed) && !file_paths.is_empty() {
                    is_converting_ref.store(false, Ordering::Relaxed);
                    filelist_widget_ref.delete_all();
                    filelist_scroll_ref.scroll_to(0, 0);
                    filelist_scroll_ref.redraw();
                }

                if add_to_conversion_queue(file_paths, &mut filelist_widget_ref, &mut filelist_scroll_ref) {
                    if !selectall_frame_rc_ref.borrow().active() {
                        selectall_frame_rc_ref.borrow_mut().activate();
                        selectall_frame_rc_ref.borrow_mut().set_label_color(enums::Color::Blue);
                    }

                    if !deselectall_frame_rc_ref.borrow().active() {
                        deselectall_frame_rc_ref.borrow_mut().activate();
                        deselectall_frame_rc_ref.borrow_mut().set_label_color(enums::Color::Blue);
                    }

                    if !convert_button_ref.active() {
                        convert_button_ref.activate();
                        selection_pack_ref.set_damage(true);
                        selectall_frame_rc_ref.borrow_mut().show();
                        deselectall_frame_rc_ref.borrow_mut().show();

                        selection_pack_ref.resize(
                            selection_pack_ref.x(),
                            selection_pack_ref.y(),
                            150,
                            40,
                        );

                        selection_pack_ref.set_damage(true);
                        selection_pack_ref.redraw();
                        columns_frame_ref.redraw();
                        row_convert_button_ref.redraw();
                    }
                }
                true
            }
            _ => false,
        }
    });

    wind.handle({
        let mut top_group_ref = top_group.clone();

        let mut helpinfo_frame_ref = helpinfo_frame.clone();

        let settings_pack_rc_ref = settings_pack_rc.clone();

        let mut filesuffix_pack_ref = filesuffix_pack.clone();
        let mut filesuffix_checkbutton_ref = filesuffix_checkbutton.clone();
        let filesuffix_input_rc_ref = filesuffix_input_rc.clone();

        let mut ocrlang_pack_ref = ocrlang_pack.clone();
        let mut ocrlang_checkbutton_ref = ocrlang_checkbutton.clone();
        let ocrlang_holdbrowser_rc_ref = ocrlang_holdbrowser_rc.clone();

        let mut openwith_pack_ref = openwith_pack.clone();
        let mut openwith_checkbutton_ref = openwith_checkbutton.clone();
        let openwith_inputchoice_rc_ref = openwith_inputchoice_rc.clone();
        let openwith_button_rc_ref = openwith_button_rc.clone();

        let ociimage_input_rc_ref = ociimage_input_rc.clone();
        let mut ociimage_checkbutton_ref = ociimage_checkbutton.clone();
        let mut ociimage_pack_ref = ociimage_pack.clone();

        let convert_pack_rc_ref = convert_pack_rc.clone();

        let mut selection_pack_ref = selection_pack.clone();
        let select_all_frame_ref = selectall_frame_rc.clone();
        let deselect_all_frame_ref = deselectall_frame_rc.clone();

        let mut filelist_scroll_ref = filelist_scroll.clone();
        let mut filelist_widget_ref = filelist_widget.clone();

        let mut row_convert_button_ref = row_convert_button.clone();
        let convert_frame_ref = convert_frame.clone();
        let mut convert_button_ref = convert_button.clone();
        let mut delete_button_ref = delete_button.clone();
        let mut columns_frame_ref = columns_frame.clone();

        let mut messages_frame_ref = messages_frame.clone();

        move |w, ev| match ev {
            enums::Event::Move => {
                w.redraw();
                true
            },
            enums::Event::Resize => {
                top_group_ref.resize(
                    WIDGET_GAP,
                    WIDGET_GAP,
                    w.w() - (WIDGET_GAP * 2),
                    30,
                );

                let tabs_width = tabconvert_button.w();
                tabsettings_button.resize(WIDGET_GAP, top_group_ref.y(), tabs_width, 30);
                tabconvert_button.resize(tabsettings_button.x() + WIDGET_GAP, top_group_ref.y(), tabs_width, 30);

                helpinfo_frame_ref.resize(tabconvert_button.x() + WIDGET_GAP, top_group_ref.y(), w.w() - (WIDGET_GAP * 4) - (tabs_width * 2), 30);

                let content_y = top_group_ref.y() + top_group_ref.h() + WIDGET_GAP;

                let scroller_height = w.h() - top_group_ref.h() - convert_frame_ref.h() - row_convert_button_ref.h() - (messages_frame_ref.h() * 3);

                convert_pack_rc_ref.borrow_mut().resize(
                    WIDGET_GAP,
                    content_y,
                    w.w() - (WIDGET_GAP * 2),
                    w.h() - top_group_ref.h() + WIDGET_GAP,
                );

                settings_pack_rc_ref.borrow_mut().resize(
                    WIDGET_GAP,
                    content_y,
                    w.w() - (WIDGET_GAP * 2),
                    w.h() - top_group_ref.h() + WIDGET_GAP,
                );

                row_convert_button_ref.resize(
                    WIDGET_GAP, row_convert_button_ref.y(), w.w() - (WIDGET_GAP * 2), row_convert_button_ref.h()
                );

                convert_button_ref.resize(
                    convert_frame_ref.w() - convert_button_ref.w(), convert_button_ref.y(), convert_button_ref.w(), convert_button_ref.h()
                );

                delete_button_ref.resize(
                    w.w() - (convert_button_ref.w() * 2), delete_button_ref.y(), delete_button_ref.w(), delete_button_ref.h()
                );

                filelist_scroll_ref.resize(
                    WIDGET_GAP,
                    filelist_scroll_ref.y(),
                    w.w() - (WIDGET_GAP * 2),
                    scroller_height,
                );

                let wval = w.w() - (WIDGET_GAP * 3);
                columns_frame_ref.widget_resize(WIDGET_GAP, columns_frame_ref.y(), w.w() - (WIDGET_GAP * 2), columns_frame_ref.h());

                filelist_widget_ref.resize(filelist_scroll_ref.x(), filelist_scroll_ref.y(), wval, 0);

                filelist_scroll_ref.redraw();

                let xx = ocrlang_holdbrowser_rc_ref.borrow_mut().x();

                ociimage_pack_ref.resize(
                    ociimage_pack.x(),
                    ociimage_pack.y(),
                    w.w() - (WIDGET_GAP * 2),
                    ociimage_pack.h(),
                );
                filesuffix_pack_ref.resize(
                    filesuffix_pack_ref.x(),
                    filesuffix_pack_ref.y(),
                    w.w() - (WIDGET_GAP * 2),
                    filesuffix_pack_ref.h(),
                );
                openwith_pack_ref.resize(
                    openwith_pack_ref.x() + WIDGET_GAP / 2,
                    openwith_pack_ref.y(),
                    w.w() - (WIDGET_GAP * 2),
                    openwith_pack_ref.h(),
                );
                filesuffix_checkbutton_ref.resize(
                    xx,
                    filesuffix_checkbutton_ref.y(),
                    ocrlang_checkbutton_ref.w(),
                    filesuffix_checkbutton_ref.h(),
                );
                ocrlang_checkbutton_ref.resize(
                    xx,
                    ocrlang_checkbutton_ref.y(),
                    ocrlang_checkbutton_ref.w(),
                    filesuffix_checkbutton_ref.h(),
                );

                let ocw = w.w() - (WIDGET_GAP * 3) - ocrlang_checkbutton.w();
                let och = (w.h() as f64 * 0.5) as i32;

                ociimage_checkbutton_ref.resize(
                    ocrlang_checkbutton_ref.x(),
                    ociimage_checkbutton_ref.y(),
                    ocrlang_checkbutton_ref.w(),
                    ociimage_checkbutton_ref.h(),
                );

                openwith_checkbutton_ref.resize(
                    ocrlang_checkbutton_ref.x(),
                    openwith_checkbutton_ref.y(),
                    ocrlang_checkbutton_ref.w(),
                    openwith_checkbutton_ref.h(),
                );

                ocrlang_pack_ref.resize(
                    xx,
                    ocrlang_pack_ref.y(),
                    w.w() - (WIDGET_GAP * 4),
                    och,
                );

                let yy = ocrlang_holdbrowser_rc_ref.borrow_mut().y();
                ocrlang_holdbrowser_rc_ref.borrow_mut().resize(
                    xx,
                    yy,
                    ocw,
                    och - (WIDGET_GAP * 2),
                );

                let ociimage_input_rc_y = ociimage_input_rc.borrow().y();
                let ociimage_input_rc_h = ociimage_input_rc.borrow().h();
                ociimage_input_rc_ref.borrow_mut().resize(xx, ociimage_input_rc_y, ocw, ociimage_input_rc_h);

                let filesuffix_input_rc_ref_y = filesuffix_input_rc_ref.borrow().y();
                let filesuffix_input_rc_ref_h = filesuffix_input_rc_ref.borrow().h();
                filesuffix_input_rc_ref.borrow_mut().resize(xx, filesuffix_input_rc_ref_y, ocw, filesuffix_input_rc_ref_h);

                let openwith_button_rc_ref_w = openwith_button_rc_ref.borrow().w();
                let openwith_inputchoice_rc_ref_y = openwith_inputchoice_rc_ref.borrow().y();
                let openwith_inputchoice_rc_ref_h = openwith_inputchoice_rc_ref.borrow().h();
                openwith_inputchoice_rc_ref.borrow_mut().resize(
                    xx,
                    openwith_inputchoice_rc_ref_y,
                    ocw - WIDGET_GAP - openwith_button_rc_ref_w,
                    openwith_inputchoice_rc_ref_h
                );

                let openwith_button_rc_ref_y = openwith_button_rc_ref.borrow().y();
                let openwith_button_rc_ref_h = openwith_button_rc_ref.borrow().h();
                openwith_button_rc_ref.borrow_mut().resize(
                    w.w() - WIDGET_GAP - openwith_button_rc_ref_w,
                    openwith_button_rc_ref_y,
                    openwith_button_rc_ref_w,
                    openwith_button_rc_ref_h
                );

                messages_frame_ref.resize(
                    messages_frame_ref.x(),
                    messages_frame_ref.y(),
                    w.w() - (WIDGET_GAP * 4),
                    messages_frame_ref.h(),
                );

                columns_frame_ref.redraw();
                filelist_scroll_ref.redraw();

                true
            }
            _ => {
                if ev.bits() == FileListWidgetEvent::SELECTION_CHANGED {
                    let selection = filelist_widget_ref.selected_indices();
                    let empty_selection = selection.is_empty();

                    if empty_selection && delete_button.active() {
                        delete_button.deactivate();
                    } else if !empty_selection && !delete_button.active() {
                        delete_button.activate();
                    }

                    if !filelist_widget_ref.has_files() {
                        selection_pack_ref.redraw();
                        convert_button_ref.deactivate();
                        select_all_frame_ref.borrow_mut().hide();
                        deselect_all_frame_ref.borrow_mut().hide();
                    }

                    filelist_widget_ref.container.redraw();
                    filelist_scroll_ref.redraw();
                    true
                } else if ev.bits() == FileListWidgetEvent::ALL_SELECTED {
                    filelist_widget_ref.select_all();
                    true
                } else if ev.bits() == FileListWidgetEvent::ALL_DESELECTED {
                    filelist_widget_ref.deselect_all();
                    true
                } else if app::event_state().is_empty() && app::event_key() == enums::Key::Escape {
                    true
                } else {
                    false
                }
            }
        }
    });

    let mut autoconvert = false;
    let args: Vec<String> = env::args().skip(1).collect();

    if !args.is_empty() {
        for arg in args.iter() {
            let input_path = PathBuf::from(&arg);

            if input_path.exists() {
                filelist_widget.add_file(input_path);
                autoconvert = true;
            }
        }
    }

    wind.end();
    wind.show();
    wind.resize(wind.x(), wind.y(), wind.w(), wind.h());

    if autoconvert {
        convert_button.do_callback();
    }

    while app.wait() {
        if let Some(msg) = r.recv() {
            let mut filelist_widget_ref = filelist_widget.clone();
            let mut scroll_ref = filelist_scroll.clone();
            let file_path = PathBuf::from(msg);
            let mut selection_pack_ref = selection_pack.clone();
            let select_all_frame_ref = selectall_frame_rc.clone();
            let mut filelist_scroll_ref = filelist_scroll.clone();
            let deselect_all_frame_ref = deselectall_frame_rc.clone();
            let is_converting_ref = is_converting.clone();

            if file_path.exists() {
                if is_converting_ref.load(Ordering::Relaxed) {
                    is_converting_ref.store(false, Ordering::Relaxed);
                    filelist_widget_ref.delete_all();
                    filelist_scroll_ref.scroll_to(0, 0);
                    filelist_scroll_ref.redraw();
                }

                if add_to_conversion_queue(vec![file_path], &mut filelist_widget_ref, &mut scroll_ref) {
                    if !selectall_frame_rc.borrow().active() {
                        selectall_frame_rc.borrow_mut().activate();
                        selectall_frame_rc.borrow_mut().set_label_color(enums::Color::Blue);
                    }

                    if !deselectall_frame_rc.borrow().active() {
                        deselectall_frame_rc.borrow_mut().activate();
                        deselectall_frame_rc.borrow_mut().set_label_color(enums::Color::Blue);
                    }

                    if !convert_button.active() {
                        convert_button.activate();
                        selection_pack_ref.set_damage(true);
                        select_all_frame_ref.borrow_mut().show();
                        deselect_all_frame_ref.borrow_mut().show();

                        selection_pack_ref.resize(
                            selection_pack_ref.x(),
                            selection_pack_ref.y(),
                            selection_pack_ref.w(),
                            40,
                        );

                        selection_pack_ref.set_damage(true);
                        selection_pack_ref.redraw();
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(not(any(target_os = "macos")))]
pub fn pdf_open_with(cmd: String, input: PathBuf) -> Result<(), Box<dyn Error>> {
    match Command::new(cmd).arg(input).spawn() {
        Ok(_) => Ok(()),
        Err(ex) => Err(ex.into()),
    }
}

#[cfg(target_os = "macos")]
pub fn pdf_open_with(cmd: String, input: PathBuf) -> Result<(), Box<dyn Error>> {
    match common::executable_find("open") {
        Some(open_cmd) => match Command::new(open_cmd).arg("-a").arg(cmd).arg(input).spawn() {
            Ok(mut child_proc) => {
                match child_proc.wait() {
                    Ok(exit_status) => {
                        if exit_status.success() {
                            Ok(())
                        } else {
                            Err("Could not open PDF file!".into())
                        }
                    },
                    Err(ex) => Err(ex.into())
                }
            },
            Err(ex) => Err(ex.into()),
        },
        None => Err("Could not find 'open' command in PATH!".into()),
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn list_apps_for_pdfs() -> HashMap<String, String> {
    HashMap::new()
}

#[cfg(target_os = "linux")]
pub fn list_apps_for_pdfs() -> HashMap<String, String> {
    use freedesktop_entry_parser::parse_entry;

    // See https://wiki.archlinux.org/title/XDG_MIME_Applications for the logic

    // TODO is TryExec the best way to get a program name vs 'Exec' and stripping arguments???
    // Exec=someapp -newtab %u => where '%u' could be the file input parameter on top of other defaults '-newtab'

    fn parse_desktop_apps(
        apps_dir: PathBuf,
        mime_pdf_desktop_refs: &str,
    ) -> HashMap<String, String> {
        let desktop_entries: Vec<&str> = mime_pdf_desktop_refs.split(";").collect();
        let mut result = HashMap::with_capacity(desktop_entries.len());

        for desktop_entry in desktop_entries {
            if desktop_entry.is_empty() {
                continue;
            }

            let mut desktop_entry_path = apps_dir.clone();
            desktop_entry_path.push(desktop_entry);

            if desktop_entry_path.exists() {
                if let Ok(desktop_entry_data) = parse_entry(desktop_entry_path) {
                    let desktop_entry_section = desktop_entry_data.section("Desktop Entry");

                    if let (Some(app_name), Some(cmd_name)) = (
                        &desktop_entry_section.attr("Name"),
                        &desktop_entry_section
                            .attr("TryExec")
                            .or(desktop_entry_section.attr("Exec")),
                    ) {
                        let cmd_name_sanitized = cmd_name
                            .to_string()
                            .replace("%u", "")
                            .replace("%U", "")
                            .replace("%f", "")
                            .replace("%F", "");
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
            if let Some(mime_pdf_desktop_refs) = conf.section("MIME Cache").attr("application/pdf")
            {
                let tmp_result = parse_desktop_apps(
                    path_usr_share_applications_orig.clone(),
                    mime_pdf_desktop_refs,
                );

                for (k, v) in &tmp_result {
                    ret.insert(k.to_string(), v.to_string());
                }
            }
        }
    }

    let mut additional_xdg_files = vec![
        PathBuf::from("/etc/xdg/mimeapps.list"),
        PathBuf::from("/usr/local/share/applications/mimeapps.list"),
        PathBuf::from("/usr/share/applications/mimeapps.list"),
    ];

    if let Ok(homedir) = env::var("HOME") {
        let home_config_mimeapps: PathBuf =
            [homedir.as_str(), ".config/mimeapps.list"].iter().collect();
        let home_local_mimeapps: PathBuf =
            [homedir.as_str(), ".local/share/applications/mimeapps.list"]
            .iter()
            .collect();
        additional_xdg_files.push(home_config_mimeapps);
        additional_xdg_files.push(home_local_mimeapps);
    }

    for additional_xdg_file in additional_xdg_files {
        if additional_xdg_file.exists() {
            if let Ok(conf) = parse_entry(additional_xdg_file) {
                if let Some(mime_pdf_desktop_refs) =
                    conf.section("Added Associations").attr("application/pdf")
                {
                    let tmp_result = parse_desktop_apps(
                        path_usr_share_applications_orig.clone(),
                        mime_pdf_desktop_refs,
                    );

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
#[cfg(target_os = "windows")]
pub fn list_apps_for_pdfs() -> HashMap<String, String> {
    use std::collections::HashSet;
    use winreg::enums::RegType;
    use winreg::enums::HKEY_CLASSES_ROOT;
    use winreg::RegKey;
    let mut ret = HashMap::new();
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    let open_with_list_hkcr_res = hkcr.open_subkey(".pdf\\OpenWithProgids");

    fn friendly_app_name(regkey: &RegKey, name: String) -> String {
        let app_id = format!("{}\\Application", name);

        if let Ok(app_application_regkey) = regkey.open_subkey(app_id) {
            let app_result: std::io::Result<String> =
                app_application_regkey.get_value("ApplicationName");

            if let Ok(ret) = app_result {
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
                            let updated_path =
                                human_app_path_with_trailing_backlash[..path_len].to_string();

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

#[cfg(target_os = "macos")]
pub fn list_apps_for_pdfs() -> HashMap<String, String> {
    use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex};
    use core_foundation::url::CFURL;
    use core_foundation::bundle::CFBundle;
    use core_services::CFString;
    use core_foundation::string::{
        kCFStringEncodingUTF8, CFStringCreateWithCString, CFStringGetCStringPtr, CFStringRef,
    };
    use core_foundation::url::{CFURLCopyPath, CFURLRef};
    use core_services::{
        kLSRolesAll, LSCopyAllRoleHandlersForContentType, LSCopyApplicationURLsForBundleIdentifier,
    };
    use percent_encoding::percent_decode;
    use std::ffi::{CStr, CString};

    let content_type = "com.adobe.pdf";
    let mut ret = HashMap::new();

    unsafe {
        if let Ok(c_key) = CString::new(content_type) {
            let cf_key = CFStringCreateWithCString(std::ptr::null(), c_key.as_ptr(), kCFStringEncodingUTF8);
            let result = LSCopyAllRoleHandlersForContentType(cf_key, kLSRolesAll);
            let count = CFArrayGetCount(result);

            for i in 0..count {
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
                        let mut app_name = String::new();

                        if let Ok(app_url) = c_str.to_str() {
                            if let Ok(app_url_decoded) = percent_decode(&app_url.as_bytes()).decode_utf8() {
                                let app_path = app_url_decoded.to_string();

                                if let Some(bundle_url) = CFURL::from_path(&app_path, true) {
                                    if let Some(bundle) = CFBundle::new(bundle_url) {
                                        let bundle_dict = bundle.info_dictionary();
                                        let bundle_key_display_name = CFString::new("CFBundleDisplayName");
                                        let bundle_key_name = CFString::new("CFBundleName");

                                        let current_key = if bundle_dict.contains_key(&bundle_key_display_name) {
                                            Some(bundle_key_display_name)
                                        } else if bundle_dict.contains_key(&bundle_key_name) {
                                            Some(bundle_key_name)
                                        } else {
                                            None
                                        };

                                        if let Some(active_key) = current_key {
                                            if let Some(active_key_value) = bundle_dict.find(&active_key)
                                                .and_then(|value_ref| value_ref.downcast::<CFString>())
                                                .map(|value| value.to_string()) {
                                                    app_name.push_str(&active_key_value);
                                                }
                                        } else {
                                            if let Some(basename_ostr) = std::path::Path::new(&app_url).file_stem() {
                                                if let Some(basename) = &basename_ostr.to_str() {
                                                    let implied_app_name = percent_decode(basename.as_bytes());

                                                    if let Ok(r_app_name_decoded)= implied_app_name.decode_utf8() {
                                                        app_name.push_str(&r_app_name_decoded);
                                                    }
                                                }
                                            }
                                        }

                                        if !app_name.is_empty() {
                                            ret.insert(app_name, app_path);
                                        }
                                    }
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
