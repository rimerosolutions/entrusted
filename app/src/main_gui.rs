#![cfg_attr(
    all(
        target_os = "windows",
        not(debug_assertions),
    ),
    windows_subsystem = "windows"
)]

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::{env, path, thread};
use std::rc::Rc;
use std::sync::{mpsc, atomic, Arc};

use eframe::egui;
use mimalloc::MiMalloc;
use uuid::Uuid;

mod l10n;
mod common;
mod error;
mod config;
mod platform;
mod processing;
mod sanitizer;

use crate::common::{AppEvent, VisualQuality};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

const VERTICAL_GAP: f32 = 10.0;
const TABLE_COLUMN_NUMBER_MARGIN_X: i8 = 4;
const LINE_COLOR: egui::Color32 = egui::Color32::ORANGE;

const PROGRESS_VALUE_MAX: usize = 100;

const ICON_LOGO: &[u8]  = include_bytes!("../assets/images/app_logo.png");

const HELP_DATA_FEATURES: &str = include_str!("../assets/help/features.org");
const HELP_DATA_TOPICS: &str   = include_str!("../assets/help/topics.org");
const HELP_DATA_USAGE: &str    = include_str!("../assets/help/usage.org");

const EMPTY_STRING: String = String::new();

#[derive(Clone, PartialEq, Eq)]
struct OcrLang {
    id: String,
    name: String,
}

impl OcrLang {
    fn new(id: String, name: String) -> Self {
        Self { id, name }
    }
}

impl Ord for OcrLang {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for OcrLang {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Display for OcrLang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

#[derive(Clone)]
struct GuiEventSender {
    tx  : mpsc::Sender<common::AppEvent>,
    ctx : egui::Context
}

impl common::EventSender for GuiEventSender {
    fn send(&self, evt: crate::common::AppEvent) -> Result<(), mpsc::SendError<crate::common::AppEvent>> {
        self.tx.send(evt)?;
        self.ctx.request_repaint();
        thread::yield_now();
        Ok(())
    }
}

#[derive(Clone, PartialEq)]
enum UiTheme {
    Light, Dark, System,
}

impl std::fmt::Display for UiTheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiTheme::Light   => f.write_str("Light"),
            UiTheme::Dark    => f.write_str("Dark"),
            UiTheme::System  => f.write_str("System"),
        }
    }
}

impl From<Option<String>> for UiTheme {
    fn from(theme_opt: Option<String>) -> Self {
        if let Some(theme_value) = theme_opt {
            let theme_val = theme_value.to_lowercase();

            match theme_val.as_ref() {
                "light" => UiTheme::Light,
                "dark"  => UiTheme::Dark,
                _       => UiTheme::System,
            }
        } else {
            UiTheme::System
        }
    }
}

impl From<UiTheme> for egui::ThemePreference {
    fn from(val: UiTheme) -> Self {
        match val {
            UiTheme::Light  => egui::ThemePreference::Light,
            UiTheme::Dark   => egui::ThemePreference::Dark,
            UiTheme::System => egui::ThemePreference::System
        }
    }
}

fn move_to_screen(screen_id: ScreenId, current_state: &AppState) {
    let mut current_screen_id = current_state.current_screen_id.borrow_mut();
    *current_screen_id = screen_id;
}

fn capitalize_first_letter(text: &str) -> String {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return EMPTY_STRING;
    };

    first.to_uppercase().chain(chars).collect()

}

fn text_width(ui: &mut egui::Ui, text: String) -> f32 {
    let galley = ui.painter().layout_no_wrap(text, egui::FontId::default(), egui::Color32::WHITE);
    galley.size().x * ui.ctx().pixels_per_point() + 10.0
}

fn paint_horizontal_line(ui: &mut egui::Ui, line_color: egui::Color32) {
    let start = ui.cursor().min;
    let end = egui::pos2(start.x + ui.available_width(), start.y);
    ui.painter().line_segment([start, end], egui::Stroke::new(1.5, line_color));
}

fn filled_triangle(
    ui: &egui::Ui,
    rect: egui::Rect,
    visuals: &egui::style::WidgetVisuals,
    _is_open: bool,
    _above_or_below: egui::AboveOrBelow,
) {
    let rect = egui::Rect::from_center_size(
        rect.center(),
        egui::vec2(rect.width() * 0.6, rect.height() * 0.4),
    );

    ui.painter().add(egui::Shape::convex_polygon(
        vec![rect.left_top(), rect.right_top(), rect.center_bottom()],
        visuals.fg_stroke.color,
        visuals.fg_stroke,
    ));
}

fn combine_options(global_options: &common::ConvertOptions, file_options: &FileUploadOptions) -> common::ConvertOptions {
    let mut convert_options = global_options.clone();
    let save_folder = &file_options.output_folder;

    if !save_folder.trim().is_empty() {
        convert_options.output_folder = Some(path::PathBuf::from(save_folder));
    }

    if !file_options.password_decrypt.trim().is_empty() {
        convert_options.password_decrypt = Some(file_options.password_decrypt.to_string());
    }

    if !file_options.password_encrypt.trim().is_empty() {
        convert_options.password_encrypt = Some(file_options.password_encrypt.to_string());
    }

    convert_options
}

struct App {
    app_state: AppState,
    navbar: Navbar,
    screens: Vec<Box<dyn AppScreen>>
}

#[derive(Clone, PartialEq)]
enum ModalDisplayState {
    FileDialog, Log, ProcessingOptions, ErrorOccured, Hidden
}

#[derive(Clone)]
struct AppState {
    ui_theme: UiTheme,
    sanitizer: sanitizer::Sanitizer,
    trans: l10n::Translations,
    current_screen_id: Rc<RefCell<ScreenId>>,
    converting: Rc<RefCell<bool>>,
    theme_set: Rc<RefCell<bool>>,
    tx_gui: mpsc::Sender<GuiEvent>,
    rx_gui: Rc<RefCell<mpsc::Receiver<GuiEvent>>>,
    tx_proc: mpsc::Sender<AppEvent>,
    rx_proc: Rc<RefCell<mpsc::Receiver<AppEvent>>>,
    tx_modal: mpsc::Sender<ModalEvent>,
    rx_modal: Rc<RefCell<mpsc::Receiver<ModalEvent>>>,
    modal_shown: bool,
    modal_display_state: ModalDisplayState,
    modal_text_data: String,
    modal_processing_data: (usize, FileUploadOptions),
    convert_options: Rc<RefCell<common::ConvertOptions>>,
}

impl AppState {
    fn new(convert_options: common::ConvertOptions, sanitizer: sanitizer::Sanitizer, trans: l10n::Translations, ui_theme: UiTheme) -> Self {
        let (tx_gui,   rx_gui)   = mpsc::channel::<GuiEvent>();
        let (tx_proc,  rx_proc)  = mpsc::channel::<AppEvent>();
        let (tx_modal, rx_modal) = mpsc::channel::<ModalEvent>();

        Self {
            convert_options: Rc::new(RefCell::new(convert_options)),
            ui_theme,
            sanitizer,
            trans,
            current_screen_id: Rc::new(RefCell::new(ScreenId::Welcome)),
            theme_set: Rc::new(RefCell::new(false)),
            converting: Rc::new(RefCell::new(false)),
            tx_gui,
            rx_gui: Rc::new(RefCell::new(rx_gui)),
            tx_proc,
            rx_proc: Rc::new(RefCell::new(rx_proc)),
            tx_modal,
            rx_modal: Rc::new(RefCell::new(rx_modal)),
            modal_shown: false,
            modal_display_state: ModalDisplayState::Hidden,
            modal_text_data: String::new(),
            modal_processing_data: (0, FileUploadOptions::default()),
        }
    }
}

impl App {
    fn new(ui_theme: UiTheme, app_logo: egui::TextureHandle, convert_options: common::ConvertOptions, sanitizer: sanitizer::Sanitizer, ocr_lang_initial_selection: usize, ocr_lang_codes_selections: Vec<bool>, ocr_langs: Vec<OcrLang>, trans: l10n::Translations) -> Self {
        let app_state = AppState::new(convert_options, sanitizer, trans.clone(), ui_theme);
        let tx_modal = app_state.tx_modal.clone();

        Self {
            app_state,
            navbar: Navbar,
            screens: vec![Box::new(WelcomeScreen::new(app_logo)),
                          Box::new(UploadScreen::new(tx_modal, trans.clone())),
                          Box::new(SettingsScreen::new(ocr_lang_initial_selection, ocr_lang_codes_selections, ocr_langs)),
                          Box::new(DocumentationScreen::new(&trans)),]
        }
    }
}

