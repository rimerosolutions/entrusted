#![windows_subsystem = "windows"]

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering, AtomicI32};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use fltk::{
    app, browser, button, dialog, draw, enums, frame, group, input, misc, prelude::*, text, window, image, menu
};

use entrusted_l10n as l10n;
mod common;
mod config;
mod container;

const WIDGET_GAP: i32 = 20;
const ELLIPSIS: &str  = "...";

const ICON_SAVE: &[u8]     = include_bytes!("../../images/Save_icon.png");
const ICON_FRAME: &[u8]    = include_bytes!("../../images/Entrusted_icon.png");
const ICON_PASSWORD: &[u8] = include_bytes!("../../images/Password_icon.png");

const FILELIST_ROW_STATUS_PENDING    :&str = "Pending";
const FILELIST_ROW_STATUS_INPROGRESS :&str = "InProgress";
const FILELIST_ROW_STATUS_SUCCEEDED  :&str = "Succeeded";
const FILELIST_ROW_STATUS_FAILED     :&str = "Failed";
const FILELIST_ROW_STATUS_CANCELLED  :&str = "Cancelled";

const FILELIST_ROW_COLOR_PENDING: enums::Color    = enums::Color::Magenta;
const FILELIST_ROW_COLOR_INPROGRESS: enums::Color = enums::Color::DarkYellow;
const FILELIST_ROW_COLOR_SUCCEEDED: enums::Color  = enums::Color::DarkGreen;
const FILELIST_ROW_COLOR_FAILED: enums::Color     = enums::Color::Red;
const FILELIST_ROW_COLOR_CANCELLED: enums::Color  = enums::Color::from_rgb(153, 0, 0);

const TAB_COLOR_PUSHED_FOREGROUND: enums::Color = enums::Color::White;
const TAB_COLOR_PUSHED_BACKGROUND: enums::Color = enums::Color::from_rgb(90, 90, 90);

const EVENT_ID_SELECTION_CHANGED: i32 = 50;
const EVENT_ID_ALL_SELECTED: i32      = 51;
const EVENT_ID_ALL_DESELECTED: i32    = 52;

#[derive(Clone)]
struct GuiEventSender {
    tx: mpsc::Sender<common::AppEvent>
}

impl common::EventSender for GuiEventSender {
    fn send(&self, evt: crate::common::AppEvent) -> Result<(), mpsc::SendError<crate::common::AppEvent>> {
        app::sleep(0.01);
        app::awake();
        self.tx.send(evt)
    }

    fn clone_box(&self) -> Box<dyn common::EventSender> {
        Box::new(self.clone())
    }
}

#[derive(Clone)]
struct ConversionTask {
    input_path: PathBuf,
    output_path: PathBuf,
    options: common::ConvertOptions,
}

#[derive(Clone)]
struct FileListRow {
    container: group::Pack,
    file: PathBuf,
    opt_output_file: Rc<RefCell<Option<String>>>,
    password_button: button::Button,
    output_file_button: button::Button,
    checkbox: button::CheckButton,
    progressbar: misc::Progress,
    status: frame::Frame,
    messages: group::Pack,
    open_link: HyperLink,
    logs_link: HyperLink,
    logs: Rc<RefCell<Vec<String>>>,
    opt_passwd: Rc<RefCell<Option<String>>>,
    viewer_app_option: Rc<RefCell<Option<String>>>
}

impl FileListRow {
    pub fn set_viewer_app(&mut self, viewer_app: String) {
        self.viewer_app_option.replace(Some(viewer_app));
    }
    
    pub fn deactivate_controls(&mut self) {
        self.checkbox.deactivate();
        self.password_button.deactivate();
        self.output_file_button.deactivate();
    }

    pub fn update_progress(&mut self, data: String, percent_complete: usize) {
        self.progressbar.set_value(percent_complete as f64);
        self.progressbar.set_label(&format!("{}%", percent_complete));
        self.logs.borrow_mut().push(data);
    }

    pub fn mark_as_cancelled(&mut self) {
        self.status.set_label_color(FILELIST_ROW_COLOR_CANCELLED);
        self.status.set_label(FILELIST_ROW_STATUS_CANCELLED);
    }
}

#[derive(Clone)]
struct HyperLink {
    container: frame::Frame,
    disabled: bool,
}

impl Deref for HyperLink {
    type Target = frame::Frame;

    fn deref(&self) -> &Self::Target {
        &self.container
    }
}

impl DerefMut for HyperLink {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.container
    }
}

impl HyperLink {
    fn new(w: i32, h: i32, lbl: &str) -> Self {
        let mut container = frame::Frame::default()
            .with_size(w, h)
            .with_label(lbl)
            .with_align(enums::Align::Inside | enums::Align::Left);
        container.set_label_color(enums::Color::Blue);

        container.draw({
            move |wid| {
                let (lw, lh) = draw::measure(&wid.label(), true);
                let old_color = draw::get_color();

                let line_color = if wid.active() {
                    wid.label_color()
                } else {
                    wid.label_color().inactive()
                };

                draw::set_draw_color(line_color);
                draw::draw_line(wid.x() + 3, wid.y() + lh/2 + wid.h() / 2, wid.x() + lw + 2, wid.y() + lh/2 + wid.h() / 2);
                draw::set_draw_color(old_color);
            }
        });

        let disabled = false;

        Self {
            container,
            disabled
        }
    }

    fn disable(&mut self) {
        self.disabled = true;
        self.container.set_label_color(enums::Color::from_rgb(82, 82, 82));
    }

    fn is_disabled(&self) -> bool {
        self.disabled
    }
}

fn inside_hyperlink_bounds<W: WidgetExt>(coords: (i32, i32), wid: &mut W) -> bool {
    let (lw, lh) = draw::measure(&wid.label(), true);
    let bounds = (wid.x(), wid.y() + wid.h() / 2 - lh/2, lw + 2, lh);

    coords.0 >= bounds.0 && coords.0 <= (bounds.0 + bounds.2) && coords.1 >= bounds.1 && coords.1 <= (bounds.1 + bounds.3)
}

