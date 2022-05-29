#![windows_subsystem = "windows"]

use serde_json;
use std::cell::RefCell;
use std::cmp;
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
    app, browser, button, dialog, draw, enums, frame, group, input, menu, misc, prelude::*, text, window,
};

mod common;
mod container;

const WIDGET_GAP: i32 = 20;

macro_rules! enum_str {
    (enum $name:ident {
        $($variant:ident = $val:expr),*,
    }) => {
        enum $name {
            $($variant = $val),*
        }

        impl $name {
            fn name(&self) -> &'static str {
                match self {
                    $($name::$variant => stringify!($variant)),*
                }
            }
        }
    };
}

enum_str! {
    enum FileListRowStatus {
        Pending    = 0x00,
        InProgress = 0x01,
        Succeeded  = 0x02,
        Failed     = 0x03,
    }
}

struct FileListWidgetEvent;

impl FileListWidgetEvent {
    const SELECTION_CHANGED: i32 = 50;
    const ALL_SELECTED: i32      = 51;
    const ALL_DESELECTED: i32    = 52;
}

#[derive(Clone)]
struct FileListRow {
    file: PathBuf,
    checkbox: button::CheckButton,
    progressbar: misc::Progress,
    status: frame::Frame,
    log_link: button::Button,
    logs: Rc<RefCell<Vec<String>>>,
}

impl FileListRow {
    pub fn reset_ui_state(&mut self) {
        self.status.set_label(FileListRowStatus::Pending.name());
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
}

impl Deref for FileListWidget {
    type Target = group::Pack;

    fn deref(&self) -> &Self::Target {
        &self.container
    }
}

impl DerefMut for FileListWidget {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.container
    }
}

impl FileListWidget {
    pub fn new() -> Self {
        let mut container = group::Pack::default().with_type(group::PackType::Vertical).with_size(300, 300);
        container.set_spacing(WIDGET_GAP);
        container.end();
        container.auto_layout();

        Self {
            container,
            selected_indices: Rc::new(RefCell::new(vec![])),
            rows: Rc::new(RefCell::new(vec![])),
        }
    }

    pub fn column_widths(&self, w: i32) -> (i32, i32, i32, i32){
        let width_checkbox    = (w as f64 * 0.4) as i32;
        let width_progressbar = (w as f64 * 0.2) as i32;
        let width_status      = (w as f64 * 0.2) as i32;
        let width_logs        = (w as f64 * 0.1) as i32;

        (width_checkbox, width_progressbar, width_status, width_logs)
    }

    pub fn resize(&mut self, x: i32, y: i32, w: i32, _: i32) {        
        self.container.resize(x, y, w, self.container.h());

        let (width_checkbox, width_progressbar, width_status, width_logs) = self.column_widths(w);

        for row in self.rows.borrow().iter() {
            let mut active_child = row.clone();

            let mut pos_x = active_child.checkbox.x();
            active_child.checkbox.resize(pos_x, active_child.checkbox.y(), width_checkbox, active_child.checkbox.h());
            active_child.checkbox.set_label(&self.adjust_label(active_child.file, width_checkbox));

            pos_x += width_checkbox + WIDGET_GAP;
            active_child.progressbar.resize(pos_x, active_child.progressbar.y(), width_progressbar, active_child.progressbar.h());

            pos_x += width_progressbar + WIDGET_GAP;

            active_child.status.resize(pos_x, active_child.status.y(), width_status, active_child.status.h());

            pos_x += width_status + WIDGET_GAP;

            active_child.log_link.resize(pos_x, active_child.log_link.y(), width_logs, active_child.log_link.h());
        }

    }

    pub fn contains_path(&self, p: PathBuf) -> bool {
        self.rows
            .borrow()
            .iter()
            .find(|row| *row.file == p)
            .is_some()
    }

    pub fn has_files(&self) -> bool {
        !self.rows.borrow().is_empty()
    }

    fn toggle_selection(&mut self, select: bool) {
        for row in self.rows.borrow().iter() {
            if row.checkbox.active() {
                row.checkbox.set_checked(select);
            }
        }
    }

    pub fn selected_indices(&self) -> Vec<usize> {
        self.selected_indices
            .borrow()
            .iter()
            .map(|i| i.clone())
            .collect()
    }

    pub fn select_all(&mut self) {
        let row_count = self.rows.borrow().len();
        self.toggle_selection(true);
        self.selected_indices.borrow_mut().extend(0..row_count);
        let _ = app::handle_main(FileListWidgetEvent::SELECTION_CHANGED);
    }

    pub fn deselect_all(&mut self) {
        self.toggle_selection(false);
        self.selected_indices.borrow_mut().clear();
        let _ = app::handle_main(FileListWidgetEvent::SELECTION_CHANGED);
    }