struct Navbar;

impl Navbar {
    fn render(&self, _: &egui::Context, ui: &mut egui::Ui, app_state: &mut AppState, home_button_enabled: bool) {
        let trans = &app_state.trans;

        if ui.add_enabled(home_button_enabled, egui::Button::new(trans.gettext("Home"))).clicked() {
            move_to_screen(ScreenId::Welcome, app_state);
        }

        ui.add_space(VERTICAL_GAP);
    }
}

#[derive(Debug, Clone, PartialEq)]
enum ScreenId {
    Welcome,
    Upload,
    Settings,
    Documentation,
}

trait AppScreen {
    fn render(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, app_state: &mut AppState);
    fn id(&self) -> ScreenId;
}

struct DocumentationScreen {
    tabs: Vec<DocumentationScreenTab>,
    active_tab_id: Uuid
}

impl DocumentationScreen {
    fn new(trans: &l10n::Translations) -> Self {
        let tabs = vec![
            DocumentationScreenTab::new(trans.gettext("Features"), org_to_layout_job(HELP_DATA_FEATURES, trans)),
            DocumentationScreenTab::new(trans.gettext("Topics"),   org_to_layout_job(HELP_DATA_TOPICS, trans)),
            DocumentationScreenTab::new(trans.gettext("Usage"),    org_to_layout_job(HELP_DATA_USAGE, trans)),
        ];

        let first_tab_id = tabs[0].id.clone();
        Self { tabs, active_tab_id: first_tab_id }
    }
}

struct DocumentationScreenTab {
    id: Uuid,
    title: String,
    contents: egui::text::LayoutJob
}

fn org_to_layout_job(contents: &'static str, trans: &l10n::Translations) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();    
    let font_size = 13.0;

    for line in contents.lines() {
        let mut line_data = String::with_capacity(line.len() + 1);

        if line.trim().is_empty() {
            line_data.push_str(line);
        } else {
            let translated_line = trans.gettext(line);
            line_data.push_str(&translated_line);
        }

        line_data.push('\n');

        if line.starts_with("*") { // Header text
            job.append(&line_data, 0.0, egui::TextFormat {
                font_id: egui::FontId::new(font_size, egui::FontFamily::Proportional),
                color: LINE_COLOR,
                ..Default::default()
            });
        } else {                    // Normal text
            job.append(&line_data, 0.0, egui::TextFormat {
                font_id: egui::FontId::new(font_size, egui::FontFamily::Proportional),
                ..Default::default()
            });
        }
    }

    job
}

impl DocumentationScreenTab {
    fn new(title: String, contents: egui::text::LayoutJob) -> Self {
        Self { id: Uuid::new_v4(), title, contents }
    }
}

impl AppScreen for DocumentationScreen {
    fn render(&mut self, _: &egui::Context, ui: &mut egui::Ui, app_state: &mut AppState) {
        let trans = &app_state.trans;

        ui.heading(trans.gettext("Documentation"));
        ui.add_space(VERTICAL_GAP);

        ui.horizontal(|ui| {
            for tab in &self.tabs {
                let tab_enabled = self.active_tab_id != tab.id;

                if ui.add_enabled(tab_enabled, egui::Link::new(&tab.title)).clicked() {
                    self.active_tab_id = tab.id;
                }
            }
        });

        let idx = self.tabs.iter().position(|x| x.id == self.active_tab_id).unwrap_or(0);

        egui::ScrollArea::both().show(ui, |ui| {
            ui.add(egui::Label::new(self.tabs[idx].contents.clone()).wrap());
        });
    }

    fn id(&self) -> ScreenId {
        ScreenId::Documentation
    }
}

#[derive(Debug, Clone)]
struct FileUploadOptions {
    password_decrypt: String,
    password_encrypt: String,
    output_folder: String,
}

impl Default for FileUploadOptions {
    fn default() -> Self {
        Self {
            password_decrypt: EMPTY_STRING,
            password_encrypt: EMPTY_STRING,
            output_folder:    EMPTY_STRING,
        }
    }
}

#[derive(Clone)]
struct FileUploadEntryAddFiles {
    id: Uuid,
    path: path::PathBuf,
    options: FileUploadOptions,
    selected: bool,
}

impl FileUploadEntryAddFiles {
    fn new(path: path::PathBuf, options: FileUploadOptions) -> Self {
        Self { path, options, id: Uuid::new_v4(), selected: false }
    }
}

#[derive(Clone, PartialEq)]
enum FileUploadStatus {
    Pending, Processing, Interrupted, Succeeded, Failed
}

impl std::fmt::Display for FileUploadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileUploadStatus::Pending     => f.write_str("Pending"),
            FileUploadStatus::Processing  => f.write_str("Processing"),
            FileUploadStatus::Interrupted => f.write_str("Interrupted"),
            FileUploadStatus::Succeeded   => f.write_str("Succeeded"),
            FileUploadStatus::Failed      => f.write_str("Failed"),
        }
    }
}

#[derive(Clone)]
struct FileUploadEntryProcessFiles {
    id: Uuid,
    path: path::PathBuf,
    output: Option<path::PathBuf>,
    options: FileUploadOptions,
    status: FileUploadStatus
}

impl FileUploadEntryProcessFiles {
    fn new(id: Uuid,
           path: path::PathBuf,
           options: FileUploadOptions) -> Self {
        Self { id, path, output: None, options, status: FileUploadStatus::Pending }
    }
}

#[derive(Clone)]
enum GuiEvent {
    ReadToUpload,
    ReadyToProcess(Vec<FileUploadEntryAddFiles>),
    FilesUploaded(Vec<FileUploadEntryAddFiles>),
    FileUploadOptionsUpdated((usize, FileUploadOptions))
}

#[derive(Clone)]
enum ModalEvent {
    FileDialog,
    Log(String),
    ErrorOccured(String),
    ProcessingOptions((usize, FileUploadOptions)),
}

trait UploadScreenDelegate {
    fn cell_content_ui(&mut self, row_number: u64, column_number: usize, ui: &mut egui::Ui);
    fn column_name(&mut self, column_number: usize) -> String;
    fn render(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, app_state: &mut AppState);
    fn ack_gui_event(&mut self, ctx: &egui::Context, event: GuiEvent, app_state: &mut AppState);
    fn ack_proc_event(&mut self, ctx: &egui::Context, event: common::AppEvent, app_state: &mut AppState);
}

struct UploadScreenAddFilesDelegate {
    column_names: Vec<String>,
    file_upload_entries: Vec<FileUploadEntryAddFiles>,
    file_upload_paths: HashSet<String>,
    tx_modal: mpsc::Sender<ModalEvent>,
}

impl UploadScreenAddFilesDelegate {
    fn new(tx_modal: mpsc::Sender<ModalEvent>, trans: l10n::Translations) -> Self {
        Self {
            column_names: vec![EMPTY_STRING, trans.gettext("File name").to_string()],
            file_upload_entries: vec![],
            file_upload_paths: HashSet::new(),
            tx_modal,
        }
    }

    fn delete_selected(&mut self) {
        let entries = &mut self.file_upload_entries;

        entries.retain(|item| {
            if item.selected {
                let path = item.path.display().to_string();
                self.file_upload_paths.remove(&path);
            }

            !item.selected
        });
    }

    fn can_select_or_deselect_all(&mut self, add_selection: bool) -> bool {
        self.file_upload_entries.iter().any(|x| x.selected != add_selection)
    }

    fn toggle_all_selections(&mut self, add_selection: bool) {
        for item in self.file_upload_entries.iter_mut() {
            item.selected = add_selection;
        }
    }
}

impl egui_table::TableDelegate for UploadScreenAddFilesDelegate {
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell_inf: &egui_table::HeaderCellInfo) {
        let col_index = cell_inf.col_range.start;

        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(TABLE_COLUMN_NUMBER_MARGIN_X, 0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(self.column_name(col_index)).strong());
            });
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell_info: &egui_table::CellInfo) {
        let egui_table::CellInfo { row_nr, col_nr, .. } = *cell_info;

        if row_nr % 2 == 1 {
            ui.painter().rect_filled(ui.max_rect(), 0.0, ui.visuals().faint_bg_color);
        }

        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(TABLE_COLUMN_NUMBER_MARGIN_X, 0))
            .show(ui, |ui| {
                self.cell_content_ui(row_nr, col_nr, ui);
            });
    }
}