#[derive(Clone)]
struct FileListWidget {
    container: group::Pack,
    selected_indices: Rc<RefCell<Vec<usize>>>,
    rows: Rc<RefCell<Vec<FileListRow>>>,
    trans: l10n::Translations,
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

fn selected_ocr_langcodes(ocr_languages_by_lang: &HashMap<String, &str>, drop_down: &browser::MultiBrowser) -> Vec<String> {
    let mut ret: Vec<String> = Vec::new();

    if drop_down.selected_text().is_none() {
        return ret;
    } else {
        for i in 0..drop_down.size() {
            if drop_down.selected(i) {
                if let Some(selected_lang) = drop_down.text(i) {
                    if let Some(langcode) = ocr_languages_by_lang.get(&selected_lang) {
                        ret.push(langcode.to_string());
                    }
                }
            }
        }
    }

    ret
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

fn paint_underline<W: WidgetExt>(wid: &mut W) {
    if wid.visible() && wid.active() {
        let (lw, _) = draw::measure(&wid.label(), true);
        let old_color = draw::get_color();
        draw::set_draw_color(wid.label_color());
        draw::draw_line(wid.x() + 3, wid.y() + wid.h(), wid.x() + lw, wid.y() + wid.h());
        draw::set_draw_color(old_color);
    }
}

fn row_to_task(active_ociimage_option: &String, image_quality: String,  active_ocrlang_option: &Option<String>, active_file_suffix: &str, active_seccomp: bool, active_row: &FileListRow) -> ConversionTask {
    let input_path = active_row.file.clone();

    let output_path = if let Some(custom_output_path) = active_row.opt_output_file.borrow().clone() {
        PathBuf::from(custom_output_path)
    } else {
        common::default_output_path(input_path.clone(), active_file_suffix.to_string()).unwrap()
    };

    active_row.opt_output_file.replace(Some(output_path.display().to_string()));

    let opt_row_passwd = active_row.opt_passwd.borrow().clone();
    let options = common::ConvertOptions::new(
        active_ociimage_option.to_owned(),
        common::LOG_FORMAT_JSON.to_string(),
        image_quality,
        active_ocrlang_option.to_owned(),
        opt_row_passwd,
        active_seccomp
    );

    ConversionTask {
        input_path,
        output_path,
        options,
    }
}

fn show_dialog_updates(parent_window_bounds: (i32, i32, i32, i32), trans: l10n::Translations) {
    let (x, y, w, h) = parent_window_bounds;

    match common::update_check(&trans) {
        Ok(opt_new_release) => {
            if let Some(new_release) = opt_new_release {
                show_dialog_newrelease(parent_window_bounds, new_release, trans);
            } else {
                let msg  = trans.gettext("No updates available at this time!");
                let (lbl_width, lbl_height)  = draw::measure(&msg, true);
                dialog::alert(x +( w/2) - (lbl_width/2), y +  (h / 2) - (lbl_height / 2), &msg);
            }
        },
        Err(ex) => {
            let msg = trans.gettext_fmt("Could not check for updates, please try later.\n{0}", vec![&ex.to_string()]);
            let (lbl_width, lbl_height)  = draw::measure(&msg, true);
            dialog::alert(x +( w/2) - (lbl_width/2), y +  (h / 2) - (lbl_height / 2), &msg);
        }
    }
}

fn show_dialog_newrelease(parent_window_bounds: (i32, i32, i32, i32), release_info: common::ReleaseInfo, trans: l10n::Translations) {
    let wind_w = 450;
    let wind_h = 100;
    let wind_x = parent_window_bounds.0 + (parent_window_bounds.2 / 2) - (wind_w / 2);
    let wind_y = parent_window_bounds.1 + (parent_window_bounds.3 / 2) - (wind_h / 2);

    let mut win = window::Window::default()
        .with_size(wind_w, wind_h)
        .with_pos(wind_x, wind_y)
        .with_label(&trans.gettext_fmt("Version {0} is out!", vec![&release_info.tag_name]));

    win.begin();
    win.make_resizable(true);
    win.make_modal(true);

    let mut grp = group::Pack::default()
        .with_pos(WIDGET_GAP, WIDGET_GAP / 2)
        .with_size(wind_w - (WIDGET_GAP * 2), wind_h - (WIDGET_GAP * 2))
        .center_of(&win)
        .with_type(group::PackType::Vertical);
    grp.set_spacing(WIDGET_GAP);

    frame::Frame::default()
        .with_size(350, 20)
        .with_label(&trans.gettext("Please get the new version!"))
        .with_align(enums::Align::Inside | enums::Align::Left);

    let mut frame_website = frame::Frame::default()
        .with_size(350, 20)
        .with_label(&release_info.html_url)
        .with_align(enums::Align::Inside | enums::Align::Left);

    frame_website.set_label_color(enums::Color::Blue);

    frame_website.draw({
        move |wid| {
            if wid.visible() && wid.active() {
                let (lw, lh) = draw::measure(&wid.label(), true);
                let old_color = draw::get_color();
                draw::set_draw_color(wid.label_color());
                let ypos = wid.y() + wid.h()/2 + lh/2;
                draw::draw_line(wid.x() + 3, ypos, wid.x() + lw, ypos);
                draw::set_draw_color(old_color);
            }
        }
    });

    frame_website.handle({
        let wind_ref = win.clone();
        let trans_ref = trans.clone();

        move |wid, ev| match ev {
            enums::Event::Push => {
                if let Err(ex) = open_document(&wid.label(), "text/html", &trans) {
                    let err_text = trans_ref.gettext_fmt("Could not open URL: {0}, {1}", vec![&wid.label(), &ex.to_string()]);
                    dialog::alert(wind_ref.x(), wind_ref.y() + wind_ref.height() / 2, &err_text);
                }

                true
            }
            _ => false,
        }
    });

    grp.end();
    win.end();

    win.show();

    while win.shown() {
        app::wait();
    }
}

fn show_dialog_help(parent_window_bounds: (i32, i32, i32, i32), trans: l10n::Translations) {
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
    win.make_modal(true);

    let mut grp = group::Pack::default()
        .with_pos(WIDGET_GAP, WIDGET_GAP / 2)
        .with_size(wind_w - (WIDGET_GAP * 2), wind_h - (WIDGET_GAP * 2))
        .center_of(&win)
        .with_type(group::PackType::Vertical);
    grp.set_spacing(WIDGET_GAP / 4);

    let label_container_solution = format!("{}{}{}",
                                           "The program requires a container solution: \n",
                                           "- Docker (Windows, Linux, Mac OS)\n",
                                           "- or Podman (Linux)");

    frame::Frame::default()
        .with_size(350, 80)
        .center_of(&win)
        .with_label(&trans.gettext(&label_container_solution))
        .with_align(enums::Align::Inside | enums::Align::Left);

    let label_supported_docs = format!("Supported document types: \n- {}\n- {}\n- {}\n- {}\n- {}\n- {}",
                                       "Images (.jpg, .jpeg, .gif, .png, .tif, .tiff)",
                                       "Document Graphics (.odg)",
                                       "Text Documents (.rtf, .doc, .docx, .odt)",
                                       "Spreadsheets (.xls, .xlsx, .ods)",
                                       "Presentations (.ppt, .pptx, .odp)",
                                       "PDF files (.pdf)"
    );

    frame::Frame::default()
        .with_size(350, 130)
        .with_label(&trans.gettext(&label_supported_docs))
        .with_align(enums::Align::Inside | enums::Align::Left);

    frame::Frame::default()
        .with_size(350, 20)
        .with_label(&trans.gettext("For more information, please visit:"))
        .with_align(enums::Align::Inside | enums::Align::Left);

    let label_website = "https://github.com/rimerosolutions/entrusted";

    let mut frame_website = frame::Frame::default()
        .with_size(350, 20)
        .with_label(label_website)
        .with_align(enums::Align::Inside | enums::Align::Left);

    frame_website.set_label_color(enums::Color::Blue);

    frame_website.draw({
        move |wid| {
            if wid.visible() && wid.active() {
                let (lw, lh) = draw::measure(&wid.label(), true);
                let old_color = draw::get_color();
                draw::set_draw_color(wid.label_color());
                let ypos = wid.y() + wid.h()/2 + lh/2;
                draw::draw_line(wid.x() + 3, ypos, wid.x() + lw, ypos);
                draw::set_draw_color(old_color);
            }
        }
    });

    frame_website.handle({
        let wind_ref = win.clone();

        move |wid, ev| match ev {
            enums::Event::Push => {
                if let Err(ex) = open_document(&wid.label(), "text/html", &trans) {
                    let err_text = &trans.gettext_fmt("Could not open URL: {0}, {1}", vec![&wid.label(), &ex.to_string()]);
                    dialog::alert(wind_ref.x(), wind_ref.y() + wind_ref.height() / 2, err_text);
                }

                true
            }
            _ => false,
        }
    });

    grp.end();

    win.end();
    win.show();

    while win.shown() {
        app::wait();
    }
}

fn filelist_column_widths(w: i32) -> (i32, i32, i32, i32, i32, i32) {
    let width_password    = 40;
    let width_output_file = 40;
    let active_w = w - width_password - width_output_file - (WIDGET_GAP * 5);

    let width_checkbox    = (active_w as f64 * 0.35)  as i32;
    let width_progressbar = (active_w as f64 * 0.20)  as i32;
    let width_status      = (active_w as f64 * 0.20)  as i32;
    let width_logs        = (active_w as f64 * 0.25)  as i32;

    (width_output_file, width_password, width_checkbox, width_progressbar, width_status, width_logs)
}

impl FileListWidget {
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
                active_row.container.resize(active_row.container.x(), active_row.container.y(), w, active_row.container.h());
                active_row.container.redraw();

                let mut col_widgets: Vec<Box<dyn WidgetExt>> = vec![
                    Box::new(row.password_button.clone()),
                    Box::new(row.output_file_button.clone()),
                    Box::new(row.checkbox.clone()),
                    Box::new(row.progressbar.clone()),
                    Box::new(row.status.clone()),
                    Box::new(row.messages.clone())
                ];

                let mut x_position = x;
                let col_count = col_widths.len();

                for i in 0..col_count {
                    let wid = &mut col_widgets[i];
                    wid.resize(x_position, wid.y(), col_widths[i], wid.h());
                    x_position = x_position + col_widths[i] + WIDGET_GAP;

                    if i == 2 {
                        if let Some(path_name) = row.file.file_name().and_then(|x| x.to_str()) {
                            wid.set_label(&clip_text(path_name, width_checkbox));
                        }
                    } else if i == col_count - 1 {
                        if let Some(gg) = wid.as_group() {
                            let cc_width = (col_widths[i] - WIDGET_GAP/2) / 2;
                            let mut startx = wid.x();

                            for j in 0..gg.children() {
                                if let Some(mut cc) = gg.child(j) {
                                    cc.resize(startx, cc.y(), cc_width, wid.h());
                                    startx += cc_width + (WIDGET_GAP/2);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn contains_path(&self, p: &PathBuf) -> bool {
        self.rows
            .borrow()
            .iter()
            .any(|row| *row.file == *p)
    }

    pub fn has_files(&self) -> bool {
        !self.rows.borrow().is_empty()
    }

    fn toggle_selection(&mut self, select: bool) -> bool {
        let mut selection_changed = false;

        for row in self.rows.borrow().iter() {
            if row.checkbox.active() && row.checkbox.is_checked() != select {
                row.checkbox.set_checked(select);
                selection_changed = true;
            }
        }

        selection_changed
    }

    pub fn selected_indices(&self) -> Vec<usize> {
        self.selected_indices
            .borrow()
            .iter()
            .copied()
            .collect()
    }

    pub fn select_all(&mut self) {
        if self.toggle_selection(true) {
            let row_count = self.rows.borrow().len();
            self.selected_indices.borrow_mut().extend(0..row_count);
            let _ = app::handle_main(EVENT_ID_SELECTION_CHANGED);
        }
    }

    pub fn deselect_all(&mut self) {
        if self.toggle_selection(false) {
            self.selected_indices.borrow_mut().clear();
            let _ = app::handle_main(EVENT_ID_SELECTION_CHANGED);
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

        let _ = app::handle_main(EVENT_ID_SELECTION_CHANGED);
    }

    pub fn delete_selection(&mut self) {
        self.selected_indices.borrow_mut().sort();

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

        let _ = app::handle_main(EVENT_ID_SELECTION_CHANGED);
    }

    pub fn add_file(&mut self, path: PathBuf) {
        let trans = &self.trans;

        let ww = self.container.w();

        let (width_password, width_output_file, width_checkbox, width_progressbar, width_status, width_logs) = filelist_column_widths(ww);

        let mut row = group::Pack::default()
            .with_size(ww, 40)
            .with_type(group::PackType::Horizontal);

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

        let path_name = path.file_name().and_then(|x| x.to_str()).unwrap().to_string();
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
        status_frame.set_label_color(FILELIST_ROW_COLOR_PENDING);

        let mut messages = group::Pack::default()
            .with_size(width_logs, 40)
            .with_type(group::PackType::Horizontal);
        messages.set_spacing(WIDGET_GAP/2);

        let mut open_link = HyperLink::new(width_logs, row_height, "");
        open_link.deactivate();

        let mut logs_link = HyperLink::new(width_logs, row_height, "");
        logs_link.deactivate();

        messages.end();

        row.end();

        let file_list_row = FileListRow {
            container: row.clone(),
            password_button: password_frame.clone(),
            checkbox: check_buttonx2,
            progressbar,
            messages,
            status: status_frame.clone(),
            open_link: open_link.clone(),
            logs_link: logs_link.clone(),
            logs: Rc::new(RefCell::new(vec![])),
            file: path.clone(),
            output_file_button: output_file_button.clone(),
            opt_output_file: Rc::new(RefCell::new(None)),
            opt_passwd: Rc::new(RefCell::new(None)),
            viewer_app_option: Rc::new(RefCell::new(None)),
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

                    win.make_modal(true);
                    win.make_resizable(true);

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

                        move |_| {
                            let mut dlg = dialog::FileDialog::new(dialog::FileDialogType::BrowseSaveFile);
                            dlg.set_title(&select_pdffile_msg);
                            dlg.show();

                            if !dlg.filename().as_os_str().is_empty() {
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

                        let opt_output_file = opt_output_file.clone();

                        move |_| {
                            let output_filename = outputfile_input_rc.borrow().value();

                            if !output_filename.trim().is_empty() {
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
                    win.show();

                    while win.shown() {
                        app::wait();
                    }
                }
            }
        });

        fn paint_highlight<W: WidgetExt>(wid: &mut W, opt_isset: bool) {
            if wid.active() {
                let old_color = draw::get_color();

                let current_color = if opt_isset {
                    enums::Color::DarkRed
                } else {
                    enums::Color::Yellow
                };

                draw::set_draw_color(current_color);

                for i in 1..3 {
                    draw::draw_rect(wid.x() + i, wid.y() + i, wid.w() - i - i, wid.h() - i - i);
                }

                draw::set_draw_color(old_color);
            }
        }

        output_file_button.draw({
            let opt_current = file_list_row.opt_output_file.clone();

            move |wid| {
                paint_highlight(wid, opt_current.borrow().is_some());
            }
        });

        let dialog_title = trans.gettext("Logs");
        let close_button_label = trans.gettext("Close");

        password_frame.draw({
            let opt_current = file_list_row.opt_passwd.clone();

            move |wid| {
                paint_highlight(wid, opt_current.borrow().is_some());
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
                                Some(input_value)
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

        open_link.handle({
            let trans_ref = trans.clone();
            let active_row = file_list_row.clone();

            move |wid, ev| match ev {
                enums::Event::Push => {
                    if inside_hyperlink_bounds(app::event_coords(), wid) {
                        if let Some(output_path) = active_row.opt_output_file.borrow().clone() {
                            let opt_viewer_app = active_row.viewer_app_option.borrow().clone();                            

                            if let Some(viewer_app) = opt_viewer_app {
                                if let Err(ex) = pdf_open_with(viewer_app, PathBuf::from(&output_path), &trans_ref) {
                                    let err_text = format!("{}\n{}.", &trans_ref.gettext("Could not open PDF file!"), ex);

                                    if let Some(wind_ref) = app::first_window() {
                                        dialog::alert(wind_ref.x(), wind_ref.y() + wind_ref.height() / 2, &err_text);
                                    }
                                }
                            } else if let Err(ex) = open_document(&output_path, "application/pdf", &trans_ref) {
                                let err_text = ex.to_string();
                                let err_msg = &trans_ref.gettext_fmt("Couldn't open PDF file in default application! {0}.", vec![&err_text]);

                                if let Some(wind_ref) = app::first_window() {
                                    dialog::alert(wind_ref.x(), wind_ref.y() + wind_ref.height() / 2, err_msg);
                                }
                            }
                        }
                    }

                    true
                }
                _ => false,
            }
        });

        logs_link.handle({
            let active_row = file_list_row.clone();

            move |wid, ev| match ev {
                enums::Event::Push => {
                    if !inside_hyperlink_bounds(app::event_coords(), wid) {
                        return true;
                    }

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

                        dialog.end();
                        dialog.make_modal(true);
                        dialog.make_resizable(true);
                        dialog.show();

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

                        while dialog.shown() {
                            app::wait();
                        }
                    }

                    true
                },
                _ => false

            }
        });

        selectrow_checkbutton.set_callback({
            let selfie = self.clone();

            move |b| {
                let idx = selfie.row_index(&path);

                if idx != -1 {
                    if b.is_checked() {
                        selfie.selected_indices.borrow_mut().push(idx as usize);
                    } else {
                        selfie.selected_indices.borrow_mut().retain( |x| *x != idx as usize);
                    }

                    let _ = app::handle_main(EVENT_ID_SELECTION_CHANGED);
                }
            }
        });

        self.container.add(&row);
        row.auto_layout();
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

    pub fn set_viewer_app(&mut self, viewer_app: String) {
        for row in self.rows.borrow_mut().iter() {
            let mut active_row = row.clone();
            active_row.set_viewer_app(viewer_app.clone());
        }
    }
    
    fn deactivate_controls(&mut self) {
        for row in self.rows.borrow_mut().iter() {
            let mut active_row = row.clone();
            active_row.deactivate_controls();
        }
    }

    fn ypos(&self, row_index: usize) -> i32{
        let mut rows = self.rows.borrow_mut();
        let row = &mut rows[row_index];
        row.checkbox.y()
    }

    fn update_status(&self, row_index: usize, file_status: &str, status_color: enums::Color) {
        let mut rows = self.rows.borrow_mut();
        let row = &mut rows[row_index];

        row.status.set_label_color(status_color);
        row.status.set_label(file_status);

        if file_status == FILELIST_ROW_STATUS_SUCCEEDED || file_status == FILELIST_ROW_STATUS_FAILED {
            row.progressbar.set_label("100%");
            row.progressbar.set_value(100.0);

            if file_status == FILELIST_ROW_STATUS_FAILED {
                row.open_link.disable();
            }

            row.open_link.set_label(&self.trans.gettext("Open"));
            row.logs_link.set_label(&self.trans.gettext("Logs"));
        }
    }

    fn activate_logs_links(&self) {
        for row in self.rows.borrow_mut().iter_mut() {
            if !row.open_link.is_disabled() {
                row.open_link.activate();
            }

            row.logs_link.activate();
        }
    }

    fn mark_as_cancelled_starting_from_index(&self, completed_count: usize) {
        let mut rows = self.rows.borrow_mut();

        for row_index in completed_count..rows.len() {
            let row = &mut rows[row_index];
            row.mark_as_cancelled();
        }
    }

    fn update_progress(&self, row_index: usize, data: String, percent_complete: usize) {
        let mut rows = self.rows.borrow_mut();
        let row = &mut rows[row_index];
        row.update_progress(data, percent_complete);
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    l10n::load_translations(incl_gettext_files!("en", "fr"));

    let locale = if let Ok(selected_locale) = env::var(l10n::ENV_VAR_ENTRUSTED_LANGID) {
        selected_locale
    } else {
        l10n::sys_locale()
    };

    let trans = l10n::new_translations(locale);
    let trans_ref = trans.clone();

    let selectfiles_dialog_title = trans.gettext("Select 'potentially suspicious' file(s)");
    let appconfig_ret = config::load_config();
    let appconfig: config::AppConfig = appconfig_ret.unwrap_or_default();

    let current_row_idx = Arc::new(AtomicI32::new(0));
    let converstion_stop_requested = Arc::new(AtomicBool::new(false));
    let is_converting = Arc::new(AtomicBool::new(false));
    let conversion_is_active = Arc::new(AtomicBool::new(false));

    let app = app::App::default().with_scheme(app::Scheme::Gleam);
    let (tx, rx) = mpsc::channel::<common::AppEvent>();
    let (app_tx, app_rx) = app::channel::<common::AppEvent>();

    #[cfg(target_os = "macos")]
    let (_, app_rx_appleevents) = app::channel::<String>();

    let wind_title = format!(
        "{} {}",
        option_env!("CARGO_PKG_NAME").unwrap_or("Unknown"),
        option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown")
    );

    let mut wind = window::Window::default()
        .with_size(739, 560)
        .center_screen()
        .with_label(&wind_title);

    wind.set_xclass("entrusted");

    if let Ok(frame_icon) = image::PngImage::from_data(ICON_FRAME) {
        wind.set_icon(Some(frame_icon));
    }

    wind.make_resizable(true);
    let default_wincolor = wind.color();

    let mut top_group = group::Pack::default()
        .with_pos(20, 20)
        .with_size(680, 25)
        .with_type(group::PackType::Horizontal)
        .with_align(enums::Align::Inside | enums::Align::Right);
    top_group.set_spacing(WIDGET_GAP);

    let tabs =  group::Pack::default()
        .with_size(240, 25)
        .with_type(group::PackType::Horizontal)
        .with_align(enums::Align::Inside | enums::Align::Left);
    let mut tabsettings_button = button::Button::default()
        .with_size(120, 20)
        .with_label(&trans.gettext("Settings"));
    tabsettings_button.set_label_color(enums::Color::Black);

    let mut tabconvert_button = button::Button::default()
        .with_size(120, 20)
        .with_label(&trans.gettext("Convert"));
    tabconvert_button.set_color(TAB_COLOR_PUSHED_BACKGROUND);
    tabconvert_button.set_label_color(TAB_COLOR_PUSHED_FOREGROUND);
    tabs.end();

    let mut helpinfo_pack = group::Pack::default()
        .with_size(400, 20)
        .with_type(group::PackType::Horizontal)
        .with_align(enums::Align::Inside | enums::Align::Right);
    helpinfo_pack.set_spacing(WIDGET_GAP/5);

    let mut spacer_frame = frame::Frame::default()
        .with_size(360 - WIDGET_GAP, 20);

    let mut updatechecks_button = button::Button::default()
        .with_size(20, 20);
    updatechecks_button.set_tooltip(&trans_ref.gettext("Check for updates"));

    let mut helpinfo_button = button::Button::default()
        .with_size(20, 20);

    helpinfo_button.draw({
        move |wid| {
            let old_color = draw::get_color();

            let new_color = if wid.active() {
                enums::Color::Blue
            } else {
                enums::Color::Blue.inactive()
            };

            draw::set_draw_color(new_color);
            draw::draw_rect_fill(wid.x(), wid.y(), wid.w(), wid.h(), new_color);
            draw::set_draw_color(enums::Color::White);

            let margin = 4;

            for i in 0..2 {
                draw::draw_line(wid.x() + (margin * 3) - 1, wid.y() + margin + i,  wid.x() + wid.w() - margin, wid.y() + margin + i);
            }

            for i in 0..2 {
                draw::draw_line(wid.x() + wid.w() - margin - 1 + i, wid.y() + margin,  wid.x() + wid.w() - margin - 1 + i, wid.y() + (wid.h()/3) + 1);
            }

            for i in 0..2 {
                draw::draw_line(wid.x() + (margin * 3), wid.y() + (wid.h()/3) + i + 1,  wid.x() + wid.w() - margin, wid.y() + (wid.h()/3) + i + 1);
            }

            draw::draw_rect_fill(wid.x() + (margin * 3), wid.y() + (wid.h()/3) + 1, wid.w() - (margin * 2 * 3)-1, wid.h() / 3 - 1, enums::Color::White);
            draw::draw_rect_fill(wid.x() + (wid.w()/2) - (wid.w() - (margin * 2 * 3))/2 , wid.y() + (wid.h()/3*2) + 2, margin+1, margin, enums::Color::White);
            draw::set_draw_color(old_color);
        }
    });

    updatechecks_button.draw({
        move |wid| {
            let old_color = draw::get_color();

            let new_color = if wid.active() {
                enums::Color::Blue
            } else {
                enums::Color::Blue.inactive()
            };

            draw::set_draw_color(new_color);
            draw::draw_rect_fill(wid.x(), wid.y(), wid.w(), wid.h(), new_color);
            draw::set_draw_color(enums::Color::White);

            let margin = 4;
            draw::draw_polygon(wid.x() + margin, wid.y() + wid.h()/3, wid.x() + (wid.w()/ 2), wid.y() + margin, wid.x() + wid.w() - margin, wid.y() + (wid.h()/3));
            draw::draw_rect_fill(wid.x() + (margin * 3), wid.y() + (wid.h()/3), wid.w() - (margin * 2 * 3), wid.h() / 3, enums::Color::White);
            draw::draw_line(wid.x() + margin, wid.y() + (wid.h()/3*2) + 2, wid.x() + wid.w() - margin, wid.y() + (wid.h()/3 * 2) + 2);

            draw::set_draw_color(old_color);
        }
    });

    helpinfo_button.set_tooltip(&trans_ref.gettext("Help"));
    helpinfo_button.set_callback({
        let wind = wind.clone();
        let trans_ref = trans_ref.clone();

        move |_| {
            show_dialog_help((wind.x(), wind.y(), wind.w(), wind.h()), trans_ref.clone());
        }
    });

    updatechecks_button.set_callback({
        let wind = wind.clone();
        let trans_ref = trans_ref.clone();

        move |_| {
            show_dialog_updates((wind.x(), wind.y(), wind.w(), wind.h()), trans_ref.clone());
        }
    });

    helpinfo_pack.end();

    top_group.end();

    let settings_pack_rc = Rc::new(RefCell::new(
        group::Pack::default()
            .with_pos(20, 20)
            .with_size(600, 580)
            .below_of(&top_group, WIDGET_GAP)
            .with_type(group::PackType::Vertical),
    ));
    settings_pack_rc.borrow_mut().set_spacing(WIDGET_GAP);

    // User settings - PDF result file suffix
    let mut filesuffix_pack = group::Pack::default()
        .with_size(570, 40)
        .with_type(group::PackType::Horizontal);

    filesuffix_pack.set_spacing(WIDGET_GAP);
    let mut filesuffix_checkbutton = button::CheckButton::default()
        .with_size(160, 20)
        .with_label(&trans.gettext("Custom file suffix"));
    filesuffix_checkbutton
        .set_tooltip(&trans.gettext("The safe PDF will be named <input>-<suffix>.pdf by default."));

    if let Some(file_suffix_cfg) = appconfig.clone().file_suffix {
        if file_suffix_cfg != config::DEFAULT_FILE_SUFFIX {
            filesuffix_checkbutton.set_checked(true);
        }
    }

    let filesuffix_input_rc = Rc::new(RefCell::new(input::Input::default().with_size(290, 20)));
    filesuffix_input_rc.borrow_mut().set_value(&appconfig.clone().file_suffix.unwrap_or_else(|| config::DEFAULT_FILE_SUFFIX.to_string()));

    if let Some(v) = appconfig.clone().file_suffix {
        if v == config::DEFAULT_FILE_SUFFIX {
            filesuffix_input_rc.borrow_mut().deactivate();
        }
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

    // User settings - Visual quality of PDF result
    let mut result_visual_quality_pack = group::Pack::default()
        .with_size(570, 40)
        .with_type(group::PackType::Horizontal);
    result_visual_quality_pack.set_spacing(WIDGET_GAP);
    let mut result_visual_quality_checkbutton = button::CheckButton::default()
        .with_size(100, 40)
        .with_label(&trans.gettext("Custom PDF result visual quality"))
        .with_align(enums::Align::Left | enums::Align::Inside);
    result_visual_quality_checkbutton.set_tooltip("Potentially sacrifice visual quality for processing time (quick preview first, etc.)");
    let result_visual_quality_menuchoice_rc = Rc::new(RefCell::new(
        menu::Choice::default().with_size(240, 40),
    ));

    for item in common::IMAGE_QUALITY_CHOICES.iter() {
        let item_translated = trans.gettext(item);
        result_visual_quality_menuchoice_rc.borrow_mut().add_choice(&item_translated);
    }

    let visual_quality_idx = {
        let vq = &appconfig.visual_quality;
        let mut ret = common::IMAGE_QUALITY_CHOICE_DEFAULT_INDEX as i32;

        if let Some(v) = vq {
            for (idx, item) in common::IMAGE_QUALITY_CHOICES.iter().enumerate() {
                if item == v {
                    ret = idx as i32;
                    break;
                }
            }
        }

        ret
    };

    result_visual_quality_menuchoice_rc.borrow_mut().set_value(visual_quality_idx);

    if visual_quality_idx == common::IMAGE_QUALITY_CHOICE_DEFAULT_INDEX as i32 {
        result_visual_quality_menuchoice_rc.borrow_mut().deactivate();
    } else {
        result_visual_quality_checkbutton.set_checked(true);
    }

    result_visual_quality_checkbutton.set_callback({
        let result_visual_quality_menuchoice_rc_ref = result_visual_quality_menuchoice_rc.clone();

        move |wid| {
            if !wid.is_checked() {
                result_visual_quality_menuchoice_rc_ref.borrow_mut().deactivate();
                result_visual_quality_menuchoice_rc_ref.borrow_mut().set_value(common::IMAGE_QUALITY_CHOICE_DEFAULT_INDEX as i32);
            } else {
                result_visual_quality_menuchoice_rc_ref.borrow_mut().activate();
            }
        }
    });

    result_visual_quality_pack.end();

    // User settings - OCR
    let mut ocrlang_pack = group::Pack::default()
        .with_size(570, 60)
        .below_of(&filesuffix_pack, WIDGET_GAP)
        .with_type(group::PackType::Horizontal);
    ocrlang_pack.set_spacing(WIDGET_GAP);
    let mut ocrlang_checkbutton = button::CheckButton::default()
        .with_size(300, 20)
        .with_label(&trans.gettext("Enable full-text search? In:"));
    ocrlang_checkbutton.set_tooltip(
        &trans.gettext("OCR (Optical character recognition) will be applied"),
    );

    if appconfig.ocr_lang.is_some() {
        ocrlang_checkbutton.set_checked(true);
    }

    let ocrlang_holdbrowser_rc = Rc::new(RefCell::new(
        browser::MultiBrowser::default().with_size(240, 60),
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

    let mut selected_ocr_languages = HashSet::new();

    if let Some(cur_ocrlangcode) = appconfig.ocr_lang.clone() {
        let selected_langcodes: Vec<&str> = cur_ocrlangcode.split('+').collect();

        for selected_langcode in selected_langcodes {
            if let Some(cur_ocrlangname) = ocr_languages_by_name_ref.get(&selected_langcode) {
                selected_ocr_languages.insert(cur_ocrlangname.to_string());
            }
        }
    }

    if selected_ocr_languages.is_empty() {
        selected_ocr_languages.insert(trans.gettext("English"));
    }

    for (i, item) in ocr_languages.iter().enumerate() {
        if selected_ocr_languages.contains(item) {
            let line = (i + 1) as i32;
            ocrlang_holdbrowser_rc.borrow_mut().select(line);
            ocrlang_holdbrowser_rc.borrow_mut().top_line(line);
        }
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
    // User settings - Open PDF result with a given application
    let mut openwith_pack = group::Pack::default()
        .with_size(570, 40)
        .with_type(group::PackType::Horizontal);
    openwith_pack.set_spacing(WIDGET_GAP);
    let mut openwith_checkbutton = button::CheckButton::default().with_size(295, 20).with_label(&trans.gettext("Open resulting PDF with"));
    openwith_checkbutton.set_tooltip(&trans.gettext("Automatically open resulting PDFs with a given program."));

    let pdf_apps_by_name = list_apps_for_pdfs();
    let openwith_inputchoice_rc = Rc::new(RefCell::new(misc::InputChoice::default().with_size(240, 20)));
    let mut pdf_viewer_app_names = Vec::with_capacity(pdf_apps_by_name.len());

    for k in pdf_apps_by_name.keys() {
        pdf_viewer_app_names.push(k.as_str());
    }

    pdf_viewer_app_names.sort();

    for k in pdf_viewer_app_names.iter() {
        openwith_inputchoice_rc.borrow_mut().add(k);
    }

    openwith_inputchoice_rc.borrow_mut().set_tooltip(&trans.gettext("You can also paste the path to a PDF viewer"));

    if !pdf_apps_by_name.is_empty() {
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

    // User settings - custom container image
    let mut ociimage_pack = group::Pack::default()
        .with_size(550, 40)
        .below_of(&ocrlang_pack, WIDGET_GAP)
        .with_type(group::PackType::Horizontal);
    ociimage_pack.set_spacing(WIDGET_GAP);
    let mut ociimage_checkbutton = button::CheckButton::default()
        .with_size(100, 20)
        .with_pos(0, 0)
        .with_align(enums::Align::Inside | enums::Align::Left);
    ociimage_checkbutton.set_label(&trans.gettext("Custom container image"));
    ociimage_checkbutton.set_label_color(enums::Color::Red);
    ociimage_checkbutton.set_tooltip(&trans.gettext("Expert option for sandbox solution"));

    let ociimage_text = if let Some(custom_container_image_name) = appconfig.container_image_name.clone() {
        custom_container_image_name
    } else {
        config::default_container_image_name()
    };

    let ociimage_input_rc = Rc::new(RefCell::new(input::Input::default().with_size(440, 20)));
    ociimage_input_rc.borrow_mut().set_value(&ociimage_text);

    if ociimage_text != config::default_container_image_name() {
        ociimage_checkbutton.set_checked(true);
    } else {
        ociimage_input_rc.borrow_mut().deactivate();
    }

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

    ociimage_pack.end();

    // User settings - save
    let savesettings_pack = group::Pack::default()
        .with_size(150, 30)
        .below_of(&ociimage_pack, WIDGET_GAP)
        .with_type(group::PackType::Horizontal);
    ociimage_pack.set_spacing(WIDGET_GAP);

    let mut savesettings_button = button::Button::default_fill()
        .with_size(wind.w() - (WIDGET_GAP * 2), 20)
        .with_label(&trans.gettext("Save current settings as defaults"))
        .with_align(enums::Align::Inside | enums::Align::Center);

    savesettings_button.set_callback({
        let result_visual_quality_menuchoice_rc_ref = result_visual_quality_menuchoice_rc.clone();
        let ocrlang_checkbutton_ref = ocrlang_checkbutton.clone();
        let ocrlang_holdbrowser_rc_ref = ocrlang_holdbrowser_rc.clone();
        let filesuffix_input_rc_ref = filesuffix_input_rc.clone();
        let filesuffix_checkbutton_ref = filesuffix_checkbutton.clone();
        let ociimage_checkbutton_ref = ociimage_checkbutton.clone();
        let ociimage_input_rc_ref = ociimage_input_rc.clone();
        let ocr_languages_by_lang_ref = ocr_languages_by_lang.clone();
        let result_visual_quality_checkbutton_ref = result_visual_quality_checkbutton.clone();
        let openwith_checkbutton_ref = openwith_checkbutton.clone();
        let openwith_inputchoice_rc_ref = openwith_inputchoice_rc.clone();
        let wind_ref = wind.clone();

        move|_| {
            let mut new_appconfig = config::AppConfig::default();

            if result_visual_quality_checkbutton_ref.is_checked() {
                let image_quality_idx = result_visual_quality_menuchoice_rc_ref.borrow().value();

                if image_quality_idx != common::IMAGE_QUALITY_CHOICE_DEFAULT_INDEX as i32 {
                    let image_quality_value = common::IMAGE_QUALITY_CHOICES[image_quality_idx as usize].to_string();
                    new_appconfig.visual_quality = Some(image_quality_value);
                } else {
                    new_appconfig.visual_quality = None;
                }
            } else {
                new_appconfig.visual_quality = None;
            }

            if ocrlang_checkbutton_ref.is_checked() {
                let ocrlang_dropdown = ocrlang_holdbrowser_rc_ref.borrow();
                let selected_langcodes = selected_ocr_langcodes(&ocr_languages_by_lang_ref, &ocrlang_dropdown);

                if !selected_langcodes.is_empty() {
                    new_appconfig.ocr_lang = Some(selected_langcodes.join("+"));
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

                if selected_filesuffix != *config::DEFAULT_FILE_SUFFIX {
                    new_appconfig.file_suffix = Some(selected_filesuffix);
                } else {
                    new_appconfig.file_suffix = None;
                }
            } else {
                new_appconfig.file_suffix = None;
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

    let mut divider = frame::Frame::default_fill()
        .with_size(wind.w(), 10);

    divider.draw({
        move |wid| {
            let ww = wid.w() / 2;
            let startx = wid.x() + wid.w() / 4;
            let old_color = draw::get_color();
            draw::set_draw_color(enums::Color::Black);
            draw::draw_line(startx , wid.y() + 1, startx + ww, wid.y() + 1);
            draw::set_draw_color(old_color);
        }
    });

    let mut row_convert_button = group::Pack::default()
        .with_size(wind.w(), 40)
        .below_of(&divider, 0)
        .with_type(group::PackType::Horizontal);
    row_convert_button.set_spacing(WIDGET_GAP/2);

    let mut selection_pack = group::Pack::default()
        .with_size(150, 40)
        .with_type(group::PackType::Vertical)
        .below_of(&convert_frame, 30);
    selection_pack.set_spacing(5);

    let mut selectall_frame = frame::Frame::default()
        .with_size(150, 10)
        .with_label(&trans.gettext("Select all"))
        .with_align(enums::Align::Inside | enums::Align::Left);

    let mut deselectall_frame = frame::Frame::default()
        .with_size(150, 10)
        .with_label(&trans.gettext("Deselect all"))
        .with_align(enums::Align::Inside | enums::Align::Left);

    selectall_frame
        .set_label_color(enums::Color::Blue);

    selectall_frame.draw({
        move |wid| {
            paint_underline(wid);
        }
    });

    deselectall_frame.draw({
        move |wid| {
            paint_underline(wid);
        }
    });

    deselectall_frame.set_label_color(enums::Color::Blue);

    selectall_frame.handle({
        move |_, ev| match ev {
            enums::Event::Push => {
                let _ = app::handle_main(EVENT_ID_ALL_SELECTED);
                true
            }
            _ => false,
        }
    });

    deselectall_frame.handle({
        move |_, ev| match ev {
            enums::Event::Push => {
                let _ = app::handle_main(EVENT_ID_ALL_DESELECTED);
                true
            }
            _ => false,
        }
    });

    selectall_frame.hide();
    deselectall_frame.hide();

    selection_pack.end();

    let mut delete_button = button::Button::default()
        .with_size(260, 20)
        .with_label(&trans.gettext("Delete selection"));
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

    let mut overall_progress_pack = group::Pack::default()
        .with_size(wind.w(), 40)
        .below_of(&row_convert_button, 30)
        .with_type(group::PackType::Horizontal);
    overall_progress_pack.set_spacing(WIDGET_GAP / 2);
    overall_progress_pack.set_label_color(enums::Color::White);
    overall_progress_pack.set_color(enums::Color::Black);

    let mut overall_progress_frame = frame::Frame::default()
        .with_size(120, 40)
        .with_label(&trans.gettext("Overall progress"));
    overall_progress_frame.set_label_color(FILELIST_ROW_COLOR_SUCCEEDED);
    overall_progress_frame.set_frame(enums::FrameType::FlatBox);
    overall_progress_frame.hide();

    let mut cancel_tasks_button = button::Button::default()
        .with_size(40, 40);
    cancel_tasks_button.set_color(FILELIST_ROW_COLOR_CANCELLED);
    cancel_tasks_button.set_tooltip(&trans.gettext("Try cancelling remaining conversions as soon as possible"));
    cancel_tasks_button.hide();

    cancel_tasks_button.set_callback({
        let converstion_stop_requested_ref = converstion_stop_requested.clone();

        move |wid| {
            converstion_stop_requested_ref.store(true, Ordering::Relaxed);
            wid.deactivate();
        }
    });

    let mut overall_progress_progressbar = misc::Progress::default_fill()
        .with_size(200, 40)
        .with_label("Pending");
    overall_progress_progressbar.hide();

    overall_progress_pack.set_color(enums::Color::Black);
    overall_progress_pack.end();

    let mut columns_frame = frame::Frame::default().with_size(500, 40).with_pos(10, 10);
    columns_frame.set_frame(enums::FrameType::NoBox);

    let mut filelist_scroll = group::Scroll::default().with_size(580, 200);
    let mut filelist_widget = FileListWidget::new(trans.clone());

    let col_label_password   = String::new();
    let col_label_outputfile = String::new();
    let col_label_filename   = trans.gettext("File name");
    let col_label_progress   = trans.gettext("Progress(%)");
    let col_label_status     = trans.gettext("Status");
    let col_label_message    = trans.gettext("Messages");

    columns_frame.draw({
        let filelist_widget_ref      = filelist_widget.clone();

        move |wid| {
            let file_count = filelist_widget_ref.children();
            if file_count != 0 {
                let w = filelist_widget_ref.container.w();
                let (width_output_file, width_password, width_checkbox, width_progressbar, width_status, width_logs) = filelist_column_widths(w);

                let column_widths = [
                    width_output_file, width_password, width_checkbox, width_progressbar, width_status, width_logs
                ];

                let column_names = vec![
                    col_label_password.clone(),
                    col_label_outputfile.clone(),
                    format!("{} [{}]", &col_label_filename, file_count),
                    col_label_progress.clone(),
                    col_label_status.clone(),
                    col_label_message.clone()
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

    let mut messages_frame = frame::Frame::default()
        .with_size(580, 40)
        .with_label("   ")
        .with_align(enums::Align::Left | enums::Align::Inside);

    convert_button.set_callback({
        let mut filelist_widget_ref = filelist_widget.clone();
        let mut convert_frame_ref = convert_frame.clone();
        let ocrlang_holdbrowser_rc_ref = ocrlang_holdbrowser_rc.clone();
        let mut tabsettings_button_ref =  tabsettings_button.clone();
        let ocrlang_checkbutton_ref = ocrlang_checkbutton.clone();
        let ociimage_input_rc_ref = ociimage_input_rc.clone();
        let filesuffix_input_rc_ref = filesuffix_input_rc.clone();
        let is_converting_ref = is_converting.clone();
        let conversion_is_active_ref = conversion_is_active.clone();
        let mut selectall_frame_ref = selectall_frame.clone();
        let mut deselectall_frame_ref = deselectall_frame.clone();
        let mut filelist_scroll_ref = filelist_scroll.clone();
        let trans_ref = trans_ref.clone();
        let current_row_idx = current_row_idx.clone();
        let mut updatechecks_button_ref = updatechecks_button.clone();
        let mut helpinfo_button_ref = helpinfo_button.clone();
        let mut overall_progress_pack_ref  = overall_progress_pack.clone();
        let mut overall_progress_frame_ref  = overall_progress_frame.clone();
        let mut cancel_tasks_button_ref = cancel_tasks_button.clone();
        let mut overall_progress_progressbar_ref  = overall_progress_progressbar.clone();
        let openwith_checkbutton_ref = openwith_checkbutton.clone();
        let pdf_viewer_list_ref = openwith_inputchoice_rc;

        move |b| {
            b.deactivate();
            helpinfo_button_ref.deactivate();
            updatechecks_button_ref.deactivate();
            tabsettings_button_ref.deactivate();
            selectall_frame_ref.deactivate();
            deselectall_frame_ref.deactivate();
            selectall_frame_ref.set_label_color(enums::Color::from_rgb(82, 82, 82));
            deselectall_frame_ref.set_label_color(enums::Color::from_rgb(82, 82, 82));
            convert_frame_ref.deactivate();
            is_converting_ref.store(true, Ordering::Relaxed);
            conversion_is_active_ref.store(true, Ordering::Relaxed);

            let image_quality_value_index = result_visual_quality_menuchoice_rc.borrow().value();
            let image_quality = common::IMAGE_QUALITY_CHOICES[image_quality_value_index as usize].to_lowercase();

            let opt_viewer_app = if openwith_checkbutton_ref.is_checked() {
                let viewer_app_name = pdf_viewer_list_ref.borrow_mut().input().value();
                if let Some(viewer_app_path) = pdf_apps_by_name.get(&viewer_app_name) {
                    Some(viewer_app_path.clone())
                } else {
                    Some(viewer_app_name.trim().to_owned())
                }
            } else {
                None
            };
            
            let opt_ocr_lang = if ocrlang_checkbutton_ref.is_checked() {
                let ocrlang_dropdown = ocrlang_holdbrowser_rc_ref.borrow();
                let selected_langcodes = selected_ocr_langcodes(&ocr_languages_by_lang, &ocrlang_dropdown);

                if selected_langcodes.is_empty() {
                    None
                } else {
                    Some(selected_langcodes.join("+"))
                }
            } else {
                None
            };

            let oci_image_text = ociimage_input_rc_ref.borrow().value();
            let opt_oci_image  = if oci_image_text.trim().is_empty() {
                config::default_container_image_name()
            } else {
                oci_image_text.trim().to_owned()
            };

            let mut file_suffix = filesuffix_input_rc_ref.borrow().value();
            if file_suffix.trim().is_empty() {
                file_suffix = appconfig.file_suffix.to_owned().unwrap_or_else(|| common::DEFAULT_FILE_SUFFIX.to_string());
            }

            let seccomp_disabled = if let Ok(env_seccomp_enablement) = env::var("ENTRUSTED_AUTOMATED_SECCOMP_ENABLEMENT") {
                env_seccomp_enablement.to_lowercase() == "false" || env_seccomp_enablement.to_lowercase() == "no"
            } else {
                false
            };

            let tasks: Vec<ConversionTask> = filelist_widget_ref.rows.borrow().iter().map(|row| {                
                row_to_task(&opt_oci_image,
                            image_quality.clone(),
                            &opt_ocr_lang,
                            &file_suffix,
                            !seccomp_disabled,
                            row
                )
            }).collect();

            filelist_widget_ref.deactivate_controls();

            if let Some(viewer_app) = &opt_viewer_app {
                filelist_widget_ref.set_viewer_app(viewer_app.clone());
            }            
            
            filelist_scroll_ref.scroll_to(0, 0);
            let task_count = tasks.len();

            if task_count > 1 && !overall_progress_frame_ref.visible() {
                overall_progress_progressbar_ref.set_value(0.0);
                overall_progress_progressbar_ref.set_label("");
                overall_progress_frame_ref.set_label_color(FILELIST_ROW_COLOR_SUCCEEDED);
                overall_progress_frame_ref.show();

                if !cancel_tasks_button_ref.active() {
                    cancel_tasks_button_ref.activate();
                }
                cancel_tasks_button_ref.show();

                overall_progress_progressbar_ref.show();
                overall_progress_pack_ref.redraw();
            }

            thread::spawn({
                let trans_ref = trans_ref.clone();
                let tx = tx.clone();
                let app_rcv = app_rx.clone();
                let current_row_idx = current_row_idx.clone();
                let conversion_is_active_ref = conversion_is_active_ref.clone();
                let converstion_stop_requested = converstion_stop_requested.clone();

                let eventer: Box<dyn common::EventSender> = Box::new(GuiEventSender {
                    tx
                });

                move || {
                    let mut idx = 0;
                    let mut move_next = true;
                    let mut fail_count = 0;
                    let mut completed_count = 0;

                    while idx < task_count {
                        if move_next {
                            if converstion_stop_requested.load(Ordering::Relaxed) {
                                converstion_stop_requested.store(false, Ordering::Relaxed);
                                break;
                            }

                            let task = &tasks[idx];
                            let input_path = task.input_path.clone();
                            let output_path = task.output_path.clone();
                            let convert_options = task.options.clone();
                            move_next = false;

                            let _ = eventer.send(common::AppEvent::ConversionStartEvent(idx));

                            if container::convert(input_path,
                                                  output_path.clone(),
                                                  convert_options,
                                                  eventer.clone_box(),
                                                  trans_ref.clone()).is_ok() {
                                completed_count += 1;
                                let _ = eventer.send(common::AppEvent::ConversionSuccessEvent(idx, task_count));
                            } else {
                                fail_count += 1;
                                completed_count += 1;
                                let _ = eventer.send(common::AppEvent::ConversionFailureEvent(idx, task_count));
                            }
                        }

                        if app_rcv.recv().is_some() {
                            idx += 1;
                            current_row_idx.store(idx as i32, Ordering::Relaxed);
                            move_next = true;
                        } else {
                            thread::yield_now();
                        }


                        if app::lock().is_ok() {
                            app::awake();
                            app::unlock();
                        }
                    }

                    current_row_idx.store(0, Ordering::Relaxed);
                    conversion_is_active_ref.store(false, Ordering::Relaxed);

                    if app::lock().is_ok() {
                        let _ = eventer.send(common::AppEvent::AllConversionEnded(completed_count, fail_count, task_count));
                        app::unlock();
                    }
                }
            });
        }

    });


    #[cfg(target_os = "macos")] {
        app::raw_open_callback(Some(|s| {
            let tx = app::Sender::<String>::get();
            tx.send({
                unsafe { std::ffi::CStr::from_ptr(s).to_string_lossy().to_string() }
            });
        }));

        menu::mac_set_about({
            let current_wind = wind.clone();
            let trans_ref = trans_ref.clone();

            move || {
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
                    &trans_ref.gettext_fmt("Copyright {0} {1}", vec!["2022-2024", "Rimero Solutions Inc."])
                );

                let mut logo_frame = frame::Frame::default()
                    .with_size(200, 50)
                    .with_pos(dialog_width/2 - 100, WIDGET_GAP);

                if let Ok(img) = image::PngImage::from_data(ICON_FRAME) {
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
                win.make_resizable(true);
                win.show();

                while win.shown() {
                    app::wait();
                }
            }
        });
    }

    convert_pack_rc.borrow_mut().end();
    tabconvert_button.set_frame(enums::FrameType::DownBox);
    tabsettings_button.set_frame(enums::FrameType::UpBox);
    settings_pack_rc.borrow_mut().hide();

    tabsettings_button.set_callback({
        let convert_pack_rc_ref = convert_pack_rc.clone();
        let settings_pack_rc_ref = settings_pack_rc.clone();
        let mut tabconvert_button_ref = tabconvert_button.clone();
        let mut filelist_scroll_ref = filelist_scroll.clone();
        let mut savesettings_button_ref = savesettings_button.clone();
        let mut wind_ref = wind.clone();

        move |b| {
            if !settings_pack_rc_ref.borrow().visible() {
                b.set_color(TAB_COLOR_PUSHED_BACKGROUND);
                b.set_label_color(TAB_COLOR_PUSHED_FOREGROUND);
                b.set_frame(enums::FrameType::DownBox);

                tabconvert_button_ref.set_color(default_wincolor);
                tabconvert_button_ref.set_label_color(enums::Color::Black);
                tabconvert_button_ref.set_frame(enums::FrameType::UpBox);

                convert_pack_rc_ref.borrow_mut().hide();
                settings_pack_rc_ref.borrow_mut().show();
                savesettings_button_ref.redraw();
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
                b.set_color(TAB_COLOR_PUSHED_BACKGROUND);
                b.set_label_color(TAB_COLOR_PUSHED_FOREGROUND);
                b.set_frame(enums::FrameType::DownBox);

                tabsettings_button_ref.set_color(default_wincolor);
                tabsettings_button_ref.set_label_color(enums::Color::Black);
                tabsettings_button_ref.set_frame(enums::FrameType::UpBox);

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
        let mut selectall_frame_rc_ref = selectall_frame.clone();
        let mut deselectall_frame_rc_ref = deselectall_frame.clone();
        let mut convert_button_ref = convert_button.clone();
        let mut columns_frame_ref = columns_frame.clone();
        let mut messages_frame_ref = messages_frame.clone();
        let mut row_convert_button_ref = row_convert_button.clone();
        let mut overall_progress_progressbar_ref = overall_progress_progressbar.clone();

        move |_, ev| match ev {
            enums::Event::DndEnter => {
                dnd = true;
                true
            },
            enums::Event::DndDrag => true,
            enums::Event::DndRelease => {
                released = true;
                true
            },
            enums::Event::Paste => {
                if dnd && released {
                    let path  = app::event_text();
                    let path  = path.trim();
                    let path  = path.replace("file://", "");
                    let paths = path.split('\n');

                    let file_paths: Vec<PathBuf> = paths
                        .map(PathBuf::from)
                        .filter(|p| p.exists())
                        .collect();

                    if !file_paths.is_empty() {
                        if is_converting_ref.load(Ordering::Relaxed) {
                            is_converting_ref.store(false, Ordering::Relaxed);
                            messages_frame_ref.set_label("");
                            messages_frame_ref.set_label_color(enums::Color::Black);
                            filelist_widget_ref.delete_all();
                            filelist_scroll_ref.scroll_to(0, 0);
                            filelist_scroll_ref.redraw();

                            overall_progress_progressbar_ref.set_value(0.0);
                            overall_progress_progressbar_ref.set_label(&format!("{:.0}%", 0.0));
                        }

                        if add_to_conversion_queue(file_paths, &mut filelist_widget_ref, &mut filelist_scroll_ref) {
                            if !selectall_frame_rc_ref.active() {
                                selectall_frame_rc_ref.activate();
                                selectall_frame_rc_ref.set_label_color(enums::Color::Blue);
                            }

                            if !deselectall_frame_rc_ref.active() {
                                deselectall_frame_rc_ref.activate();
                                deselectall_frame_rc_ref.set_label_color(enums::Color::Blue);
                            }

                            if !convert_button_ref.active() {
                                convert_button_ref.activate();
                                selection_pack_ref.set_damage(true);
                                selectall_frame_rc_ref.show();
                                deselectall_frame_rc_ref.show();

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
                }

                true
            },
            enums::Event::Push => {
                let mut selectfiles_filedialog = dialog::FileDialog::new(dialog::FileDialogType::BrowseMultiFile);
                selectfiles_filedialog.set_title(&selectfiles_dialog_title);
                selectfiles_filedialog.show();

                let file_paths: Vec<PathBuf> = selectfiles_filedialog
                    .filenames()
                    .iter()
                    .cloned()
                    .filter(|p| p.exists())
                    .collect();

                if !file_paths.is_empty() {
                    if is_converting_ref.load(Ordering::Relaxed) {
                        is_converting_ref.store(false, Ordering::Relaxed);
                        messages_frame_ref.set_label("");
                        messages_frame_ref.set_label_color(enums::Color::Black);
                        filelist_widget_ref.delete_all();
                        filelist_scroll_ref.scroll_to(0, 0);
                        filelist_scroll_ref.redraw();

                        overall_progress_progressbar_ref.set_value(0.0);
                        overall_progress_progressbar_ref.set_label(&format!("{:.0}%", 0.0));
                    }

                    if add_to_conversion_queue(file_paths, &mut filelist_widget_ref, &mut filelist_scroll_ref) {
                        if !selectall_frame_rc_ref.active() {
                            selectall_frame_rc_ref.activate();
                            selectall_frame_rc_ref.set_label_color(enums::Color::Blue);
                        }

                        if !deselectall_frame_rc_ref.active() {
                            deselectall_frame_rc_ref.activate();
                            deselectall_frame_rc_ref.set_label_color(enums::Color::Blue);
                        }

                        if !convert_button_ref.active() {
                            convert_button_ref.activate();
                            selection_pack_ref.set_damage(true);
                            selectall_frame_rc_ref.show();
                            deselectall_frame_rc_ref.show();

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
            },
            _ => false,
        }
    });

    wind.set_callback({
        move |wid| {
            let mut close_window = true;

            if conversion_is_active.load(Ordering::Relaxed) {
                if let Some(choice) = dialog::choice2(wid.x(),
                                                      wid.y() + wid.h()/2,
                                                      &trans.gettext("Really close"),
                                                      &trans.gettext("No"),
                                                      &trans.gettext("Yes"),
                                                      "") {
                    if choice == 0 {
                        close_window = false;
                    }
                }
            }

            if close_window {
                wid.hide();
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

    let _ = tabconvert_button.take_focus();

    if autoconvert {
        app::add_timeout3(0.2, {
            let mut convert_button = convert_button.clone();
            move |_| {
                convert_button.do_callback();
            }});
    }

    wind.handle({
        let mut top_group_ref = top_group.clone();

        let mut filesuffix_pack_ref = filesuffix_pack.clone();
        let mut filesuffix_checkbutton_ref = filesuffix_checkbutton.clone();

        let mut result_visual_quality_pack_ref = result_visual_quality_pack.clone();
        let mut result_visual_quality_checkbutton_ref = result_visual_quality_checkbutton.clone();

        let mut ocrlang_pack_ref = ocrlang_pack.clone();
        let mut ocrlang_checkbutton_ref = ocrlang_checkbutton.clone();

        let mut openwith_pack_ref = openwith_pack.clone();
        let mut openwith_checkbutton_ref = openwith_checkbutton.clone();

        let ociimage_input_rc_ref = ociimage_input_rc.clone();
        let mut ociimage_checkbutton_ref = ociimage_checkbutton.clone();
        let mut ociimage_pack_ref = ociimage_pack.clone();

        let mut selection_pack_ref = selection_pack.clone();
        let mut select_all_frame_ref = selectall_frame.clone();
        let mut deselect_all_frame_ref = deselectall_frame.clone();

        let mut filelist_scroll_ref = filelist_scroll.clone();
        let mut filelist_widget_ref = filelist_widget.clone();

        let mut row_convert_button_ref = row_convert_button.clone();
        let convert_frame_ref = convert_frame.clone();
        let mut convert_button_ref = convert_button.clone();
        let mut delete_button_ref = delete_button.clone();
        let mut columns_frame_ref = columns_frame.clone();

        let mut messages_frame_ref = messages_frame.clone();
        let mut helpinfo_pack_ref = helpinfo_pack.clone();
        let mut helpinfo_button_ref = helpinfo_button.clone();
        let mut updatechecks_button_ref = updatechecks_button.clone();

        let mut overall_progress_pack_ref = overall_progress_pack.clone();
        let mut overall_progress_frame_ref  = overall_progress_frame.clone();
        let mut overall_progress_progressbar_ref  = overall_progress_progressbar.clone();
        let mut cancel_tasks_button_ref = cancel_tasks_button.clone();
        let mut savesettings_button_ref = savesettings_button.clone();
        let mut divider_ref = divider.clone();

        let mut tabconvert_button_ref = tabconvert_button.clone();
        let mut tabsettings_button_ref = tabsettings_button.clone();

        move |wid, ev| match ev {
            enums::Event::Move => {
                wid.redraw();
                true
            },
            enums::Event::Resize => {
                top_group_ref.resize(
                    WIDGET_GAP,
                    WIDGET_GAP,
                    wid.w() - (WIDGET_GAP * 2),
                    30,
                );

                let tabs_width = tabconvert_button_ref.w();
                tabsettings_button_ref.resize(WIDGET_GAP, top_group_ref.y(), tabs_width, 30);
                tabconvert_button_ref.resize(tabsettings_button_ref.x() + WIDGET_GAP, top_group_ref.y(), tabs_width, 30);

                helpinfo_pack_ref.resize(tabconvert_button_ref.x() + WIDGET_GAP, top_group_ref.y(), wid.w() - (WIDGET_GAP * 3) - (tabs_width * 2), 30);
                spacer_frame.resize(helpinfo_pack_ref.x(), helpinfo_pack_ref.y(), helpinfo_pack_ref.w() - (WIDGET_GAP/4 * 2) - (30 * 2), 30);
                helpinfo_button_ref.resize(spacer_frame.x() + spacer_frame.w() + WIDGET_GAP/4, spacer_frame.y(), 30, 30);
                updatechecks_button_ref.resize(helpinfo_button_ref.x() + WIDGET_GAP/4, spacer_frame.y(), 30, 30);

                let content_y = top_group_ref.y() + top_group_ref.h() + WIDGET_GAP;

                let scroller_height = wid.h() - (WIDGET_GAP * 8) - top_group_ref.h() - convert_frame_ref.h() - row_convert_button_ref.h() - (messages_frame_ref.h() ) - overall_progress_pack_ref.h() - (WIDGET_GAP * 3);

                convert_pack_rc.borrow_mut().resize(
                    WIDGET_GAP,
                    content_y,
                    wid.w() - (WIDGET_GAP * 2),
                    wid.h() - top_group_ref.h() + WIDGET_GAP,
                );

                settings_pack_rc.borrow_mut().resize(
                    WIDGET_GAP,
                    content_y,
                    wid.w() - (WIDGET_GAP * 2),
                    wid.h() - top_group_ref.h() + WIDGET_GAP,
                );

                row_convert_button_ref.resize(
                    WIDGET_GAP, row_convert_button_ref.y(), wid.w() - (WIDGET_GAP * 2), row_convert_button_ref.h()
                );

                overall_progress_pack_ref.resize(
                    WIDGET_GAP, overall_progress_pack_ref.y(), wid.w() - (WIDGET_GAP * 2), overall_progress_pack_ref.h()
                );

                let (ow, _) = draw::measure(&overall_progress_frame_ref.label(), true);
                overall_progress_frame_ref.resize(
                    WIDGET_GAP, overall_progress_frame_ref.y(), ow, overall_progress_frame_ref.h()
                );

                cancel_tasks_button_ref.resize(WIDGET_GAP + ow + WIDGET_GAP,
                                               cancel_tasks_button_ref.y(),
                                               40, 40);

                overall_progress_progressbar_ref.resize(
                    (WIDGET_GAP * 2) + ow,
                    overall_progress_progressbar_ref.y(),
                    wid.w() - (WIDGET_GAP * 3) - ow - 40,
                    overall_progress_progressbar_ref.h()
                );

                convert_button_ref.resize(
                    convert_frame_ref.w() - convert_button_ref.w(), convert_button_ref.y() + 5, convert_button_ref.w(), convert_button_ref.h() - 5
                );

                delete_button_ref.resize(
                    wid.w() - (convert_button_ref.w() * 2), delete_button_ref.y(), delete_button_ref.w(), delete_button_ref.h()
                );

                divider_ref.resize(
                    WIDGET_GAP, divider_ref.y(), wid.w() - (WIDGET_GAP * 2), divider_ref.h()
                );

                filelist_scroll_ref.resize(
                    WIDGET_GAP,
                    filelist_scroll_ref.y(),
                    wid.w() - (WIDGET_GAP * 2),
                    scroller_height,
                );

                let wval = wid.w() - (WIDGET_GAP * 3);
                columns_frame_ref.widget_resize(WIDGET_GAP, columns_frame_ref.y(), wid.w() - (WIDGET_GAP * 2), columns_frame_ref.h());

                filelist_widget_ref.resize(filelist_scroll_ref.x(), filelist_scroll_ref.y(), wval, 0);

                filelist_scroll_ref.redraw();

                result_visual_quality_pack_ref.resize(
                    result_visual_quality_pack_ref.x(),
                    result_visual_quality_pack_ref.y(),
                    wid.w() - (WIDGET_GAP * 2),
                    result_visual_quality_pack_ref.h()
                );

                result_visual_quality_checkbutton_ref.resize(
                    result_visual_quality_checkbutton_ref.x(),
                    result_visual_quality_checkbutton_ref.y(),
                    ocrlang_checkbutton.w(),
                    result_visual_quality_checkbutton_ref.h()
                );

                let xx = ocrlang_holdbrowser_rc.borrow_mut().x();

                ociimage_pack_ref.resize(
                    ociimage_pack.x(),
                    ociimage_pack.y(),
                    wid.w() - (WIDGET_GAP * 2),
                    ociimage_pack.h(),
                );
                filesuffix_pack_ref.resize(
                    filesuffix_pack_ref.x(),
                    filesuffix_pack_ref.y(),
                    wid.w() - (WIDGET_GAP * 2),
                    filesuffix_pack_ref.h(),
                );
                openwith_pack_ref.resize(
                    openwith_pack_ref.x() + WIDGET_GAP / 2,
                    openwith_pack_ref.y(),
                    wid.w() - (WIDGET_GAP * 2),
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

                let ocw = wid.w() - (WIDGET_GAP * 3) - ocrlang_checkbutton.w();
                let och = wid.h() - (WIDGET_GAP * 8) - (30 * 7);

                
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
                    wid.w() - (WIDGET_GAP * 4),
                    och,
                );

                let yy = ocrlang_holdbrowser_rc.borrow_mut().y();
                ocrlang_holdbrowser_rc.borrow_mut().resize(
                    xx,
                    yy,
                    ocw,
                    och
                );

                let ociimage_input_rc_y = ociimage_input_rc.borrow().y();
                let ociimage_input_rc_h = ociimage_input_rc.borrow().h();
                ociimage_input_rc_ref.borrow_mut().resize(xx, ociimage_input_rc_y, ocw, ociimage_input_rc_h);

                let filesuffix_input_rc_ref_y = filesuffix_input_rc.borrow().y();
                let filesuffix_input_rc_ref_h = filesuffix_input_rc.borrow().h();
                filesuffix_input_rc.borrow_mut().resize(xx, filesuffix_input_rc_ref_y, ocw, filesuffix_input_rc_ref_h);


                messages_frame_ref.resize(
                    messages_frame_ref.x(),
                    wid.h() - (WIDGET_GAP * 2) - messages_frame_ref.h(),
                    wid.w() - (WIDGET_GAP * 2),
                    messages_frame_ref.h(),
                );

                savesettings_button_ref.resize(
                    savesettings_button_ref.x(),
                    wid.y() + wid.h() - WIDGET_GAP - savesettings_button_ref.h(),
                    wid.w() - (WIDGET_GAP * 2),
                    savesettings_button_ref.h(),
                );

                columns_frame_ref.redraw();
                filelist_scroll_ref.redraw();

                true
            }
            _ => {
                if ev.bits() == EVENT_ID_SELECTION_CHANGED {
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
                        select_all_frame_ref.hide();
                        deselect_all_frame_ref.hide();

                        overall_progress_frame_ref.hide();
                        overall_progress_progressbar_ref.hide();
                        cancel_tasks_button_ref.hide();
                    }

                    filelist_widget_ref.container.redraw();
                    filelist_scroll_ref.redraw();
                    true
                } else if ev.bits() == EVENT_ID_ALL_SELECTED {
                    filelist_widget_ref.select_all();
                    true
                } else if ev.bits() == EVENT_ID_ALL_DESELECTED {
                    filelist_widget_ref.deselect_all();
                    true
                } else {
                    app::event_state().is_empty() && app::event_key() == enums::Key::Escape
                }
            }
        }
    });
    
    wind.resize(wind.x(), wind.y(), wind.w(), wind.h());

    while app.wait() {
        #[cfg(target_os = "macos")] {
            if let Some(msg) = app_rx_appleevents.recv() {
                let mut filelist_widget_ref = filelist_widget.clone();
                let mut scroll_ref = filelist_scroll.clone();
                let file_path = PathBuf::from(msg);
                let mut selection_pack_ref = selection_pack.clone();
                let mut filelist_scroll_ref = filelist_scroll.clone();
                let mut select_all_frame_ref = selectall_frame.clone();
                let mut deselect_all_frame_ref = deselectall_frame.clone();
                let is_converting_ref = is_converting.clone();

                if file_path.exists() {
                    if is_converting_ref.load(Ordering::Relaxed) {
                        is_converting_ref.store(false, Ordering::Relaxed);
                        messages_frame.set_label("");
                        messages_frame.set_label_color(enums::Color::Black);
                        filelist_widget_ref.delete_all();
                        filelist_scroll_ref.scroll_to(0, 0);
                        filelist_scroll_ref.redraw();
                        overall_progress_progressbar.set_value(0.0);
                        overall_progress_progressbar.set_label(&format!("{:.0}%", 0.0));
                    }

                    if add_to_conversion_queue(vec![file_path], &mut filelist_widget_ref, &mut scroll_ref) {
                        if !selectall_frame.active() {
                            selectall_frame.activate();
                            selectall_frame.set_label_color(enums::Color::Blue);
                        }

                        if !deselectall_frame.active() {
                            deselectall_frame.activate();
                            deselectall_frame.set_label_color(enums::Color::Blue);
                        }

                        if !convert_button.active() {
                            convert_button.activate();
                            selection_pack_ref.set_damage(true);
                            select_all_frame_ref.show();
                            deselect_all_frame_ref.show();

                            selection_pack_ref.resize(selection_pack_ref.x(),
                                                      selection_pack_ref.y(),
                                                      selection_pack_ref.w(),
                                                      40);

                            selection_pack_ref.set_damage(true);
                            selection_pack_ref.redraw();
                        }
                    }
                }
            }

        }

        if let Ok(app_event) = rx.try_recv() {
            match app_event {
                common::AppEvent::ConversionProgressEvent(msg) => {
                    let log_msg_ret: serde_json::Result<common::LogMessage> = serde_json::from_slice(msg.as_bytes());

                    if let Ok(log_msg) = log_msg_ret {
                        messages_frame.set_label(&clip_text(&log_msg.data, messages_frame.w()));
                        let row_idx = current_row_idx.load(Ordering::Relaxed) as usize;
                        filelist_widget.update_progress(row_idx, log_msg.data, log_msg.percent_complete);
                    }

                    app::awake();
                },
                common::AppEvent::AllConversionEnded(completed_count, fail_count, total_count) => {
                    let summary_message = if completed_count != total_count {
                        trans_ref.gettext("The conversion was cancelled!")
                    } else if fail_count != total_count {
                        trans_ref.ngettext("One file failed to process", "Multiple files failed to process", fail_count as u64)
                    } else {
                        trans_ref.gettext("All files failed to process")
                    };

                    let messages_frame_color = if completed_count != total_count {
                        FILELIST_ROW_COLOR_CANCELLED
                    } else if fail_count == 0 {
                        FILELIST_ROW_COLOR_SUCCEEDED
                    } else {
                        FILELIST_ROW_COLOR_FAILED
                    };

                    messages_frame.set_label(&summary_message);
                    messages_frame.set_label_color(messages_frame_color);
                    tabsettings_button.activate();
                    helpinfo_button.activate();
                    updatechecks_button.activate();
                    convert_frame.activate();
                    filelist_scroll.scroll_to(0, 0);
                    filelist_scroll.redraw();
                    filelist_widget.activate_logs_links();

                    if cancel_tasks_button.active() {
                        cancel_tasks_button.deactivate();
                    }

                    if completed_count != total_count {
                        overall_progress_frame.set_label_color(FILELIST_ROW_COLOR_CANCELLED);
                        filelist_widget.mark_as_cancelled_starting_from_index(completed_count);
                        overall_progress_frame.redraw();
                    }

                    app::awake();
                },
                common::AppEvent::ConversionSuccessEvent(row_idx, total) => {
                    filelist_widget.update_status(row_idx, FILELIST_ROW_STATUS_SUCCEEDED, enums::Color::DarkGreen);

                    if overall_progress_progressbar.visible() {
                        let percent_complete = ((row_idx + 1) as f64 * 100.0) / (total as f64);
                        overall_progress_progressbar.set_value(percent_complete);
                        overall_progress_progressbar.set_label(&format!("{:.0}%", percent_complete));
                    }

                    app::awake();
                    app_tx.send(common::AppEvent::ConversionFinishedAckEvent);
                },
                common::AppEvent::ConversionFailureEvent(row_idx, total) => {
                    filelist_widget.update_status(row_idx, FILELIST_ROW_STATUS_FAILED, FILELIST_ROW_COLOR_FAILED);

                    if overall_progress_progressbar.visible() {
                        overall_progress_frame.set_label_color(FILELIST_ROW_COLOR_FAILED);
                        overall_progress_frame.redraw();
                        let percent_complete = ( (row_idx + 1) as f64 * 100.0) / (total as f64);
                        overall_progress_progressbar.set_value(percent_complete);
                        overall_progress_progressbar.set_label(&format!("{:.0}%", percent_complete));
                    }

                    app::awake();
                    app_tx.send(common::AppEvent::ConversionFinishedAckEvent);
                },
                common::AppEvent::ConversionStartEvent(row_idx) => {
                    filelist_widget.update_status(row_idx, FILELIST_ROW_STATUS_INPROGRESS, FILELIST_ROW_COLOR_INPROGRESS);
                    let mut row_ypos  = filelist_widget.ypos(row_idx);
                    let scroll_height = filelist_scroll.h();

                    if row_ypos > (filelist_scroll.yposition()) {
                        let distance = row_ypos % scroll_height;

                        if distance >= (scroll_height - 40) {
                            row_ypos -= 40;
                        }

                        filelist_scroll.scroll_to(0, row_ypos - filelist_scroll.y());
                        filelist_scroll.redraw();
                    }

                    app::awake();
                },
                _ => {}
            }
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
pub fn open_document(url: &str, _: &str, trans: &l10n::Translations) -> Result<(), Box<dyn Error>> {
    if let Some(cmd_open) = common::executable_find("open") {
        match Command::new(cmd_open).arg(url).spawn() {
            Ok(_)   => Ok(()),
            Err(ex) => Err(ex.into()),
        }
    } else {
        Err(trans.gettext("Could not find 'open' command in 'PATH' environment variable!").into())
    }
}

#[cfg(target_os = "windows")]
pub fn open_document(url: &str, content_type: &str,  trans: &l10n::Translations) -> Result<(), Box<dyn Error>> {
    extern "system" {
        pub fn ShellExecuteW(hwnd: winapi::HWND,
                             lpOperation: winapi::LPCWSTR,
                             lpFile: winapi::LPCWSTR,
                             lpParameters: winapi::LPCWSTR,
                             lpDirectory: winapi::LPCWSTR,
                             nShowCmd: winapi::c_int)
                             -> winapi::HINSTANCE;
    }

    const SW_SHOW: winapi::c_int = 5;    

    fn str_to_utf16(s: &str) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        let v = std::ffi::OsStr::new(s);
        v.encode_wide().chain(std::iter::once(0)).collect()
    }
    
    let path = str_to_utf16(url);
    let operation = str_to_utf16("open");
    let result = unsafe {
        ShellExecuteW(std::ptr::null_mut(),
                      operation.as_ptr(),
                      path.as_ptr(),
                      std::ptr::null(),
                      std::ptr::null(),
                      SW_SHOW)
    };

    if result as usize > 32 {
        Ok(())
    } else {
        Err(trans.gettext_fmt("Cannot find default application for content type: {0}", vec![content_type]).into())
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn open_document(url: &str,  content_type: &str, trans: &l10n::Translations) -> Result<(), Box<dyn Error>> {
    use freedesktop_entry_parser::parse_entry;

    let known_commands: Vec<(&str, Vec<&str>)> = vec![
        ("xdg-open"   , vec![]),
        ("gio"        , vec!["open"]),
        ("gnome-open" , vec![]),
        ("kde-open"   , vec![]),
        ("wslview"    , vec![]),
    ];

    let mut opt_cmd_open = None;

    if opt_cmd_open.is_none() {
        for (known_command, known_command_args) in known_commands {
            if let Some(abs_known_command) = common::executable_find(known_command) {
                opt_cmd_open = Some((abs_known_command.display().to_string(), known_command_args.clone()));
                break;
            }
        }
    }

    fn desktop_file_cmd(desktop_file: PathBuf) -> Option<String> {
        if !desktop_file.exists() {
            return None;
        }

        if let Ok(desktop_entry_data) = parse_entry(desktop_file) {
            let desktop_entry_section = desktop_entry_data.section("Desktop Entry");

            if let (Some(_), Some(cmd_name)) = (
                desktop_entry_section.attr("Name"),
                desktop_entry_section
                    .attr("Exec")
                    .or_else(|| desktop_entry_section.attr("TryExec"))
            ) {
                return Some(cmd_name.to_string());
            }
        }

        None
    }

    // Deeper look at file associations following the XDG specification conventions
    // This is similar to finding the list of PDF viewers on Linux, except that we're looking for default and then registered apps
    if opt_cmd_open.is_none() {
        let mut applications_folders = Vec::new();

        if let Some(data_local_folder) = dirs::data_local_dir() {
            if data_local_folder.exists() {
                let local_apps_folder = data_local_folder.join("applications");
                applications_folders.push(local_apps_folder.display().to_string());
            }
        }

        applications_folders.push("/usr/share/applications".to_string());
        applications_folders.push("/usr/local/share/applications".to_string());
        applications_folders.push("/var/lib/snapd/desktop/applications".to_string());

        for applications_folder in applications_folders {
            if opt_cmd_open.is_some() {
                break;
            }

            let path_usr_share_applications_orig = PathBuf::from(applications_folder);

            if !path_usr_share_applications_orig.exists() {
                continue;
            }

            let mut path_mimeinfo_cache = path_usr_share_applications_orig.clone();
            path_mimeinfo_cache.push("mimeinfo.cache");

            let defaults_list = path_usr_share_applications_orig.join("defaults.list");

            if defaults_list.exists() {
                if let Ok(conf) = parse_entry(defaults_list) {
                    if let Some(mime_pdf_desktop_refs) = conf.section("Default Applications").attr(content_type) {
                        let desktop_file = path_usr_share_applications_orig.join(mime_pdf_desktop_refs);

                        if let Some(desktop_cmd) = desktop_file_cmd(desktop_file) {
                            opt_cmd_open = Some((desktop_cmd, vec![]));
                            break;
                        }
                    }
                }
            }

            if path_mimeinfo_cache.exists() {
                if let Ok(conf) = parse_entry(path_mimeinfo_cache) {
                    if let Some(mime_pdf_desktop_refs) = conf.section("MIME Cache").attr(content_type) {
                        let desktop_entries: Vec<&str> = mime_pdf_desktop_refs.split(';').collect();

                        for desktop_entry in desktop_entries {
                            if desktop_entry.is_empty() {
                                continue;
                            }

                            let mut desktop_file = path_usr_share_applications_orig.clone();
                            desktop_file.push(desktop_entry);

                            if let Some(desktop_cmd) = desktop_file_cmd(desktop_file) {
                                opt_cmd_open = Some((desktop_cmd, vec![]));
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some((cmd_open, cmd_args)) = opt_cmd_open {
        let mut arguments = cmd_open;
        arguments.push(' ');

        for cmd_arg in cmd_args.iter() {
            arguments.push_str(cmd_arg);
            arguments.push(' ');
        }

        let mut url_added = false;
        let replacement_patterns = &["%u", "%U", "%f", "%F"];

        for replacement_pattern in replacement_patterns {
            if arguments.contains(replacement_pattern) {
                arguments = arguments.replace(replacement_pattern, url);
                url_added = true;
                break;
            }
        }

        if !url_added {
            arguments.push_str(url);
        }

        match Command::new("/bin/sh").arg("-c").arg(arguments).spawn() {
            Ok(_)   => Ok(()),
            Err(ex) => Err(ex.into()),
        }
    } else {
        Err(trans.gettext_fmt("Cannot find default application for content type: {0}", vec![content_type]).into())
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn list_apps_for_pdfs() -> HashMap<String, String> {
    use freedesktop_entry_parser::parse_entry;

    let mut ret = HashMap::new();

    // See https://wiki.archlinux.org/title/XDG_MIME_Applications for the logic
    // TODO is TryExec the best way to get a program name vs 'Exec' and stripping arguments???
    // TODO This is not robust enough...
    // Exec=someapp -newtab %u => where '%u' could be the file input parameter on top of other defaults '-newtab'
    fn parse_desktop_apps(
        apps_dir: Vec<&str>,
        mime_pdf_desktop_refs: &str,
    ) -> HashMap<String, String> {
        let desktop_entries: Vec<&str> = mime_pdf_desktop_refs.split(";").collect();
        let mut result = HashMap::with_capacity(desktop_entries.len());

        for desktop_entry in desktop_entries {
            if desktop_entry.is_empty() {
                continue;
            }

            for app_dir in apps_dir.iter() {
                let mut desktop_entry_path = PathBuf::from(app_dir);
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
        }

        result
    }

    let applications_folders = &["/usr/share/applications", "/usr/local/share/applications"];
    let applications_folders_vec = applications_folders.to_vec();

    for applications_folder in applications_folders {
        let path_usr_share_applications_orig = PathBuf::from(applications_folder);
        let mut ret: HashMap<String, String> = HashMap::new();
        let mut path_mimeinfo_cache = path_usr_share_applications_orig.clone();
        path_mimeinfo_cache.push("mimeinfo.cache");

        if path_mimeinfo_cache.exists() {
            if let Ok(conf) = parse_entry(path_mimeinfo_cache) {
                if let Some(mime_pdf_desktop_refs) = conf.section("MIME Cache").attr("application/pdf") {
                    let tmp_result = parse_desktop_apps(
                        applications_folders_vec.clone(),
                        mime_pdf_desktop_refs,
                    );

                    for (k, v) in &tmp_result {
                        ret.insert(k.to_string(), v.to_string());
                    }
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
                        applications_folders_vec.clone(),
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

#[cfg(target_os = "windows")]
pub fn list_apps_for_pdfs() -> HashMap<String, String> {
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
                                let new_name_copy = new_name.clone();

                                if new_name_copy.to_lowercase() == "entrusted" {
                                    continue;
                                }

                                ret.insert(new_name_copy, updated_path);
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
    use core_foundation::url::{CFURLCopyFileSystemPath, kCFURLPOSIXPathStyle, CFURLRef};
    use core_services::{
        kLSRolesAll, LSCopyAllRoleHandlersForContentType, LSCopyApplicationURLsForBundleIdentifier,
    };

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

                if err_ref.is_null() {
                    let app_count = CFArrayGetCount(apps);

                    for j in 0..app_count {
                        let cf_ref = CFArrayGetValueAtIndex(apps, j) as CFURLRef;
                        let cf_path = CFURLCopyFileSystemPath(cf_ref, kCFURLPOSIXPathStyle);
                        let cf_ptr = CFStringGetCStringPtr(cf_path, kCFStringEncodingUTF8);
                        let c_str = CStr::from_ptr(cf_ptr);
                        let mut app_name = String::new();

                        if let Ok(app_url) = c_str.to_str() {                            
                            let app_path = app_url.to_string();

                            if let Some(bundle_url) = CFURL::from_path(&app_url, true) {
                                if let Some(bundle) = CFBundle::new(bundle_url) {
                                    let bundle_dict = bundle.info_dictionary();
                                    let bundle_key_display_name = CFString::new("CFBundleDisplayName");
                                    let bundle_key_name = CFString::new("CFBundleName");

                                    let current_key = if bundle_dict.contains_key(&bundle_key_display_name) {
                                        Some(bundle_key_display_name)
                                    } else if bundle_dict.contains_key(&bundle_key_display_name) {
                                        Some(bundle_key_name)
                                    } else {
                                        None
                                    };

                                    if let Some(active_key) = current_key {                                            
                                        if let Some(active_key_value) = bundle_dict.find(&active_key)
                                            .and_then(|value_ref| value_ref.downcast::<CFString>())
                                            .map(|value| value.to_string()) {
                                                if &active_key_value == "Entrusted" {
                                                    continue;
                                                }
                                                app_name.push_str(&active_key_value);
                                            }
                                    } else if let Some(basename_ostr) = std::path::Path::new(&app_url).file_stem() {
                                        if let Some(basename) = &basename_ostr.to_str() {
                                            app_name.push_str(basename);
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

    ret
}

#[cfg(target_os = "windows")]
pub fn pdf_open_with(cmd: String, input: PathBuf, _: &l10n::Translations) -> Result<(), Box<dyn Error>> {
    use std::os::windows::process::CommandExt;
    match Command::new(cmd)
        .arg(input)
        .creation_flags(0x08000000)
        .spawn() {
            Ok(_)   => Ok(()),
            Err(ex) => Err(ex.into()),
        }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn pdf_open_with(cmd: String, input: PathBuf, _: &l10n::Translations) -> Result<(), Box<dyn Error>> {
    match Command::new(cmd).arg(input).spawn() {
        Ok(_)   => Ok(()),
        Err(ex) => Err(ex.into()),
    }
}

#[cfg(target_os = "macos")]
pub fn pdf_open_with(cmd: String, input: PathBuf, trans: &l10n::Translations) -> Result<(), Box<dyn Error>> {
    let p = std::path::Path::new(&cmd);

    if p.exists() && p.is_dir() {
        match common::executable_find("open") {
            Some(open_cmd) => match Command::new(open_cmd).arg("-a").arg(cmd).arg(input).spawn() {
                Ok(mut child_proc) => {
                    match child_proc.wait() {
                        Ok(exit_status) => {
                            if exit_status.success() {
                                Ok(())
                            } else {
                                Err(trans.gettext("Could not open PDF file!").into())
                            }
                        },
                        Err(ex) => Err(ex.into())
                    }
                },
                Err(ex) => Err(ex.into()),
            },
            None => Err(trans.gettext("Could not find 'open' command in 'PATH' environment variable!").into()),
        }
    } else {
        if let Err(ex) = Command::new(cmd).arg(input).spawn() {
            return Err(ex.into());
        }

        Ok(())
    }
}