    pub fn delete_selection(&mut self) {
        self.selected_indices.borrow_mut().sort_by(|a, b| a.cmp(b));

        while !self.selected_indices.borrow().is_empty() {
            if let Some(idx) = self.selected_indices.borrow_mut().pop() {
                let row = self.rows.borrow_mut().remove(idx);
                self.container.remove(&row.checkbox.parent().unwrap());
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

    pub fn adjust_label(&self, path: PathBuf, w1: i32) -> String {
        let mut path_name = format!("{}", path.file_name().and_then(|x| x.to_str()).unwrap());
        let effective_w = w1 - WIDGET_GAP;

        let (mut xx, _) = draw::measure(&path_name, true);
        if xx > effective_w {
            path_name = path_name
                .chars()
                .take(path_name.len() - cmp::min(3, path_name.len()))
                .collect::<String>();
            while xx > effective_w {
                path_name = path_name
                    .chars()
                    .take(path_name.len() - 1)
                    .collect::<String>();
                let (xxx, _) = draw::measure(&path_name, true);
                xx = xxx;
            }
            path_name = path_name + "...";
        }

        path_name
    }

    pub fn add_file(&mut self, path: PathBuf) {
        let ww = self.container.w();

        let (width_checkbox, width_progressbar, width_status, width_logs) = self.column_widths(ww);

        let mut row = group::Pack::default()
            .with_type(group::PackType::Horizontal)
            .with_size(ww, 40);

        row.set_spacing(WIDGET_GAP);

        let path_tooltip = format!("{}", path.display());
        let mut select_row_checkbutton = button::CheckButton::default()
            .with_size(width_checkbox, 30)
            .with_label(&self.adjust_label(path.clone(), width_checkbox));
        select_row_checkbutton.set_tooltip(&path_tooltip);

        let check_buttonx2 = select_row_checkbutton.clone();
        let progressbar = misc::Progress::default().with_size(width_progressbar, 20).with_label("0%");

        let mut status_frame = frame::Frame::default()
            .with_size(width_status, 30)
            .with_label(FileListRowStatus::Pending.name())
            .with_align(enums::Align::Inside | enums::Align::Left);
        status_frame.set_label_color(enums::Color::Magenta);

        let mut logs_button = button::Button::default()
            .with_size(width_logs, 30)
            .with_label("   ");
        logs_button.set_frame(enums::FrameType::NoBox);
        logs_button.set_down_frame(enums::FrameType::NoBox);
        logs_button.deactivate();
        logs_button.set_label_color(enums::Color::Blue);

        row.end();

        let file_list_row = FileListRow {
            checkbox: check_buttonx2,
            progressbar,
            status: status_frame,
            log_link: logs_button.clone(),
            logs: Rc::new(RefCell::new(vec![])),
            file: path.clone(),
        };

        logs_button.set_callback({
            let active_row = file_list_row.clone();

            move |_| {
                if let Some(current_wind) = app::first_window() {
                    let wind_w = 400;
                    let wind_h = 400;
                    let button_width = 50;
                    let button_height = 30;
                    let wind_x = current_wind.x() + (current_wind.w() / 2) - (wind_w / 2);
                    let wind_y = current_wind.y() + (current_wind.h() / 2) - (wind_h / 2);

                    let mut dialog = window::Window::default()
                        .with_size(wind_w, wind_h)
                        .with_pos(wind_x, wind_y)
                        .with_label("Logs");

                    dialog.begin();

                    let mut textdisplay_cmdlog = text::TextDisplay::default()
                        .with_type(group::PackType::Vertical)
                        .with_size(wind_w, 350);
                    let mut text_buffer = text::TextBuffer::default();
                    let logs = active_row.logs.borrow().join("\n") + "\n";

                    let mut log_close_button = button::Button::default()
                        .with_pos((wind_w / 2) - (button_width / 2), 400 - button_height - (WIDGET_GAP / 2))
                        .with_size(button_width, button_height)
                        .with_label("Close");


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

        select_row_checkbutton.set_callback({
            let selfie = self.clone();
            let current_path = path.clone();

            move |b| {
                let idx = selfie.row_index(&current_path);

                if idx != -1 {
                    if b.is_checked() {
                        selfie.selected_indices.borrow_mut().push(idx as usize);
                    } else {
                        selfie.selected_indices.borrow_mut().remove(idx as usize);
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
    let is_converting = Arc::new(AtomicBool::new(false));
    let app = app::App::default().with_scheme(app::Scheme::Gleam);

    let wind_title = format!(
        "{} {}",
        option_env!("CARGO_PKG_NAME").unwrap_or("Unknown"),
        option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown")
    );

    let mut wind = window::Window::default()
        .with_size(680, 600)
        .center_screen()
        .with_label(&wind_title);

    wind.make_resizable(true);

    let mut top_group = group::Pack::default()
        .with_pos(20, 20)
        .with_size(680, 25)
        .with_type(group::PackType::Horizontal)
        .with_align(enums::Align::Inside | enums::Align::Right);

    top_group.set_spacing(WIDGET_GAP);

    let mut settings_togglebutton = button::Button::default()
        .with_size(80, 20)
        .with_label("Settings");
    let mut convert_togglebutton = button::Button::default()
        .with_size(80, 20)
        .with_label("Convert");
    top_group.end();

    let settings_group = Rc::new(RefCell::new(
        group::Pack::default()
            .with_pos(20, 20)
            .with_size(600, 580)
            .below_of(&top_group, WIDGET_GAP)
            .with_type(group::PackType::Vertical),
    ));

    settings_group.borrow_mut().set_spacing(WIDGET_GAP);

    let mut row_inputloc = group::Pack::default()
        .with_size(570, 40)
        .with_type(group::PackType::Horizontal);

    row_inputloc.set_spacing(WIDGET_GAP);
    let mut checkbutton_custom_output = button::CheckButton::default()
        .with_size(160, 20)
        .with_label("Custom file suffix");
    checkbutton_custom_output
        .set_tooltip("The safe PDF will be named <input>-<suffix>.pdf by default.");
    checkbutton_custom_output.set_checked(false);

    let input_outputloc = Rc::new(RefCell::new(input::Input::default().with_size(290, 20)));
    let input_outputlocx = input_outputloc.clone();
    input_outputloc.borrow_mut().set_value(common::DEFAULT_FILE_SUFFIX);
    input_outputloc.borrow_mut().deactivate();

    checkbutton_custom_output.set_callback({
        let input_outputloc_ref = input_outputlocx.clone();

        move|b| {
            if b.is_checked() {
                input_outputloc_ref.borrow_mut().activate();
            } else {
                input_outputloc_ref.borrow_mut().set_value(common::DEFAULT_FILE_SUFFIX);
                input_outputloc_ref.borrow_mut().deactivate();
            }
        }
    });

    row_inputloc.end();

    let mut row_ocr_language = group::Pack::default()
        .with_size(570, 60)
        .below_of(&row_inputloc, WIDGET_GAP);
    row_ocr_language.set_type(group::PackType::Horizontal);
    row_ocr_language.set_spacing(WIDGET_GAP);
    let mut checkbutton_ocr_lang = button::CheckButton::default()
        .with_size(300, 20)
        .with_label("Searchable PDF, with language");
    checkbutton_ocr_lang.set_tooltip(
        "Make the PDF searchable, with a given language for OCR (Optical character recognition).",
    );
    checkbutton_ocr_lang.set_checked(false);

    let ocr_language_list = Rc::new(RefCell::new(
        browser::HoldBrowser::default().with_size(240, 60),
    ));
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
        ocr_language_list
            .borrow_mut()
            .select((selected_ocr_language_idx + 1) as i32);
    }

    ocr_language_list.borrow_mut().deactivate();

    checkbutton_ocr_lang.set_callback({
        let ocr_language_list_ref = ocr_language_list.clone();

        move |b| {
            if !b.is_checked() {
                ocr_language_list_ref.borrow_mut().deactivate();
            } else {
                ocr_language_list_ref.borrow_mut().activate();
            }
        }
    });
    row_ocr_language.end();

    let mut row_openwith = group::Pack::default()
        .with_size(570, 40)
        .with_type(group::PackType::Horizontal);
    row_openwith.set_spacing(WIDGET_GAP);
    let mut checkbutton_openwith = button::CheckButton::default().with_size(295, 20).with_label("Open document after conversion, using");
    checkbutton_openwith.set_tooltip("Automatically open resulting PDFs with a given program.");

    let pdf_apps_by_name = list_apps_for_pdfs();
    let pdf_viewer_list = Rc::new(RefCell::new(misc::InputChoice::default().with_size(240, 20)));
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
    let button_browse_for_pdf_appx = button_browse_for_pdf_app.clone();
    button_browse_for_pdf_app.borrow_mut().set_tooltip("Browse for PDF viewer program");
    button_browse_for_pdf_app.borrow_mut().deactivate();

    checkbutton_openwith.set_callback({
        let pdf_viewer_list_ref = pdf_viewer_list.clone();

        move |b| {
            let will_be_read_only = !b.is_checked();
            pdf_viewer_list_ref.borrow_mut().input().set_readonly(will_be_read_only);

            if will_be_read_only {
                pdf_viewer_list_ref.borrow_mut().deactivate();
                button_browse_for_pdf_app_copy.borrow_mut().deactivate();
            } else {
                pdf_viewer_list_ref.borrow_mut().activate();
                button_browse_for_pdf_app_copy.borrow_mut().activate();
            };
        }
    });

    button_browse_for_pdf_app.borrow_mut().set_callback({
        let pdf_viewer_list_ref = pdf_viewer_list.clone();

        move |_| {
            let mut dlg = dialog::FileDialog::new(dialog::FileDialogType::BrowseFile);
            dlg.set_title("Select PDF viewer program");
            dlg.show();

            let selected_filename = dlg.filename();

            if !selected_filename.as_os_str().is_empty() {
                let path_name = format!("{}", dlg.filename().display());
                pdf_viewer_list_ref.borrow_mut().set_value(&path_name);
            }
        }
    });

    row_openwith.end();

    let mut row_oci_image = group::Pack::default()
        .with_size(550, 40)
        .below_of(&row_ocr_language, WIDGET_GAP);
    row_oci_image.set_type(group::PackType::Horizontal);
    row_oci_image.set_spacing(WIDGET_GAP);
    let mut output_oci_image = button::CheckButton::default()
        .with_size(100, 20)
        .with_pos(0, 0)
        .with_align(enums::Align::Inside | enums::Align::Left);
    output_oci_image.set_label("Custom container image");
    output_oci_image.set_tooltip("Expert option for sandbox solution");
    output_oci_image.set_checked(false);

    let input_oci_image = Rc::new(RefCell::new(input::Input::default().with_size(440, 20)));
    input_oci_image.borrow_mut().set_value(&common::container_image_name());
    input_oci_image.borrow_mut().deactivate();
    row_oci_image.end();

    output_oci_image.set_callback({
        let input_oci_image_ref = input_oci_image.clone();

        move|b| {
            if !b.is_checked() {
                input_oci_image_ref.borrow_mut().deactivate();
                input_oci_image_ref.borrow_mut().set_value(&common::container_image_name());
            } else {
                input_oci_image_ref.borrow_mut().activate();
            }
        }
    });

    settings_group.borrow_mut().end();

    let convert_group = Rc::new(RefCell::new(
        group::Pack::default()
            .with_pos(20, 20)
            .with_size(600, 580)
            .below_of(&top_group, WIDGET_GAP)
            .with_type(group::PackType::Vertical),
    ));

    convert_group.borrow_mut().set_spacing(WIDGET_GAP);

    let mut convert_frame = frame::Frame::default().with_size(500, 80).with_pos(10, 10);
    convert_frame.set_frame(enums::FrameType::RFlatBox);
    convert_frame.set_label_color(enums::Color::White);
    convert_frame.set_label("Drop file(s) here\nor Click here to select file(s)");
    convert_frame.set_color(enums::Color::Red);

    let mut row_convert_button = group::Pack::default()
        .with_size(500, 40)
        .below_of(&convert_frame, 30);
    row_convert_button.set_type(group::PackType::Horizontal);
    row_convert_button.set_spacing(WIDGET_GAP);

    let mut file_actions_group = group::Pack::default()
        .with_size(150, 20)
        .with_type(group::PackType::Vertical)
        .below_of(&convert_frame, 30);

    let select_all_frame = Rc::new(RefCell::new(
        frame::Frame::default()
            .with_size(130, 10)
            .with_label("Select all")
            .with_align(enums::Align::Inside | enums::Align::Left),
    ));
    select_all_frame
        .borrow_mut()
        .set_label_color(enums::Color::Blue);
    let deselect_all_frame = Rc::new(RefCell::new(
        frame::Frame::default()
            .with_size(130, 10)
            .with_label("Deselect all")
            .with_align(enums::Align::Inside | enums::Align::Left),
    ));
    deselect_all_frame
        .borrow_mut()
        .set_label_color(enums::Color::Blue);
    file_actions_group.set_spacing(WIDGET_GAP / 2);

    select_all_frame.borrow_mut().draw({
        move |w| {
            let (lw, _) = draw::measure(&w.label(), true);
            draw::draw_line(w.x() + 3, w.y() + w.h(), w.x() + lw, w.y() + w.h());
        }
    });

    deselect_all_frame.borrow_mut().draw({
        move |w| {
            let (lw, _) = draw::measure(&w.label(), true);
            draw::draw_line(w.x() + 3, w.y() + w.h(), w.x() + lw, w.y() + w.h());
        }
    });

    select_all_frame.borrow_mut().handle({
        move |_, ev| match ev {
            enums::Event::Push => {
                let _ = app::handle_main(FileListWidgetEvent::ALL_SELECTED);
                true
            }
            _ => false,
        }
    });

    deselect_all_frame.borrow_mut().handle({
        move |_, ev| match ev {
            enums::Event::Push => {
                let _ = app::handle_main(FileListWidgetEvent::ALL_DESELECTED);
                true
            }
            _ => false,
        }
    });

    select_all_frame.borrow_mut().hide();
    deselect_all_frame.borrow_mut().hide();

    file_actions_group.end();

    let mut button_delete_file = button::Button::default()
        .with_size(200, 20)
        .with_label("Remove selected file(s)");
    button_delete_file.deactivate();

    let mut button_convert = button::Button::default()
        .with_size(200, 20)
        .with_label("Convert to trusted PDF(s)");
    let button_convertx = button_convert.clone();
    let mut button_convertxx = button_convert.clone();
    button_convert.set_label_color(enums::Color::White);
    button_convert.set_frame(enums::FrameType::ThinUpBox);
    button_convert.set_color(enums::Color::Black);

    button_convert.deactivate();

    row_convert_button.end();

    let scroll = group::Scroll::default().with_size(580, 200);

    let mut filelist_widget = FileListWidget::new();

    button_delete_file.set_callback({
        let mut filelist_widget_ref = filelist_widget.clone();

        move |_| {
            filelist_widget_ref.delete_selection();
        }
    });

    scroll.end();

    let messages_frame = frame::Frame::default()
        .with_size(580, 50)
        .with_label(" ")
        .with_align(enums::Align::Left | enums::Align::Inside);

    button_convert.set_callback({
        let wind_ref = wind.clone();
        let filelist_widget_ref = filelist_widget.clone();
        let mut messages_frame_ref = messages_frame.clone();
        let ocr_language_list_ref = ocr_language_list.clone();
        let checkbutton_ocr_lang_ref = checkbutton_ocr_lang.clone();
        let input_oci_image_ref = input_oci_image.clone();
        let pdf_viewer_list_ref = pdf_viewer_list.clone();
        let input_outputloc_ref = input_outputlocx.clone();
        let is_converting_ref = is_converting.clone();

        move |b| {
            is_converting_ref.store(true, Ordering::Relaxed);
            let file_suffix = input_outputloc_ref.borrow().value();
            let mut file_suffix = String::from(file_suffix.clone().trim());

            if file_suffix.is_empty() {
                file_suffix = String::from(common::DEFAULT_FILE_SUFFIX);
            }

            let viewer_app_name = pdf_viewer_list_ref.borrow_mut().input().value();
            let viewer_app_exec = if checkbutton_openwith.is_checked() {
                if let Some(viewer_app_path) = pdf_apps_by_name.get(&viewer_app_name) {
                    Some(viewer_app_path.clone())
                } else {
                    Some(String::from(viewer_app_name.trim()))
                }
            } else {
                None
            };

            b.deactivate();

            for current_row in filelist_widget_ref.rows.borrow_mut().iter() {
                let mut active_row = current_row.clone();
                active_row.reset_ui_state();
            }

            let ocr_lang_setting = if checkbutton_ocr_lang_ref.is_checked() {
                if let Some(selected_lang) = ocr_language_list_ref.borrow().selected_text() {
                    ocr_languages_by_lang
                        .get(selected_lang.as_str())
                        .map(|i| format!("{}", i))
                } else {
                    None
                }
            } else {
                None
            };

            let oci_image_text = input_oci_image_ref.borrow().value();

            let oci_image  = if oci_image_text.trim().is_empty() {
                None
            } else {
                Some(String::from(oci_image_text.trim()))
            };

            let viewer_app_option = viewer_app_exec.clone();

            for current_row in filelist_widget_ref.rows.borrow_mut().iter() {
                let result = Arc::new(AtomicBool::new(false));
                let mut row = current_row.clone();
                let input_path = row.file.clone();
                let ocr_lang_param = ocr_lang_setting.clone();
                let oci_image_copy = oci_image.clone();
                let viewer_app_option2 = viewer_app_option.clone();
                let file_suffix_value = file_suffix.clone();
                let (tx, rx) = mpsc::channel();

                if let Ok(output_path) = common::default_output_path(input_path.clone(), file_suffix_value) {
                    let output_path2 = output_path.clone();
                    let input_path2 = input_path.clone();
                    row.status.set_label(FileListRowStatus::InProgress.name());
                    row.status.set_label_color(enums::Color::DarkYellow);

                    let mut exec_handle = Some(thread::spawn(move || {
                        match container::convert(
                            input_path2.clone(),
                            output_path.clone(),
                            oci_image_copy,
                            String::from("json"),
                            ocr_lang_param,
                            tx,
                        ) {
                            Ok(_) => None,
                            Err(ex) => Some(format!("{}", ex)),
                        }
                    }));

                    while let Ok(raw_msg) = rx.recv() {
                        app::wait();
                        let log_msg_ret: serde_json::Result<common::LogMessage> =
                            serde_json::from_slice(raw_msg.as_bytes());

                        if let Ok(log_msg) = log_msg_ret {
                            let progress_text = format!("{} %", log_msg.percent_complete);
                            row.progressbar.set_label(&progress_text);
                            row.progressbar.set_value(log_msg.percent_complete as f64);
                            messages_frame_ref.set_label(&log_msg.data);
                            row.logs.borrow_mut().push(log_msg.data);
                            row.progressbar.parent().unwrap().redraw();
                        }

                        app::awake();
                    }

                    let mut status_color = enums::Color::Red;
                    let mut row_status = FileListRowStatus::Failed.name();

                    match exec_handle.take().map(thread::JoinHandle::join) {
                        Some(exec_handle_result) => match exec_handle_result {
                            Ok(None) => {
                                result.swap(true, Ordering::Relaxed);
                                row.progressbar.set_label("100%");
                                row.progressbar.set_value(100.0);
                                status_color = enums::Color::DarkGreen;
                                row_status = FileListRowStatus::Succeeded.name();
                            }
                            Ok(err_string_opt) => {
                                if let Some(err_text) = err_string_opt {
                                    row.logs.borrow_mut().push(err_text.clone());
                                    row.log_link.set_label(&err_text);
                                }
                            }
                            Err(ex) => {
                                let err_text = format!("{:?}", ex);
                                row.logs.borrow_mut().push(err_text.clone());
                                row.log_link.set_label(&err_text);
                            }
                        },
                        None => {
                            let label_text = "Conversion failed";
                            row.log_link.set_label(label_text);
                            row.logs.borrow_mut().push(String::from(label_text));
                        }
                    }

                    row.status.set_label(row_status);
                    row.status.set_label_color(status_color);
                    row.progressbar.set_label("100%");
                    row.progressbar.set_value(100.0);
                    row.log_link.set_label("Logs");
                    row.log_link.set_frame(enums::FrameType::ThinUpBox);
                    row.log_link.set_down_frame(enums::FrameType::ThinDownBox);
                    row.log_link.activate();
                    messages_frame_ref.set_label("");

                    if result.load(Ordering::Relaxed) && viewer_app_option2.is_some() {
                        if let Some(viewer_exe) = viewer_app_option2 {
                            if let Err(exe) = pdf_open_with(viewer_exe, output_path2.clone()) {
                                let err_text = format!("Could not open PDF result\n.{}.", exe.to_string());
                                dialog::alert(wind_ref.x(), wind_ref.y() + wind_ref.height() / 2, &err_text);
                            }
                        }
                    }

                }
            }

            b.activate();
        }
    });

    if cfg!(target_os = "macos") {
        menu::mac_set_about({
            let current_wind = wind.clone();
            move || {
                let ww = 350;
                let wh = 150;
                let wwx = current_wind.x() + (current_wind.w() / 2) - (ww / 2);
                let wwy = current_wind.y() + (current_wind.h() / 2) - (wh / 2);

                let win_title = format!("About {}", option_env!("CARGO_PKG_NAME").unwrap_or("Unknown"));
                let mut win = window::Window::default()
                    .with_size(ww, wh)
                    .with_pos(wwx, wwy)
                    .with_label(&win_title);

                let dialog_text = format!(
                    "{}\nVersion {}",
                    option_env!("CARGO_PKG_DESCRIPTION").unwrap_or("Unknown"),
                    option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown")
                );

                frame::Frame::default_fill()
                    .center_of_parent()
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

    convert_group.borrow_mut().end();
    convert_togglebutton.set_frame(enums::FrameType::DownBox);

    settings_group.borrow_mut().hide();

    settings_togglebutton.set_callback({
        let convert_group_ref = convert_group.clone();
        let settings_group_ref = settings_group.clone();
        let mut convert_togglebutton_ref = convert_togglebutton.clone();
        let mut scroll_ref = scroll.clone();

        move |b| {
            if !settings_group_ref.borrow().visible() {
                convert_togglebutton_ref.set_frame(enums::FrameType::UpBox);
                b.set_frame(enums::FrameType::DownBox);
                convert_group_ref.borrow_mut().hide();
                settings_group_ref.borrow_mut().show();
                scroll_ref.redraw();
            }
        }
    });
    convert_togglebutton.set_callback({
        let convert_group_ref = convert_group.clone();
        let mut settings_togglebutton_ref = settings_togglebutton.clone();
        let settings_group_ref = settings_group.clone();
        let mut wind_ref = wind.clone();

        move |b| {
            if !convert_group_ref.borrow().visible() {
                settings_togglebutton_ref.set_frame(enums::FrameType::UpBox);
                b.set_frame(enums::FrameType::DownBox);
                settings_group_ref.borrow_mut().hide();
                convert_group_ref.borrow_mut().show();
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

        for p in paths {
            let path = PathBuf::from(p);

            if path.exists() {
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
        let mut scroll_ref = scroll.clone();
        let mut filelist_widget_ref = filelist_widget.clone();
        let mut file_actions_group_ref = file_actions_group.clone();
        let is_converting_ref = is_converting.clone();
        let select_all_frame_ref = select_all_frame.clone();
        let deselect_all_frame_ref = deselect_all_frame.clone();

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
                    let path = app::event_text();
                    let path = path.trim();
                    let path = path.replace("file://", "");
                    let paths = path.split("\n");

                    if is_converting_ref.load(Ordering::Relaxed) {
                        is_converting_ref.store(false, Ordering::Relaxed);
                        filelist_widget_ref.select_all();
                        filelist_widget_ref.delete_selection();
                    }

                    let file_paths: Vec<PathBuf> = paths
                        .map(|p| PathBuf::from(p))
                        .filter(|p| {
                            if !p.exists() {
                                return false;
                            }
                            !filelist_widget_ref.contains_path(p.to_path_buf())
                        })
                        .collect();
                    if add_to_conversion_queue(file_paths, &mut filelist_widget_ref, &mut scroll_ref) {
                        if !button_convert.active() {
                            button_convert.activate();
                            file_actions_group_ref.set_damage(true);
                            select_all_frame_ref.borrow_mut().show();
                            deselect_all_frame_ref.borrow_mut().show();

                            file_actions_group_ref.resize(
                                file_actions_group_ref.x(),
                                file_actions_group_ref.y(),
                                150,
                                40,
                            );
                            file_actions_group_ref.set_damage(true);
                            file_actions_group_ref.redraw();
                        }
                    }
                }

                true
            }
            enums::Event::Push => {
                let mut dlg = dialog::FileDialog::new(dialog::FileDialogType::BrowseMultiFile);
                dlg.set_title("Select suspicious file(s)");
                dlg.show();

                let file_paths: Vec<PathBuf> = dlg
                    .filenames()
                    .iter()
                    .map(|p| p.clone())
                    .filter(|p| {
                        if !p.exists() {
                            return false;
                        }
                        !filelist_widget_ref.contains_path(p.to_path_buf())
                    })
                    .collect();

                if is_converting_ref.load(Ordering::Relaxed) {
                    is_converting_ref.store(false, Ordering::Relaxed);
                    filelist_widget_ref.select_all();
                    filelist_widget_ref.delete_selection();
                }

                if add_to_conversion_queue(file_paths, &mut filelist_widget_ref, &mut scroll_ref) {
                    if !button_convert.active() {
                        button_convert.activate();
                        file_actions_group_ref.set_damage(true);
                        select_all_frame_ref.borrow_mut().show();
                        deselect_all_frame_ref.borrow_mut().show();

                        file_actions_group_ref.resize(
                            file_actions_group_ref.x(),
                            file_actions_group_ref.y(),
                            150,
                            40,
                        );
                        file_actions_group_ref.set_damage(true);
                        file_actions_group_ref.redraw();
                    }
                }
                true
            }
            _ => false,
        }
    });

    wind.handle({
        let mut file_actions_group_ref = file_actions_group.clone();
        let convert_group_ref = convert_group.clone();
        let settings_group_ref = settings_group.clone();
        let mut top_group_ref = top_group.clone();
        let mut scroll_ref = scroll.clone();
        let mut button_convert_ref = button_convertx.clone();
        let select_all_frame_ref = select_all_frame.clone();
        let deselect_all_frame_ref = deselect_all_frame.clone();
        let mut group_ocr_language = row_ocr_language.clone();
        let ocr_language_list_ref = ocr_language_list.clone();
        let input_oci_image_ref = input_oci_image.clone();
        let mut output_oci_image_ref = output_oci_image.clone();
        let mut row_oci_image_ref = row_oci_image.clone();
        let mut row_openwith_ref = row_openwith.clone();
        let button_browse_for_pdf_app_copy2 = button_browse_for_pdf_appx.clone();
        let pdf_viewer_list_ref = pdf_viewer_list.clone();
        let mut row_inputloc_ref = row_inputloc.clone();
        let mut checkbutton_custom_output_ref = checkbutton_custom_output.clone();
        let mut checkbutton_ocr_lang_ref = checkbutton_ocr_lang.clone();
        let input_outputloc2 = input_outputloc.clone();
        let mut filelist_widget_ref = filelist_widget.clone();
        let mut messages_frame_ref = messages_frame.clone();

        move |w, ev| match ev {
            enums::Event::Resize => {
                top_group_ref.resize(
                    WIDGET_GAP,
                    WIDGET_GAP,
                    w.w() - (WIDGET_GAP * 2),
                    30,
                );

                convert_togglebutton.resize(WIDGET_GAP, top_group_ref.y() + WIDGET_GAP, 80, 30);
                settings_togglebutton.resize(WIDGET_GAP, top_group_ref.y() + WIDGET_GAP, 80, 30);
                let new_y = top_group_ref.y() + top_group_ref.h() + WIDGET_GAP;

                let scroller_height = ((w.h() - top_group_ref.h() + WIDGET_GAP) as f64 * 0.5) as i32;

                convert_group_ref.borrow_mut().resize(
                    WIDGET_GAP,
                    new_y,
                    w.w() - (WIDGET_GAP * 2),
                    w.h() - top_group_ref.h() + WIDGET_GAP,
                );

                settings_group_ref.borrow_mut().resize(
                    WIDGET_GAP,
                    new_y,
                    w.w() - (WIDGET_GAP * 2),
                    w.h() - top_group_ref.h() + WIDGET_GAP,
                );
                scroll_ref.resize(
                    scroll_ref.x(),
                    scroll_ref.y(),
                    w.w() - (WIDGET_GAP * 3),
                    scroller_height,
                );

                let wval = w.w() - (WIDGET_GAP * 3);

                filelist_widget_ref.resize(scroll_ref.x(), scroll_ref.y(), wval, 0);

                scroll_ref.redraw();

                let xx = ocr_language_list_ref.borrow_mut().x();

                row_oci_image_ref.resize(
                    row_oci_image.x(),
                    row_oci_image.y(),
                    w.w() - (WIDGET_GAP * 2),
                    row_oci_image.h(),
                );
                row_inputloc_ref.resize(
                    row_inputloc_ref.x(),
                    row_inputloc_ref.y(),
                    w.w() - (WIDGET_GAP * 2),
                    row_inputloc_ref.h(),
                );
                row_openwith_ref.resize(
                    row_inputloc_ref.x() + WIDGET_GAP / 2,
                    row_openwith.y(),
                    w.w() - (WIDGET_GAP * 2),
                    row_openwith.h(),
                );
                checkbutton_custom_output_ref.resize(
                    xx,
                    checkbutton_custom_output_ref.y(),
                    checkbutton_ocr_lang_ref.w(),
                    checkbutton_custom_output_ref.h(),
                );
                checkbutton_ocr_lang_ref.resize(
                    xx,
                    checkbutton_ocr_lang_ref.y(),
                    checkbutton_ocr_lang_ref.w(),
                    checkbutton_custom_output_ref.h(),
                );

                let ocw = w.w() - (WIDGET_GAP * 3) - checkbutton_ocr_lang.w();
                let och = (w.h() as f64 * 0.5) as i32;

                output_oci_image_ref.resize(
                    checkbutton_ocr_lang_ref.x(),
                    output_oci_image_ref.y(),
                    checkbutton_ocr_lang_ref.w(),
                    output_oci_image_ref.h(),
                );

                group_ocr_language.resize(
                    xx,
                    group_ocr_language.y(),
                    w.w() - (WIDGET_GAP * 4),
                    och,
                );

                let yy = ocr_language_list_ref.borrow_mut().y();
                ocr_language_list_ref.borrow_mut().resize(
                    xx,
                    yy,
                    ocw,
                    och - (WIDGET_GAP * 2),
                );

                let input_oci_image_2_y = input_oci_image.borrow().y();
                let input_oci_image_2_h = input_oci_image.borrow().h();
                input_oci_image_ref.borrow_mut().resize(xx, input_oci_image_2_y, ocw, input_oci_image_2_h);
                let yyy = input_outputloc.borrow().y();
                let hhh = input_outputloc.borrow().h();
                input_outputloc2.borrow_mut().resize(xx, yyy, ocw, hhh);

                let yyyy = pdf_viewer_list_ref.borrow().y();
                let hhhh = pdf_viewer_list_ref.borrow().h();
                pdf_viewer_list_ref.borrow_mut().resize(
                    xx,
                    yyyy,
                    ocw - button_browse_for_pdf_app_copy2.borrow().w() - WIDGET_GAP ,
                    hhhh
                );

                messages_frame_ref.resize(
                    messages_frame_ref.x(),
                    messages_frame_ref.y(),
                    w.w() - (WIDGET_GAP * 4),
                    60,
                );

                scroll_ref.redraw();
                true
            }
            _ => {
                if ev.bits() == FileListWidgetEvent::SELECTION_CHANGED {
                    let selection = filelist_widget_ref.selected_indices();
                    let empty_selection = selection.is_empty();

                    if empty_selection && button_delete_file.active() {
                        button_delete_file.deactivate();
                    } else if !empty_selection && !button_delete_file.active() {
                        button_delete_file.activate();
                    }

                    if !filelist_widget_ref.has_files() {
                        file_actions_group_ref.redraw();
                        button_convert_ref.deactivate();
                        select_all_frame_ref.borrow_mut().hide();
                        deselect_all_frame_ref.borrow_mut().hide();
                    }

                    filelist_widget_ref.container.redraw();
                    scroll_ref.redraw();
                    true
                } else if ev.bits() == FileListWidgetEvent::ALL_SELECTED {
                    filelist_widget_ref.select_all();
                    true
                } else if ev.bits() == FileListWidgetEvent::ALL_DESELECTED {
                    filelist_widget_ref.deselect_all();
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
    wind.resize(wind.x(), wind.y(), 680, 600);

    if autoconvert {
        button_convertxx.do_callback();
    }

    match app.run() {
        Ok(_) => Ok(()),
        Err(ex) => Err(ex.into()),
    }
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
    match which::which("open") {
        Ok(open_cmd) => match Command::new(open_cmd).arg("-a").arg(cmd).arg(input).spawn() {
            Ok(mut child_proc) => {
                if let Ok(exit_status) = child_proc.wait() {
                    if exit_status.success() {
                        Ok(())
                    } else {
                        Err("Could not open PDF file!".into())
                    }
                } else {
                    Err("Could not run PDF viewer!".into())
                }
            }
            Err(ex) => Err(ex.into()),
        },
        Err(ex) => Err(ex.into()),
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
            let cf_key =
                CFStringCreateWithCString(std::ptr::null(), c_key.as_ptr(), kCFStringEncodingUTF8);
            let result = LSCopyAllRoleHandlersForContentType(cf_key, kLSRolesAll);
            let count = CFArrayGetCount(result);

            for i in 0..count - 1 {
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
                                    if let (Ok(r_app_name), Ok(r_app_url)) = (
                                        percent_decode(basename.as_bytes()).decode_utf8(),
                                        percent_decode(app_url.as_bytes()).decode_utf8(),
                                    ) {
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