impl UploadScreenDelegate for UploadScreenAddFilesDelegate {
    fn cell_content_ui(&mut self, row_number: u64, column_number: usize, ui: &mut egui::Ui) {
        if column_number == 0 {    // Row number
            ui.label((row_number + 1).to_string());
        } else {                   // File name
            if row_number < self.file_upload_entries.len() as u64 {
                let entry = &mut self.file_upload_entries[row_number as usize];
                let file_name = entry.path.file_name().unwrap().to_string_lossy();

                ui.horizontal(|ui| {
                    ui.add(egui::Checkbox::without_text(&mut entry.selected));
                    ui.add(egui::Label::new(file_name).truncate());
                });
            } else {
                ui.label(EMPTY_STRING);
            }
        }
    }

    fn ack_proc_event(&mut self, _: &egui::Context, _: common::AppEvent, _: &mut AppState) {
        // No processing event to handle while uploading files
    }

    fn ack_gui_event(&mut self, _: &egui::Context, event: GuiEvent, _: &mut AppState) {
        match event {
            GuiEvent::FilesUploaded(entries) => {
                let existing_paths = &mut self.file_upload_paths;

                for entry in entries.iter() {
                    let entry_path = entry.path.display().to_string();

                    if !existing_paths.contains(&entry_path) {
                        self.file_upload_entries.push(entry.clone());
                        existing_paths.insert(entry_path);
                    }
                }
            },
            GuiEvent::FileUploadOptionsUpdated((index, options)) => {
                self.file_upload_entries[index].options = options;
            },
            _ => {}
        }
    }

    fn column_name(&mut self, column_number: usize) -> String {
        if self.file_upload_paths.is_empty() {
            EMPTY_STRING
        } else {
            self.column_names[column_number].to_owned()
        }
    }

    fn render(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, app_state: &mut AppState) {
        let tx = app_state.tx_gui.clone();
        let tx_modal = app_state.tx_modal.clone();
        let trans = &app_state.trans;

        ui.vertical_centered(|ui| {
            let max_width = ui.available_width();

            if ui.add(egui::Button::new(trans.gettext("Click to browse files or Drag and drop files into this window.")).min_size([max_width, 40.0].into())).clicked() {
                let ctx = ctx.clone();
                let builder = thread::Builder::new().name("entrusted_deletedfiles_thread".into());

                let _ = builder.spawn(move || {
                    let _ = tx_modal.send(ModalEvent::FileDialog);
                    ctx.request_repaint();
                });
            }
        });

        ui.add_space(VERTICAL_GAP);

        let row_count = self.file_upload_entries.len() as u64;

        ui.horizontal(|ui| {
            let selection_active = self.can_select_or_deselect_all(false);

            ui.vertical(|ui| {
                if ui.add_enabled(self.can_select_or_deselect_all(true), egui::Link::new(trans.gettext("Select all"))).clicked() {
                    self.toggle_all_selections(true);
                }

                if ui.add_enabled(selection_active, egui::Link::new(trans.gettext("Deselect all"))).clicked() {
                    self.toggle_all_selections(false);
                }
            });

            ui.separator();

            ui.vertical(|ui| {
                if ui.add_enabled(selection_active, egui::Link::new(trans.gettext("Remove selected"))).clicked() {
                    self.delete_selected();
                }

                if ui.add_enabled(selection_active, egui::Link::new(trans.gettext("Configure selected"))).clicked() && row_count != 0 {
                    if let Some(index) = self.file_upload_entries.iter().position(|x| x.selected) {
                        let entries = &self.file_upload_entries;
                        let tx_modal = self.tx_modal.clone();
                        let entry = &entries[index];
                        let options = entry.options.clone();
                        let ctx = ctx.clone();
                        let builder = thread::Builder::new().name("entrusted_modal_thread".into());

                        let _ = builder.spawn(move || {
                            let _ = tx_modal.send(ModalEvent::ProcessingOptions((index, options)));
                            ctx.request_repaint();
                        });
                    }
                }
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(row_count != 0, egui::Button::new(trans.gettext("Sanitize"))).clicked() {
                    let proc_entries = self.file_upload_entries.clone();

                    self.file_upload_entries.clear();
                    self.file_upload_paths.clear();

                    *app_state.converting.borrow_mut() = true;
                    let builder = thread::Builder::new().name("entrusted_filepicker_thread".into());
                    let ctx = ctx.clone();

                    let _ = builder.spawn(move || {
                        let _ = tx.send(GuiEvent::ReadyToProcess(proc_entries));
                        ctx.request_repaint();
                    });
                }
            });
        });
        ui.add_space(VERTICAL_GAP);

        let document_upload_label = if row_count > 0 {
            let row_count_string = row_count.to_string();
            trans.gettext_fmt("Documents uploaded ({0}):", vec![&row_count_string])
        } else {
            EMPTY_STRING
        };

        ui.label(document_upload_label);
        ui.add_space(VERTICAL_GAP);

        let wn1 = text_width(ui, row_count.to_string());
        let desired_w = ui.available_width() - wn1;

        let table = egui_table::Table::new()
            .id_salt("table_upload_add")
            .num_rows(row_count)
            .columns(vec![
                egui_table::Column::new(wn1).resizable(false).range(wn1..=wn1),
                egui_table::Column::new(desired_w).resizable(false).range(desired_w..=desired_w),
            ])
            .num_sticky_cols(1)
            .headers([egui_table::HeaderRow::new(20.0)])
            .auto_size_mode(egui_table::AutoSizeMode::default());

        table.show(ui, self);
    }
}

struct UploadScreenProcessFilesDelegate {
    column_names: Vec<String>,
    file_upload_entries: Vec<FileUploadEntryProcessFiles>,
    file_upload_statuses: HashMap<Uuid, Vec<(usize, String)>>,
    started: bool,
    done: bool,
    interrupted: bool,
    stop_requested: Arc<atomic::AtomicBool>,
    processed_count: Arc<atomic::AtomicUsize>,
    tx_modal: mpsc::Sender<ModalEvent>,
    document_processing_label: String,
    trans: l10n::Translations,
}

impl UploadScreenProcessFilesDelegate {
    fn new(tx_modal: mpsc::Sender<ModalEvent>, trans: l10n::Translations) -> Self {
        Self {
            column_names: vec![EMPTY_STRING, trans.gettext("File name"), trans.gettext("Progress"), trans.gettext("Status"), trans.gettext("Result")],
            file_upload_entries: vec![],
            processed_count: Arc::new(atomic::AtomicUsize::new(0)),
            started: false,
            stop_requested: Arc::new(atomic::AtomicBool::new(false)),
            done: false,
            interrupted: false,
            file_upload_statuses: HashMap::new(),
            document_processing_label: trans.gettext("Documents processing:"),
            tx_modal,
            trans,
        }
    }
}

impl egui_table::TableDelegate for UploadScreenProcessFilesDelegate {
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell_inf: &egui_table::HeaderCellInfo) {
        let col_index = cell_inf.col_range.start;

        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(TABLE_COLUMN_NUMBER_MARGIN_X, 0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(self.column_name(col_index)).strong());
            });
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell_info: &egui_table::CellInfo) {
        let egui_table::CellInfo { row_nr, col_nr, .. } = *cell_info;

        if row_nr % 2 == 1 {
            ui.painter().rect_filled(ui.max_rect(), 0.0, ui.visuals().faint_bg_color);
        }

        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(TABLE_COLUMN_NUMBER_MARGIN_X, 0))
            .show(ui, |ui| {
                self.cell_content_ui(row_nr, col_nr, ui);
            });
    }
}

impl UploadScreenDelegate for UploadScreenProcessFilesDelegate {
    fn cell_content_ui(&mut self, row_number: u64, column_number: usize, ui: &mut egui::Ui) {
        if column_number == 0 { // Row number
            ui.label((row_number + 1).to_string());
        } else {
            let row_count = self.file_upload_entries.len();

            if row_count == 0 {
                ui.label(EMPTY_STRING);
            } else {
                let entry = &self.file_upload_entries[row_number as usize];
                let doc_id = entry.id;
                let statuses = &self.file_upload_statuses;

                let (links_enabled, progress_value) = if let Some(data) = statuses.get(&doc_id) {
                    if let Some((last_progress_value, _)) = data.last() {
                        let links_enabled = entry.status == FileUploadStatus::Failed || entry.status == FileUploadStatus::Succeeded || entry.status == FileUploadStatus::Interrupted;
                        let last_progress_value = (*last_progress_value as f32)/ PROGRESS_VALUE_MAX as f32;
                        (links_enabled, last_progress_value)
                    } else {
                        (false, 0.0)
                    }
                } else {
                    (false, 0.0)
                };

                if column_number == 1 {        // File name
                    let file_name = entry.path.file_name().unwrap().to_string_lossy();
                    ui.horizontal(|ui| {
                        ui.add(egui::Label::new(file_name).truncate());
                    });
                } else if column_number == 4 { // Results
                    ui.horizontal(|ui| {
                        if links_enabled {
                            let mut ui_builder = egui::UiBuilder::new();

                            if entry.output.is_none() {
                                ui_builder = ui_builder.invisible();
                            }

                            ui.scope_builder(ui_builder, |ui| {
                                if ui.link(self.trans.gettext("Output")).clicked() {
                                    if let Some(output) = &entry.output {
                                        let trans = &self.trans;

                                        if let Err(ex) = utils::open_with_default_application(output.display().to_string(), "application/pdf", trans) {
                                            let tx_modal = self.tx_modal.clone();
                                            let error_message = ex.to_string();
                                            let builder = thread::Builder::new().name("entrusted_modal_thread".into());

                                            let _ = builder.spawn(move || {
                                                let _ = tx_modal.send(ModalEvent::ErrorOccured(error_message));
                                            });
                                        }
                                    }
                                }

                                ui.add_space(VERTICAL_GAP / 6.0);
                            });

                            if ui.link(self.trans.gettext("Log")).clicked() {
                                if let Some(data) = statuses.get(&doc_id) {
                                    let mut log_messages = String::with_capacity((data.len() + 1) * 4);

                                    for (_, message) in data.iter() {
                                        log_messages.push_str(message);
                                        log_messages.push('\n');
                                    }

                                    let tx_modal = self.tx_modal.clone();
                                    let builder = thread::Builder::new().name("entrusted_modal_thread".into());

                                    let _ = builder.spawn(move || {
                                        let _ = tx_modal.send(ModalEvent::Log(log_messages.clone()));
                                    });
                                }
                            }
                        } else {
                            ui.label(EMPTY_STRING);
                        }
                    });
                } else if column_number == 2 { // Progress bar
                    ui.add(egui::ProgressBar::new(progress_value).desired_width(75.0).show_percentage().corner_radius(0.0));
                } else if column_number == 3 { // Status
                    let status_name   = entry.status.to_string();
                    let status_string = self.trans.gettext(&status_name);
                    let status_color  = match entry.status {
                        FileUploadStatus::Succeeded   => egui::Color32::from_rgb(3, 218, 40),
                        FileUploadStatus::Failed      => egui::Color32::LIGHT_RED,
                        FileUploadStatus::Interrupted => LINE_COLOR,
                        _                             => ui.ctx().style().visuals.widgets.noninteractive.fg_stroke.color,
                    };

                    ui.label(egui::RichText::new(status_string).color(status_color));
                }
            }
        }
    }

    fn column_name(&mut self, column_number: usize) -> String {
        self.column_names[column_number].to_owned()
    }

    fn ack_proc_event(&mut self, ctx: &egui::Context, event: common::AppEvent, app_state: &mut AppState) {
        let trans = &app_state.trans;

        match event {
            common::AppEvent::ConversionFinished(index, output_path_opt) => {
                if let Some(output_path) = output_path_opt {
                    self.file_upload_entries[index].output = Some(output_path);
                    self.file_upload_entries[index].status = FileUploadStatus::Succeeded;
                } else {
                    self.file_upload_entries[index].status = FileUploadStatus::Interrupted;
                    self.interrupted = true;
                }

                self.processed_count.fetch_add(1, atomic::Ordering::SeqCst);
            },
            common::AppEvent::ConversionStarted(index) => {
                self.file_upload_entries[index].status = FileUploadStatus::Processing;
            },
            common::AppEvent::ConversionFailed(index)  => {
                self.file_upload_entries[index].status = FileUploadStatus::Failed;
                self.processed_count.fetch_add(1, atomic::Ordering::SeqCst);
            },
            common::AppEvent::ConversionProgressed(doc_id, progress_value, progress_message) => {
                self.file_upload_statuses.entry(doc_id)
                    .or_insert_with(Vec::new)
                    .push((progress_value, progress_message));
            },
            common::AppEvent::AllConversionEnded(counter_succeeded, counter_failed, counter_total) => {
                if self.interrupted {
                    self.document_processing_label = trans.gettext("The processing was interrupted!");
                } else if counter_succeeded != counter_total {
                    if counter_total == 1 {
                        self.document_processing_label = trans.gettext("The document failed to process!");
                    } else if counter_failed == counter_total {
                        self.document_processing_label = trans.gettext("All documents failed to process!");
                    } else {
                        let counter_failed_string = counter_failed.to_string();
                        let counter_total_string = counter_total.to_string();
                        self.document_processing_label = trans.gettext_fmt("{0} documents out of {1} failed processing!", vec![&counter_failed_string, &counter_total_string]);
                    }
                } else {
                    self.document_processing_label = trans.gettext("All documents were successfully processed!");
                };

                self.done = true;
                *app_state.converting.borrow_mut() = false;
                ctx.request_repaint();
            },
        }
    }

    fn ack_gui_event(&mut self, _: &egui::Context, event: GuiEvent, app_state: &mut AppState) {
        let trans = &app_state.trans;

        match event {
            GuiEvent::ReadyToProcess(entries) => {
                for entry in entries.iter() {
                    let item = FileUploadEntryProcessFiles::new(entry.id, entry.path.clone(), entry.options.clone());
                    self.file_upload_entries.push(item);
                }

                let row_count_string = entries.len().to_string();
                self.document_processing_label = trans.gettext_fmt("Documents processing ({0}):", vec![&row_count_string]);
            },
            GuiEvent::ReadToUpload => {
                self.done = false;
                self.started = false;
                self.file_upload_entries.clear();
                self.interrupted = false;
                self.document_processing_label = trans.gettext("Documents processing:");
                self.processed_count.store(0, atomic::Ordering::SeqCst);
                self.stop_requested.store(false, atomic::Ordering::SeqCst);
            },
            _ => {}
        }
    }

    fn render(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, app_state: &mut AppState) {
        let tx_proc = app_state.tx_proc.clone();
        let trans = &app_state.trans;
        let max_width = ui.available_width();

        ui.horizontal(|ui| {
            if ui.add_enabled(self.done, egui::Link::new(trans.gettext("Add more files"))).clicked() {
                *app_state.converting.borrow_mut() = false;
                let tx = app_state.tx_gui.clone();
                let _ = tx.send(GuiEvent::ReadToUpload);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let controls_size = ui.ctx().style().spacing.interact_size.y;
                let processing_active = !self.done;
                let entries = &self.file_upload_entries;
                let entries_count = entries.len();

                let progress_value = if processing_active {
                    if entries_count == 0 {   // Prevent crashes while UI is out of sync
                        0.0
                    } else if entries_count == 1 { // Reflect the current document progress information
                        let doc_id = entries[0].id;

                        if let Some(statuses) = self.file_upload_statuses.get(&doc_id) {
                            if let Some((p, _)) = statuses.last() {
                                *p as f32 / PROGRESS_VALUE_MAX as f32
                            } else { 0.0 }
                        } else { 0.0 }
                    } else {
                        let mut total_progress = 0.0;

                        for entry in entries.iter() {
                            if let Some(statuses) = self.file_upload_statuses.get(&entry.id) {
                                if let Some((p, _)) = statuses.last() {
                                    total_progress += *p as f32;
                                }
                            }
                        }

                        if total_progress > 0.0 {
                            total_progress / entries_count as f32 / PROGRESS_VALUE_MAX as f32
                        } else {
                            total_progress
                        }
                    }
                } else { 1.0 };

                ui.add_sized([controls_size, controls_size], egui::ProgressBar::new(progress_value).desired_width(75.0).show_percentage().corner_radius(0.0));

                ui.add_enabled_ui(processing_active, |ui| {
                    let button_enabled = !(self.stop_requested.load(atomic::Ordering::SeqCst) || self.done);

                    ui.add_enabled_ui(button_enabled, |ui| {
                        let button_color = if button_enabled {
                            LINE_COLOR
                        } else {
                            egui::Color32::DARK_GRAY
                        };

                        let button = egui::Button::new(EMPTY_STRING).fill(button_color);

                        if ui.add_sized([controls_size, controls_size], button).on_hover_text(trans.gettext("Stop processing")).clicked() {
                            self.stop_requested.store(true, atomic::Ordering::SeqCst);
                            ctx.request_repaint();
                        }
                    });
                });

                ui.label(trans.gettext("Overall progress"));
            });

        });

        ui.add_space(VERTICAL_GAP);
        paint_horizontal_line(ui, LINE_COLOR);
        ui.add_space(VERTICAL_GAP);

        ui.label(&self.document_processing_label);
        ui.add_space(VERTICAL_GAP);

        let files = self.file_upload_entries.clone();
        let row_count = files.len() as u64;

        let wn1 = text_width(ui, row_count.to_string());
        let rem_w = max_width - wn1 - 75.0 - 100.0 - 110.0;
        let (w1, w2, w3, w4, w5) = (wn1, rem_w, 75.0, 100.0, 110.0);

        let table = egui_table::Table::new()
            .id_salt("table_upload_process")
            .num_rows(row_count)
            .columns(vec![
                egui_table::Column::new(w1).resizable(true).range(w1..=w1),
                egui_table::Column::new(w2).resizable(true).range(w2..=w2),
                egui_table::Column::new(w3).resizable(true).range(w3..=w3),
                egui_table::Column::new(w4).resizable(true).range(w4..=w4),
                egui_table::Column::new(w5).resizable(true).range(w5..=w5),
            ])
            .num_sticky_cols(1)
            .headers([egui_table::HeaderRow::new(20.0)])
            .auto_size_mode(egui_table::AutoSizeMode::default());

        table.show(ui, self);        

        if !self.started && !self.done {
            self.started = true;

            let ctx             = ctx.clone();
            let files           = files.clone();
            let trans           = trans.clone();
            let tx              = tx_proc.clone();
            let processed_count = self.processed_count.clone();

            let stop_flag = Arc::clone(&self.stop_requested);
            let stop_flag_proc = Arc::clone(&self.stop_requested);

            let sanitizer = app_state.sanitizer.clone();
            let default_convert_options = app_state.convert_options.borrow().clone();
            let builder = thread::Builder::new().name("entrusted_processing_thread".into());

            let _ = builder.spawn(move || {
                let counter_total = row_count as usize;
                let mut counter_succeeded: usize = 0;
                let mut counter_failed: usize = 0;

                let eventer = Box::new(GuiEventSender {
                    tx: tx.clone(), ctx: ctx.clone()
                });

                for (i, file) in files.iter().enumerate() {
                    if stop_flag.load(atomic::Ordering::SeqCst) {
                        break;
                    }

                    // Allow the UI catch up with conversion progress messages
                    // TODO: risk of infite loop if there are business logic bugs...
                    while processed_count.load(atomic::Ordering::SeqCst) != i {
                        thread::yield_now();
                    }

                    let (src_path, doc_id) = (&file.path, file.id);
                    let convert_options = combine_options(&default_convert_options, &file.options);
                    let stop_flag_current = Arc::clone(&stop_flag_proc);

                    let _ = tx.send(AppEvent::ConversionStarted(i));
                    thread::yield_now();

                    match sanitizer.sanitize(doc_id, src_path.clone(), convert_options, eventer.clone(), trans.clone(), stop_flag_current) {
                        Ok(output_path_opt) => {
                            let _ = tx.send(AppEvent::ConversionFinished(i, output_path_opt));
                            thread::yield_now();
                            counter_succeeded += 1;
                        },
                        Err(ex) => {
                            let _ = tx.send(AppEvent::ConversionProgressed(file.id, PROGRESS_VALUE_MAX, ex.to_string()));
                            let _ = tx.send(AppEvent::ConversionFailed(i));
                            thread::yield_now();
                            counter_failed += 1;
                        }
                    }

                    ctx.request_repaint();
                }

                let _ = tx.send(common::AppEvent::AllConversionEnded(counter_succeeded, counter_failed, counter_total));
                thread::yield_now();
                ctx.request_repaint();
            });
        }
    }
}

struct UploadScreen {
    delegate_add_files: UploadScreenAddFilesDelegate,
    delegate_process_files: UploadScreenProcessFilesDelegate
}

impl UploadScreen {
    fn new(tx_modal: mpsc::Sender<ModalEvent>, trans: l10n::Translations) -> Self {
        Self {
            delegate_add_files     : UploadScreenAddFilesDelegate::new(tx_modal.clone(), trans.clone()),
            delegate_process_files : UploadScreenProcessFilesDelegate::new(tx_modal.clone(), trans.clone()),            
        }
    }
}

impl AppScreen for UploadScreen {
    fn render(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, app_state: &mut AppState) {
        let trans = &app_state.trans;

        ui.heading(trans.gettext("Upload"));
        ui.add_space(VERTICAL_GAP);

        let rx_gui = app_state.rx_gui.clone();
        let rx_proc = app_state.rx_proc.clone();
        let conversion_in_progress = *app_state.converting.borrow_mut();
        let mut data_received = false;

        while let Ok(app_event) = rx_gui.borrow_mut().try_recv() {
            // Ignore drag and drop events when the user is on the processing screen
            if let GuiEvent::FilesUploaded(_) = app_event {
                if conversion_in_progress {
                    continue;
                }
            }

            self.delegate_add_files.ack_gui_event(ctx, app_event.clone(), app_state);
            self.delegate_process_files.ack_gui_event(ctx, app_event.clone(), app_state);
            data_received = true;
        }

        while let Ok(app_event) = rx_proc.borrow_mut().try_recv() {
            self.delegate_add_files.ack_proc_event(ctx, app_event.clone(), app_state);
            self.delegate_process_files.ack_proc_event(ctx, app_event.clone(), app_state);
            data_received = true;
        }

        let processing_files = !self.delegate_process_files.file_upload_entries.is_empty();

        if !conversion_in_progress && !processing_files {
            self.delegate_add_files.render(ctx, ui, app_state);
        } else {
            self.delegate_process_files.render(ctx, ui, app_state);
        }

        if data_received {
            ctx.request_repaint();
        }
    }

    fn id(&self) -> ScreenId {
        ScreenId::Upload
    }
}

#[derive(Clone, PartialEq)]
enum OutputPathOptions {
    SameFolder, DedicatedFolder
}

struct SettingsScreen {
    enable_custom_filename_suffix: bool,
    custom_filename_suffix: String,
    enable_custom_visual_quality: bool,
    custom_visual_quality: VisualQuality,
    enable_ocr: bool,
    ocr_lang_initial_selection: usize,
    ocr_lang_codes_selections: Vec<bool>,
    ocr_langs: Vec<OcrLang>,
    ui_theme: UiTheme,
    enable_save_in_same_folder: OutputPathOptions,
    dedicated_save_folder_path: String,
}

impl SettingsScreen {
    // TODO from config for initial startup in AppState construction
    fn new(ocr_lang_initial_selection: usize, ocr_lang_codes_selections: Vec<bool>, ocr_langs: Vec<OcrLang>) -> Self {
        Self {
            enable_custom_filename_suffix: false,
            custom_filename_suffix: common::DEFAULT_FILE_SUFFIX.to_string(),
            enable_custom_visual_quality: false,
            custom_visual_quality: VisualQuality::default_value(),
            enable_ocr: false,
            ocr_lang_initial_selection,
            ocr_lang_codes_selections,
            ui_theme: UiTheme::System,
            ocr_langs,
            enable_save_in_same_folder: OutputPathOptions::SameFolder,
            dedicated_save_folder_path: EMPTY_STRING,
        }
    }

    // TODO: Use Config object once that part is fully refactored
    fn maybe_update_default_convert_options(&self, ui: &mut egui::Ui, app_state: &mut AppState, old_enable_custom_filename_suffix: bool, old_custom_filename_suffix: String, old_enable_custom_visual_quality: bool, old_custom_visual_quality: VisualQuality, old_enable_ocr: bool, old_ocr_lang_codes_selections: Vec<bool>, old_enable_save_in_same_folder: OutputPathOptions, old_dedicated_save_folder_path: String, old_ui_theme: UiTheme) {
        let mut default_convert_options = app_state.convert_options.borrow_mut();

        if self.enable_custom_filename_suffix != old_enable_custom_filename_suffix || self.custom_filename_suffix != old_custom_filename_suffix {
            default_convert_options.filename_suffix = self.custom_filename_suffix.clone();
        }

        if self.enable_custom_visual_quality != old_enable_custom_visual_quality || self.custom_visual_quality != old_custom_visual_quality {
            default_convert_options.visual_quality = self.custom_visual_quality.clone();
        }

        if self.enable_ocr != old_enable_ocr || self.ocr_lang_codes_selections != old_ocr_lang_codes_selections {
            default_convert_options.ocr_lang_code = if self.enable_ocr {
                let mut ocr_lang_codes = Vec::with_capacity(2);

                for (i, item) in self.ocr_langs.iter().enumerate() {
                    if self.ocr_lang_codes_selections[i] {
                        ocr_lang_codes.push(item.id.clone());
                    }
                }

                let ocr_lang_str: String = ocr_lang_codes.join("+");

                if !ocr_lang_str.is_empty() {
                    Some(ocr_lang_str)
                } else {
                    None
                }
            } else {
                None
            };
        }

        if self.enable_save_in_same_folder != old_enable_save_in_same_folder || self.dedicated_save_folder_path !=  old_dedicated_save_folder_path {
            let new_output_folder = if self.enable_save_in_same_folder == OutputPathOptions::SameFolder || self.dedicated_save_folder_path.trim().is_empty() {
                None
            } else {
                Some(path::PathBuf::from(&self.dedicated_save_folder_path))
            };

            default_convert_options.output_folder = new_output_folder;
        }

        if self.ui_theme != old_ui_theme {
            ui.ctx().set_theme(self.ui_theme.clone());
        }
    }
}

impl AppScreen for SettingsScreen {
    fn render(&mut self, _: &egui::Context, ui: &mut egui::Ui, app_state: &mut AppState) {
        let trans = &app_state.trans;

        ui.heading(trans.gettext("Settings"));
        ui.add_space(VERTICAL_GAP);

        ui.label(egui::RichText::new(trans.gettext("Appearance settings")).strong());
        ui.add_space(VERTICAL_GAP);

        let old_enable_custom_filename_suffix = self.enable_custom_filename_suffix;
        let old_custom_filename_suffix = self.custom_filename_suffix.clone();
        let old_enable_custom_visual_quality = self.enable_custom_visual_quality;
        let old_custom_visual_quality = self.custom_visual_quality.clone();
        let old_enable_ocr = self.enable_ocr;
        let old_ocr_lang = self.ocr_lang_codes_selections.clone();
        let old_enable_save_in_same_folder = self.enable_save_in_same_folder.clone();
        let old_dedicated_save_folder_path = self.dedicated_save_folder_path.clone();
        let old_ui_theme = self.ui_theme.clone();

        ui.horizontal(|ui| {
            ui.label(trans.gettext("Theme"));

            egui::ComboBox::from_id_salt("combobox_ui_theme")
                .selected_text(trans.gettext(&self.ui_theme.to_string()))
                .icon(filled_triangle)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.ui_theme, UiTheme::Light,  trans.gettext(&UiTheme::Light.to_string())).clicked();
                    ui.selectable_value(&mut self.ui_theme, UiTheme::Dark,   trans.gettext(&UiTheme::Dark.to_string())).clicked();
                    ui.selectable_value(&mut self.ui_theme, UiTheme::System, trans.gettext(&UiTheme::System.to_string())).clicked();
                });
        });

        ui.add_space(VERTICAL_GAP);
        paint_horizontal_line(ui, LINE_COLOR);
        ui.add_space(VERTICAL_GAP);

        ui.label(egui::RichText::new(trans.gettext("Processing settings")).strong());
        ui.add_space(VERTICAL_GAP);

        egui::Grid::new("grid_settings_conversion").show(ui, |ui| {
            if ui.checkbox(&mut self.enable_custom_filename_suffix, trans.gettext("Custom output file name suffix")).changed() && !self.enable_custom_filename_suffix {
                self.custom_filename_suffix = common::DEFAULT_FILE_SUFFIX.to_string();
            }

            ui.add_enabled(self.enable_custom_filename_suffix, egui::TextEdit::singleline(&mut self.custom_filename_suffix));
            ui.end_row();

            if ui.checkbox(&mut self.enable_custom_visual_quality, trans.gettext("Custom output visual quality")).changed() && !self.enable_custom_visual_quality {
                self.custom_visual_quality = VisualQuality::default_value();
            }

            egui::ComboBox::from_id_salt("combobox_visual_quality")
                .selected_text(trans.gettext(&self.custom_visual_quality.to_string()))
                .icon(filled_triangle)
                .show_ui(ui, |ui| {
                    ui.add_enabled_ui(self.enable_custom_visual_quality, |ui| {
                        ui.selectable_value(&mut self.custom_visual_quality, VisualQuality::Low,    trans.gettext(&VisualQuality::Low.to_string()));
                        ui.selectable_value(&mut self.custom_visual_quality, VisualQuality::Medium, trans.gettext(&VisualQuality::Medium.to_string()));
                        ui.selectable_value(&mut self.custom_visual_quality, VisualQuality::High,   trans.gettext(&VisualQuality::High.to_string()));
                    });
                });

            ui.end_row();

            ui.label(EMPTY_STRING);
            ui.end_row();

            if ui.checkbox(&mut self.enable_ocr, trans.gettext("Searchable PDFs via OCR")).changed() && !self.enable_ocr {
                for i in 0..self.ocr_lang_codes_selections.len() {
                    self.ocr_lang_codes_selections[i] = self.ocr_lang_initial_selection == i;
                }
            }

            // TODO future flexibility, calculate dimensions ased on longuest label
            egui::ScrollArea::both().max_width(500.0).max_height(50.0).show(ui, |ui| {
                if !self.enable_ocr {
                    ui.disable();
                }

                ui.vertical(|ui| {
                    for (i, item) in &mut self.ocr_langs.iter().enumerate() {
                        ui.checkbox(&mut self.ocr_lang_codes_selections[i], item.to_string());
                    }
                });
            });

            ui.end_row();
        });

        ui.add_space(VERTICAL_GAP);

        ui.label(trans.gettext("By default, sanitized output will be saved in:"));

        if ui.add(egui::RadioButton::new(self.enable_save_in_same_folder == OutputPathOptions::SameFolder, trans.gettext("The same folder as the uploaded file"))).clicked() {
            self.enable_save_in_same_folder = OutputPathOptions::SameFolder;
            self.dedicated_save_folder_path.clear();
        }

        ui.horizontal(|ui| {
            let use_dedicated_folder = self.enable_save_in_same_folder == OutputPathOptions::DedicatedFolder;
            ui.radio_value(&mut self.enable_save_in_same_folder, OutputPathOptions::DedicatedFolder, trans.gettext("A dedicated folder"));

            if ui.add_enabled(use_dedicated_folder, egui::Button::new(trans.gettext("Browse..."))).clicked() {
                let selected_folder_opt = rfd::FileDialog::new().set_title(trans.gettext("Select output folder")).pick_folder();

                if let Some(ref selected_folder) = selected_folder_opt {
                    self.dedicated_save_folder_path = selected_folder.display().to_string();
                }
            }

            ui.add_enabled(use_dedicated_folder, egui::TextEdit::singleline(&mut self.dedicated_save_folder_path).hint_text(trans.gettext("Same folder if blank")));
        });

        // TODO update config another way, too many params...
        self.maybe_update_default_convert_options(ui, app_state, old_enable_custom_filename_suffix, old_custom_filename_suffix, old_enable_custom_visual_quality, old_custom_visual_quality, old_enable_ocr, old_ocr_lang, old_enable_save_in_same_folder, old_dedicated_save_folder_path, old_ui_theme);
    }


    fn id(&self) -> ScreenId {
        ScreenId::Settings
    }
}

struct WelcomeScreen {
    app_logo: egui::TextureHandle
}

impl WelcomeScreen {
    fn new(app_logo: egui::TextureHandle) -> Self {
        Self { app_logo }
    }
}

impl AppScreen for WelcomeScreen {
    fn render(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, app_state: &mut AppState) {
        let trans = &app_state.trans;

        ui.heading(trans.gettext("Welcome"));
        ui.add_space(VERTICAL_GAP);

        ui.horizontal(|ui| {
            ui.image((self.app_logo.id(), self.app_logo.size_vec2()));

            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(capitalize_first_letter(option_env!("CARGO_PKG_NAME").unwrap_or("Unknown")));
                    ui.label(egui::RichText::new(option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown")).strong());
                });

                ui.add_space(VERTICAL_GAP);

                ui.label(trans.gettext("This program helps neuter potentially suspicious files, by removing active content: a PDF output is generated."));
                ui.add_space(VERTICAL_GAP);

                ui.label(trans.gettext("The project is hosted on GitHub."));

                if let Some(project_homepage) = option_env!("CARGO_PKG_REPOSITORY") {
                    if ui.link(project_homepage).clicked() {
                        if let Err(ex) = utils::open_with_default_application(project_homepage.to_string(), "text/html", trans) {
                            let tx_modal = app_state.tx_modal.clone();
                            let error_message = ex.to_string();
                            let builder = thread::Builder::new().name("entrusted_modal_thread".into());

                            let _ = builder.spawn(move || {
                                tx_modal.send(ModalEvent::ErrorOccured(error_message)).unwrap();
                            });
                        }
                    }
                }
            });
        });

        ui.add_space(VERTICAL_GAP);

        let current_state = app_state.clone();

        ui.vertical(|ui| {
            ui.label(trans.gettext("Here is what you can do:"));
            ui.add_space(VERTICAL_GAP);

            fn add_link(ui: &mut egui::Ui, current_state: &AppState, title: String, screen_id: ScreenId) {
                ui.horizontal(|ui| {
                    ui.label("- ");

                    if ui.link(title).clicked() {
                        move_to_screen(screen_id, current_state);
                    }
                });
            }

            add_link(ui, &current_state, trans.gettext("Upload files"),          ScreenId::Upload);
            add_link(ui, &current_state, trans.gettext("Adjust settings"),       ScreenId::Settings);
            add_link(ui, &current_state, trans.gettext("Consult documentation"), ScreenId::Documentation);

            ui.horizontal(|ui| {
                ui.label("- ");
                if ui.link(trans.gettext("Report an issue")).clicked() {
                    const ISSUES_URL: &str = "https://github.com/rimerosolutions/entrusted/issues";

                    if let Err(ex) = utils::open_with_default_application(ISSUES_URL.to_string(), "text/html", trans) {
                        let tx_modal = app_state.tx_modal.clone();
                        let ctx = ctx.clone();
                        let error_message = trans.gettext_fmt("Could not open project issues URL: {0}! {1}", vec![ISSUES_URL, &ex.to_string()]);
                        let builder = thread::Builder::new().name("entrusted_modal_thread".into());

                        let _ = builder.spawn(move || {
                            let _ = tx_modal.send(ModalEvent::ErrorOccured(error_message));
                            ctx.request_repaint();
                        });
                    }
                }
            });
        });
    }

    fn id(&self) -> ScreenId {
        ScreenId::Welcome
    }
}

fn modal_show_filedialog(ctx: &egui::Context, frame: &mut eframe::Frame, app_state: &mut AppState) {
    let trans = &app_state.trans;

    let selected_files = rfd::FileDialog::new()
        .set_parent(frame)
        .set_title(trans.gettext("Select files"))
        .add_filter(trans.gettext("Known files"),    &["pdf", "doc", "docx", "odt", "rtf", "odp", "ppt", "pptx", "ods", "xls", "xlsx", "odg", "epub", "mobi", "png", "jpeg", "gif", "tiff", "pnm", "bmp"])
        .add_filter(trans.gettext("PDF documents"),  &["pdf"])
        .add_filter(trans.gettext("Text documents"), &["doc", "docx", "odt", "rtf"])
        .add_filter(trans.gettext("Presentations"),  &["odp", "ppt", "pptx"])
        .add_filter(trans.gettext("Spreadsheets"),   &["ods", "xls", "xlsx"])
        .add_filter(trans.gettext("Drawing"),        &["odg"])
        .add_filter(trans.gettext("Ebooks"),         &["epub", "mobi", "cbz", "fb2"])
        .add_filter(trans.gettext("Images"),         &["png", "jpeg", "gif", "tiff", "pnm", "bmp"])
        .add_filter(trans.gettext("All files"),      &["*"])
        .pick_files();

    if let Some(picked_files) = selected_files {
        let mut entries = Vec::with_capacity(picked_files.len());

        for picked_file in picked_files.iter() {
            entries.push(FileUploadEntryAddFiles::new(picked_file.clone(), FileUploadOptions::default()));
        }

        let tx = app_state.tx_gui.clone();
        let ctx = ctx.clone();
        let builder = thread::Builder::new().name("entrusted_filepicker_thread".into());

        let _ = builder.spawn(move || {
            let _ = tx.send(GuiEvent::FilesUploaded(entries));
            ctx.request_repaint();
        });
    }

    app_state.modal_shown = false;
}

fn modal_show_processing_options(ctx: &egui::Context, app_state: &mut AppState) {
    let trans = &app_state.trans;

    egui::Window::new(trans.gettext("Configure PDF output"))
        .default_size([480.0, 380.0])
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .collapsible(false)
        .resizable(false)
        .open(&mut app_state.modal_shown)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(trans.gettext("Output location"));

                if ui.button(trans.gettext("Browse...")).clicked() {
                    let selected_folder_opt = rfd::FileDialog::new().set_title(trans.gettext("Select output folder")).pick_folder();

                    if let Some(ref selected_folder) = selected_folder_opt {
                        app_state.modal_processing_data.1.output_folder = selected_folder.display().to_string();
                    }
                }
            });

            ui.add(egui::TextEdit::singleline(&mut app_state.modal_processing_data.1.output_folder).desired_width(200.0).hint_text(trans.gettext("Default if blank")));

            ui.label(trans.gettext("Password to decrypt input"));
            ui.add(egui::TextEdit::singleline(&mut app_state.modal_processing_data.1.password_decrypt).desired_width(200.0).password(true).hint_text(trans.gettext("None if blank")));

            ui.label(trans.gettext("Password to encrypt output"));
            ui.add(egui::TextEdit::singleline(&mut app_state.modal_processing_data.1.password_encrypt).desired_width(200.0).password(true).hint_text(trans.gettext("None if blank")));
        });

    if !app_state.modal_shown {
        let index = app_state.modal_processing_data.0;
        let processing_options = app_state.modal_processing_data.1.clone();
        let tx = app_state.tx_gui.clone();
        let builder = thread::Builder::new().name("entrusted_upload_notification_thread".into());

        let _ = builder.spawn(move || {
            let _ = tx.send(GuiEvent::FileUploadOptionsUpdated((index, processing_options)));
        });
    }
}

fn modal_show_text(ctx: &egui::Context, app_state: &mut AppState, window_title: String) {
    egui::Window::new(window_title)
        .default_size([480.0, 380.0])
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .collapsible(false)
        .resizable(false)
        .open(&mut app_state.modal_shown)
        .show(ctx, |ui| {
            egui::ScrollArea::both().max_width(450.0).max_height(270.0).show(ui, |ui| {
                let mut data = app_state.modal_text_data.clone();
                ui.add(egui::TextEdit::multiline(&mut data));
            });
        });
}

fn modal_show_logs(ctx: &egui::Context, app_state: &mut AppState) {
    let trans = &app_state.trans;
    modal_show_text(ctx, app_state, trans.gettext("Log"));
}

fn modal_show_error(ctx: &egui::Context, app_state: &mut AppState) {
    let trans = &app_state.trans;
    modal_show_text(ctx, app_state, trans.gettext("Error!"));
}

fn handle_modal_events(ctx: &egui::Context, frame: &mut eframe::Frame, app_state: &mut AppState) {
    let rx_modal = app_state.rx_modal.clone();

    if let Ok(modal_event) = rx_modal.borrow_mut().try_recv() {
        match modal_event {
            ModalEvent::FileDialog => {
                app_state.modal_shown = true;
                app_state.modal_display_state = ModalDisplayState::FileDialog;
            },
            ModalEvent::Log(msg) => {
                app_state.modal_shown = true;
                app_state.modal_display_state = ModalDisplayState::Log;
                app_state.modal_text_data = msg.clone();
            },
            ModalEvent::ProcessingOptions((index, upload_options)) => {
                app_state.modal_shown = true;
                app_state.modal_display_state = ModalDisplayState::ProcessingOptions;
                app_state.modal_processing_data = (index, upload_options);
            },
            ModalEvent::ErrorOccured(msg) => {
                app_state.modal_shown = true;
                app_state.modal_display_state = ModalDisplayState::ErrorOccured;
                app_state.modal_text_data = msg.clone();
            },
        }
    }

    if app_state.modal_shown {
        match app_state.modal_display_state {
            ModalDisplayState::FileDialog => {
                modal_show_filedialog(ctx, frame, app_state);
            },
            ModalDisplayState::ProcessingOptions => {
                modal_show_processing_options(ctx, app_state);
            },
            ModalDisplayState::Log => {
                modal_show_logs(ctx, app_state);
            },
            ModalDisplayState::ErrorOccured => {
                modal_show_error(ctx, app_state);
            },
            _ => {},
        }
    }
}

fn handle_dropped_files(ctx: &egui::Context, tx: mpsc::Sender<GuiEvent>, files: Vec<egui::DroppedFile>) {
    let tx  = tx.clone();
    let ctx = ctx.clone();
    let builder = thread::Builder::new().name("entrusted_upload_notification_thread".into());

    let _ = builder.spawn(move || {
        let mut entries = Vec::with_capacity(files.len());

        for file in files.iter() {
            if let Some(ref path) = file.path {
                if !path.is_dir() {
                    entries.push(FileUploadEntryAddFiles::new(path.clone(), FileUploadOptions::default()));
                }
            }
        }

        if !entries.is_empty() {
            let _ = tx.send(GuiEvent::FilesUploaded(entries));
            ctx.request_repaint();
        }
    });
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let screens = &mut self.screens;
        let screen_id  = self.app_state.current_screen_id.borrow_mut().clone();
        let idx = &screens.iter().position(|x| x.id() == screen_id).unwrap_or(0);
        let mut app_state = self.app_state.clone();
        let tx = app_state.tx_gui.clone();
        let app_state_clone = app_state.clone();
        let mut theme_set = app_state_clone.theme_set.borrow_mut();

        if !*theme_set {
            let theme = Into::<egui::ThemePreference>::into(app_state.ui_theme.clone());
            ctx.set_theme(theme);
            *theme_set = true;
        }

        handle_modal_events(ctx, frame, &mut self.app_state);

        let converting: bool = *app_state.converting.borrow();

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.app_state.modal_shown {
                ui.disable();
            }

            let screen_index = *idx;
            let navbar = &self.navbar;
            navbar.render(ctx, ui, &mut app_state, screen_index != 0 && !converting);

            let current_screen = &mut screens[screen_index];
            current_screen.render(ctx, ui, &mut app_state);
        });

        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() && !converting && screen_id == ScreenId::Upload {
                handle_dropped_files(ctx, tx, i.raw.dropped_files.clone());
            }
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let args: Vec<String> = env::args().collect();
    let exe_path = path::PathBuf::from(&args[0]);

    let app_config_ret = config::load_config();
    let app_config: config::AppConfig = app_config_ret.unwrap_or_default();

    l10n::load_translations(incl_gettext_files!("en", "fr"));
    let locale = env::var(l10n::ENV_VAR_ENTRUSTED_LANGID).unwrap_or(l10n::sys_locale());
    let trans = l10n::new_translations(locale);

    let frame_icon = eframe::icon_data::from_png_bytes(ICON_LOGO).expect("Invalid frame icon data!");
    let logo = egui::ColorImage::from(frame_icon.clone());
    let icon: Arc<egui::IconData> = Arc::new(frame_icon);
    let window_size: egui::Vec2 = egui::Vec2::new(500.0, 460.0);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(window_size)
            .with_icon(icon)
            .with_min_inner_size(window_size),
        ..Default::default()
    };

    let ui_theme = UiTheme::from(app_config.ui_theme);
    let window_title = capitalize_first_letter(option_env!("CARGO_PKG_NAME").unwrap_or("Unknown"));

    eframe::run_native(&window_title, options, Box::new(|cc| {
        cc.egui_ctx.set_pixels_per_point(1.2);
        let texture = cc.egui_ctx.load_texture("logo_entrusted", logo, Default::default());
        let sanitizer = sanitizer::Sanitizer::new(platform::resolve_sanitizer_settings(exe_path));

        // TODO multiple language codes can be selected and enabled in user preferences
        // Need multiple default selection indices
        let default_ocr_lang_code = "eng";
        let mut ocr_lang_initial_selection: usize = 0;
        let ocr_lang_by_code = l10n::ocr_lang_key_by_name(&trans);
        let mut ocr_langs = Vec::with_capacity(ocr_lang_by_code.len());

        for (ocr_lang_code, ocr_lang_name) in ocr_lang_by_code.iter() {
            ocr_langs.push(OcrLang::new(ocr_lang_code.to_string(), ocr_lang_name.to_string()));
        }

        ocr_langs.sort();

        let mut ocr_lang_codes_selections = vec![false; ocr_langs.len()];

        for (i, item) in ocr_langs.iter().enumerate() {
            if item.id == default_ocr_lang_code {
                ocr_lang_codes_selections[i] = true;
                ocr_lang_initial_selection = i;
                break;
            }
        }

        let convert_options = common::ConvertOptions::new(None,
                                                          common::DEFAULT_FILE_SUFFIX.to_string(),
                                                          VisualQuality::default_value(),
                                                          None,
                                                          None,
                                                          None);
        Ok(Box::new(App::new(ui_theme, texture, convert_options, sanitizer, ocr_lang_initial_selection, ocr_lang_codes_selections, ocr_langs, trans)))
    }))
}

// TODO: Need to account for Flatpak packaging at some point (ashpd)
// See https://github.com/bilelmoussaoui/ashpd
mod utils {
    use crate::l10n;

    pub fn open_with_default_application(path: String, content_type: &str, trans: &l10n::Translations) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(not(any(target_os = "macos", target_os = "windows")))] {
            linux::open_with_default_application(path, content_type, trans)
        }

        #[cfg(target_os = "macos")] {
            macos::open_with_default_application(path, content_type, trans)
        }

        #[cfg(target_os = "windows")] {
            windows::open_with_default_application(path, content_type, trans)
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    mod linux {
        use crate::l10n;

        pub fn open_with_default_application(path: String, content_type: &str, trans: &l10n::Translations) -> Result<(), Box<dyn std::error::Error>> {
            use xdg_utils::query_default_app;

            if let Ok(cmd) = query_default_app(content_type) {
                match std::process::Command::new(cmd).arg(path).spawn() {
                    Ok(_)   => Ok(()),
                    Err(ex) => Err(ex.into())
                }
            } else {
                Err(trans.gettext("Cannot find default application for this file type!").into())
            }
        }
    }

    #[cfg(target_os = "windows")]
    mod windows {
        use std::{
            ffi::OsStr,
            iter,
            os::windows::ffi::OsStrExt,
            ptr
        };

        use windows::{
            core::{PWSTR, PCWSTR},
            Win32::Foundation::GetLastError,
            Win32::UI::Shell::ShellExecuteW,
            Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD,
            Win32::System::Diagnostics::Debug::{
                FormatMessageW, FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS, FORMAT_MESSAGE_ALLOCATE_BUFFER
            },
            Win32::Globalization::GetUserDefaultLangID
        };

        use crate::l10n;

        fn get_last_error_as_string() -> String {
            unsafe {
                let buffer = PWSTR(ptr::null_mut());
                let error_code = GetLastError();
                let flags = FORMAT_MESSAGE_ALLOCATE_BUFFER
                    | FORMAT_MESSAGE_FROM_SYSTEM
                    | FORMAT_MESSAGE_IGNORE_INSERTS;

                let lang_id = GetUserDefaultLangID();

                let size = FormatMessageW(
                    flags,
                    None,
                    error_code.0,
                    lang_id.into(),
                    buffer,
                    0,
                    None,
                );

                if size == 0 || buffer.is_null() {
                    return String::new();
                }

                let message = String::from_utf16_lossy(std::slice::from_raw_parts(buffer.0, size as usize));

                message
            }
        }

        fn str_to_u16(path: &str) -> Vec<u16> {
            OsStr::new(path)
                .encode_wide()
                .chain(iter::once(0))
                .collect()
        }

        pub fn open_with_default_application(path: String, _content_type: &str, _: &l10n::Translations) -> Result<(), Box<dyn std::error::Error>> {
            let operation_u16 = str_to_u16("open");
            let operation     = PCWSTR::from_raw(operation_u16.as_ptr());
            let file_u16      = str_to_u16(&path);
            let file_path     = PCWSTR::from_raw(file_u16.as_ptr());

            unsafe {
                let result = ShellExecuteW(None, operation, file_path, None, None, SHOW_WINDOW_CMD(1));

                if result.0 as usize > 32 {
                    return Ok(());
                }

	            Err(get_last_error_as_string().into())
            }
        }
    }

    #[cfg(target_os = "macos")]
    mod macos {
        use crate::l10n;

        pub fn open_with_default_application(path: String, _content_type: &str, _: &l10n::Translations) -> Result<(), Box<dyn std::error::Error>> {
            if let Err(ex) = std::process::Command::new("open").arg(path).spawn() {
                return Err(ex.into())
            }

            Ok(())
        }
    }
}
