use clap::{Command, Arg, builder::PossibleValue};
use std::env;
use mupdf::{Image, Colorspace, ImageFormat, Matrix, Document };
use mupdf::pdf:: {PdfDocument, PdfWriteOptions};
use mupdf::pdf::document::Encryption;
use mupdf::document_writer::DocumentWriter;
use uuid::Uuid;
use std::collections::HashMap;
use std::error::Error;
use std::ffi::CString;

use std::fs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Instant;
use std::sync::atomic::{AtomicBool, Ordering};
use libreofficekit::{Office, OfficeOptionalFeatures, CallbackType, DocUrl};
use once_cell::sync::OnceCell;
use file_format::FileFormat;
use entrusted_l10n as l10n;
use std::rc::Rc;

const LOG_FORMAT_PLAIN: &str = "plain";
const LOG_FORMAT_JSON: &str  = "json";

const IMAGE_QUALITY_CHOICES: [&str; 3] = ["low", "medium", "high"];
const IMAGE_QUALITY_CHOICE_DEFAULT_INDEX: usize = 1;

const DEFAULT_DIR_TESSERACT_TESSDATA: &str  = "/usr/share/tesseract-ocr/4.00/tessdata";
const DEFAULT_DIR_LIBREOFFICE_PROGRAM: &str = "/usr/lib/libreoffice/program";

const ENV_VAR_ENTRUSTED_TESSERACT_TESSDATA_DIR: &str  = "ENTRUSTED_TESSERACT_TESSDATA_DIR";
const ENV_VAR_ENTRUSTED_LIBREOFFICE_PROGRAM_DIR: &str = "ENTRUSTED_LIBREOFFICE_PROGRAM_DIR";
const ENV_VAR_ENTRUSTED_DOC_PASSWD: &str              = "ENTRUSTED_DOC_PASSWD";

// See https://www.a4-size.com/a4-size-in-pixels/?size=a4&unit=px&ppi=150
const TARGET_DPI_LOW: f32    = 96.0;
const TARGET_DPI_MEDIUM: f32 = 150.0;
const TARGET_DPI_HIGH: f32   = 300.0;

static INSTANCE_DEFAULT_VISUAL_QUALITY: OnceCell<String> = OnceCell::new();

macro_rules! incl_gettext_files {
    ( $( $x:expr ),* ) => {
        {
            let mut ret = HashMap::with_capacity(2);
            $(
                let data = include_bytes!(concat!("../translations/", $x, "/LC_MESSAGES/messages.mo")).as_slice();
                ret.insert($x, data);
            )*

            ret
        }
    };
}

struct TessSettings<'a> {
    lang: &'a str,     // tesseract lang code
    data_dir: &'a str, // tesseract tessdata folder
}

fn default_visual_quality_to_str() -> &'static str {
    INSTANCE_DEFAULT_VISUAL_QUALITY.get().expect("INSTANCE_VISUAL_QUALITY value not set!")
}

struct ExecCtx {
    doc_uuid: String,
    root_tmp_dir: PathBuf,
    input_path: PathBuf,
    output_path: PathBuf,
    visual_quality: String,
    ocr_lang: Option<String>,
    doc_passwd: Option<String>,
    l10n: l10n::Translations,
    logger: &'static dyn Fn(usize, String),
}

fn main() -> Result<(), Box<dyn Error>> {
    let timer = Instant::now();

    l10n::load_translations(incl_gettext_files!("en", "fr"));

    let locale = if let Ok(selected_locale) = env::var(l10n::ENV_VAR_ENTRUSTED_LANGID) {
        selected_locale
    } else {
        l10n::sys_locale()
    };

    let l10n = l10n::new_translations(locale);

    let help_input_filename = l10n.gettext("Input filename");
    let help_output_filename = l10n.gettext("Optional output filename defaulting to <filename>-entrusted.pdf.");
    let help_visual_quality = l10n.gettext("PDF result visual quality");
    let help_ocr_lang = l10n.gettext("Optional language for OCR (i.e. 'eng' for English)");
    let help_log_format = l10n.gettext("Log format (json or plain)");

    let cmd_help_template = l10n.gettext(&format!("{}\n{}\n{}\n\n{}\n\n{}\n{}",
        "{bin} {version}",
        "{author}",
        "{about}",
        "Usage: {usage}",
        "Options:",
        "{options}"));

    INSTANCE_DEFAULT_VISUAL_QUALITY.set(IMAGE_QUALITY_CHOICES[IMAGE_QUALITY_CHOICE_DEFAULT_INDEX].to_string())?;

    let app = Command::new(option_env!("CARGO_PKG_NAME").unwrap_or("Unknown"))
        .version(option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown"))
        .help_template(cmd_help_template)
        .author(option_env!("CARGO_PKG_AUTHORS").unwrap_or("Unknown"))
        .about(option_env!("CARGO_PKG_DESCRIPTION").unwrap_or("Unknown"))
        .arg(
            Arg::new("input-filename")
                .long("input-filename")
                .help(help_input_filename)
                .required(false)
                .default_value("/tmp/input_file")
        ).arg(
            Arg::new("output-filename")
                .long("output-filename")
                .help(help_output_filename)
                .required(false)
                .default_value("/safezone/safe-output-compressed.pdf")
        ).arg(
            Arg::new("ocr-lang")
                .long("ocr-lang")
                .help(help_ocr_lang)
                .required(false)
        ).arg(
            Arg::new("log-format")
                .long("log-format")
                .help(help_log_format)
                .value_parser([
                    PossibleValue::new(LOG_FORMAT_JSON),
                    PossibleValue::new(LOG_FORMAT_PLAIN)
                ])
                .default_value(LOG_FORMAT_JSON)
                .required(false)
        ).arg(
            Arg::new("visual-quality")
                .long("visual-quality")
                .help(help_visual_quality)
                .value_parser([
                    PossibleValue::new(IMAGE_QUALITY_CHOICES[0]),
                    PossibleValue::new(IMAGE_QUALITY_CHOICES[1]),
                    PossibleValue::new(IMAGE_QUALITY_CHOICES[2]),
                ])
                .default_value(default_visual_quality_to_str())
                .required(false)
        );

    let run_matches = app.get_matches();

    let input_path = if let Some(v) = run_matches.get_one::<String>("input-filename") {
        PathBuf::from(v)
    } else {
        PathBuf::from("/tmp/input_file")
    };

    let output_path = if let Some(v) = run_matches.get_one::<String>("output-filename") {
        PathBuf::from(v)
    } else {
        PathBuf::from("/safezone/safe-output-compressed.pdf")
    };

    let ocr_lang = run_matches.get_one::<String>("ocr-lang").cloned();

    let visual_quality = if let Some(v) = run_matches.get_one::<String>("visual-quality") {
        v.clone()
    } else {
        default_visual_quality_to_str().to_string()
    };

    let log_format = if let Some(v) = run_matches.get_one::<String>("log-format") {
        v.clone()
    } else {
        LOG_FORMAT_JSON.to_string()
    };

    let doc_passwd = if let Ok(v) = env::var(ENV_VAR_ENTRUSTED_DOC_PASSWD) {
        if !v.is_empty() {
            Some(v)
        } else {
            None
        }
    } else {
        None
    };

    let doc_uuid     = Uuid::new_v4().to_string();
    let tmp_dir      = env::temp_dir();
    let root_tmp_dir = tmp_dir.join(&doc_uuid);

    let log_fn: &dyn Fn(usize, String) = match log_format.as_str() {
        "json" => {
            &log_json
        },
        _ => &log_plain
    };

    let ctx = ExecCtx {
        root_tmp_dir,
        doc_uuid,
        input_path,
        output_path,
        visual_quality,
        ocr_lang,
        doc_passwd,
        l10n: l10n.clone(),
        logger: log_fn
    };

    let mut exit_code = 0;
    let msg: String = if let Err(ex) = execute(ctx) {
        exit_code = 1;
        l10n.gettext_fmt("Conversion failed with reason: {0}", vec![&ex.to_string()])
    } else {
        l10n.gettext("Conversion succeeded!")
    };

    log_fn(99, msg);

    let millis = timer.elapsed().as_millis();
    log_fn(100, format!("{}: {}", l10n.gettext("Elapsed time"), elapsed_time_string(millis, l10n)));

    std::process::exit(exit_code);
}

fn execute(ctx: ExecCtx) -> Result<(), Box<dyn Error>> {
    let document_password = ctx.doc_passwd;
    let target_dpi = match ctx.visual_quality.as_str() {
        "low"    => TARGET_DPI_LOW,
        "medium" => TARGET_DPI_MEDIUM,
        "high"   => TARGET_DPI_HIGH,
        _        => TARGET_DPI_MEDIUM
    };
    let doc_uuid = ctx.doc_uuid;
    let l10n = ctx.l10n;

    let logger = ctx.logger;

    let root_tmp_dir     = ctx.root_tmp_dir;
    let raw_input_path   = ctx.input_path;
    let output_file_path = root_tmp_dir.join(format!("{}.pdf", doc_uuid));
    let output_dir_path  = root_tmp_dir.clone();
    let safe_dir_path    = ctx.output_path;

    if let Err(ex) = fs::create_dir_all(&root_tmp_dir) {
        return Err(l10n.gettext_fmt("Cannot temporary folder: {0}! Error: {1}", vec![&root_tmp_dir.display().to_string(), &ex.to_string()]).into());
    }

    // step 1 (0%-20%)
    let mut progress_range = ProgressRange::new(0, 20);
    let input_file_path = input_as_pdf_to_pathbuf_uri(target_dpi, &*logger, &progress_range, raw_input_path, document_password.clone(), l10n.clone())?;

    let doc = PdfDocument::open(&input_file_path)?;
    let page_count = doc.page_count()? as usize;

    // step 2 (20%-45%)
    progress_range.update(20, 45);
    split_pdf_pages_into_images(&*logger, &progress_range, page_count, doc, target_dpi, output_dir_path.clone(), l10n.clone())?;

    // step 3 (45%-90%)
    progress_range.update(45, 90);

    if let Some(v) = ctx.ocr_lang {
        let ocr_lang_text = v.as_str();
        let selected_langcodes: Vec<&str> = ocr_lang_text.split('+').collect();

        for selected_langcode in selected_langcodes {
            if !l10n::ocr_lang_key_by_name(&l10n).contains_key(&selected_langcode) {
                return Err(l10n.gettext_fmt("Unknown language code for the ocr-lang parameter: {0}. Hint: Try 'eng' for English.", vec![selected_langcode]).into());
            }
        }

        let provided_tessdata_dir = if let Ok(tessdata_dir) = env::var(ENV_VAR_ENTRUSTED_TESSERACT_TESSDATA_DIR) {
            tessdata_dir
        } else {
            DEFAULT_DIR_TESSERACT_TESSDATA.to_string()
        };

        let tess_settings = TessSettings {
            lang: ocr_lang_text,
            data_dir: &provided_tessdata_dir
        };

        ocr_imgs_to_pdf(&*logger, &progress_range, page_count, tess_settings, output_dir_path.clone(), output_dir_path.clone(), l10n.clone())?;
    } else {
        imgs_to_pdf(&*logger, &progress_range, page_count, output_dir_path.clone(), output_dir_path.clone(), target_dpi, l10n.clone())?;
    }

    // step 4 (90%-98%)
    progress_range.update(90, 98);
    pdf_combine_pdfs(&*logger, &progress_range, page_count, output_dir_path, output_file_path.clone(), l10n.clone())?;

    // step 5 (98%-98%)
    progress_range.update(98, 98);
    move_file_to_dir(&*logger, &progress_range, output_file_path, safe_dir_path, l10n)
}

fn move_file_to_dir(log_fn: &dyn Fn(usize, String), progress_range: &ProgressRange, src_file_path: PathBuf, dest_dir_path: PathBuf, l10n: l10n::Translations) -> Result<(), Box<dyn Error>> {
    if let Err(ex) = fs::copy(&src_file_path, &dest_dir_path) {
        log_fn(progress_range.min, l10n.gettext_fmt("Failed to copy file from {0} to {1}", vec![&src_file_path.display().to_string(), &dest_dir_path.display().to_string()]));
        return Err(ex.into());
    }

    if let Err(ex) = fs::remove_file(&src_file_path) {
        log_fn(progress_range.min, l10n.gettext_fmt("Failed to remove file from {0}.", vec![&src_file_path.display().to_string()]));
        return Err(ex.into());
    }

    log_fn(progress_range.min, l10n.gettext("Moving output files to their final destination"));

    Ok(())
}

fn input_as_pdf_to_pathbuf_uri(target_dpi: f32, log_fn: &dyn Fn(usize, String), _: &ProgressRange, raw_input_path: PathBuf, opt_passwd: Option<String>, l10n: l10n::Translations) -> Result<String, Box<dyn Error>> {
    if !raw_input_path.exists() {
        return Err(l10n.gettext_fmt("Cannot find file at {0}", vec![&raw_input_path.display().to_string()]).into());
    }

    let file_format = FileFormat::from_file(&raw_input_path)?;

    if let Some(mime_type) = file_format.short_name() {
        if let Some(parent_dir) = raw_input_path.parent() {
            let filename_pdf: String = {
                if let Some(basename) = raw_input_path.file_stem().and_then(|i| i.to_str()) {
                    let input_name = format!("{}_input.pdf", basename);
                    parent_dir.join(input_name.as_str()).display().to_string()
                } else {
                    return Err(l10n.gettext_fmt("Could not determine basename for file {0}", vec![&raw_input_path.display().to_string()]).into());
                }
            };

            match mime_type {
                "PDF" => {
                    log_fn(5, l10n.gettext_fmt("Copying PDF input to {0}", vec![&filename_pdf]));
                    let path_loc = raw_input_path.display().to_string();

                    if let Some(passwd) = opt_passwd {
                        let mut ret_doc = PdfDocument::open(&path_loc)?;

                        if ret_doc.needs_password()? {
                            ret_doc.authenticate(&passwd)?;                            
                            let mut binding = PdfWriteOptions::default();
                            let options = binding.set_pretty(false).set_encryption(Encryption::None).set_compress(true).set_garbage_level(4);                            
                            ret_doc.save_with_options(&filename_pdf, *options)?;
                        } else {
                            fs::copy(raw_input_path, &filename_pdf)?;
                        }
                    } else {
                        fs::copy(raw_input_path, &filename_pdf)?;
                    }                    
                },
                "BMP" | "PNM" | "PNG" | "JPG" | "GIF" | "TIFF" => {
                    log_fn(5, l10n.gettext("Converting input image to PDF"));
                    img_to_pdf(target_dpi, raw_input_path, PathBuf::from(&filename_pdf))?;
                },
                "DOC" | "DOCX" | "ODG" | "ODP" | "ODS" | "ODT" | "PPT" | "PPTX" | "RTF" | "XLS" | "XLSX" => {
                    log_fn(5, l10n.gettext("Converting to PDF using LibreOffice"));
                    let fileext = mime_type.to_lowercase();
                    let new_input_loc = format!("/tmp/input.{}", fileext);
                    let new_input_path = Path::new(&new_input_loc);
                    fs::copy(raw_input_path, new_input_path)?;

                    let libreoffice_program_dir = if let Ok(env_libreoffice_program_dir) = env::var(ENV_VAR_ENTRUSTED_LIBREOFFICE_PROGRAM_DIR) {
                        env_libreoffice_program_dir
                    } else {
                        DEFAULT_DIR_LIBREOFFICE_PROGRAM.to_string()
                    };

                    let office = Office::new(&libreoffice_program_dir)?;
                    let input_uri = DocUrl::from_absolute_path(new_input_path.display().to_string())?;
                    let needs_password = Rc::new(AtomicBool::new(false));

                    if let Some(passwd) = opt_passwd {
                        if let Err(ex) = office.set_optional_features(OfficeOptionalFeatures::DOCUMENT_PASSWORD) {
                            return Err(l10n.gettext_fmt("Failed to enable password-protected Office document features! {0}", vec![&ex.to_string()]).into());
                        }

                        if let Err(ex) = office.register_callback({
                            let needs_password = needs_password.clone();
                            let input_uri = input_uri.clone();

                            move |office, ty, _| {
                                if let CallbackType::DocumentPassword = ty {
                                    if needs_password.swap(true, Ordering::SeqCst) {
                                        let _ = office.set_document_password(&input_uri, None);
                                        return;
                                    }

                                    
                                    let _ = office.set_document_password(&input_uri, Some(&passwd));
                            }
                        }
                        }) {
                            return Err(l10n.gettext_fmt("Failed to handle password-protected Office document features! {0}", vec![&ex.to_string()]).into());
                        }
                    }

                    let res_document_saved: Result<(), Box<dyn Error>> = match office.document_load(&input_uri) {
                        Ok(mut doc) => {
                            match DocUrl::from_absolute_path(&filename_pdf) {
                                Err(ex) => {
                                    let msg = ex.to_string();
                                    Err(l10n.gettext_fmt("Could not save document as PDF: {0}", vec![&msg]).into())
                                }
                                Ok(doc_url) => {
                                    if let Err(ex) = doc.save_as(&doc_url, "pdf", None) {
                                        let msg = ex.to_string();
                                        Err(l10n.gettext_fmt("Could not save document as PDF: {0}", vec![&msg]).into())
                                    } else {
                                        Ok(())
                                    }
                            }                                
                        }
                        },
                        Err(ex) =>  {                            
                            Err(ex.to_string().into())
                        }
                    };


                    if let Err(ex) = res_document_saved {
                        return Err(l10n.gettext_fmt("Could not export input document as PDF! {0}", vec![&ex.to_string()]).into());
                    }
                },
                &_ => {
                    return Err(l10n.gettext("Mime type error! Does the input have a 'known' file extension?").into());
                }
            }
            
            Ok(filename_pdf)
        } else {
            Err(l10n.gettext("Cannot find input parent directory!").into())
        }
    } else {
        Err(l10n.gettext("Mime type error! Does the input have a 'known' file extension?").into())
    }
}

#[inline]
fn elapsed_time_string(millis: u128, l10n: l10n::Translations) -> String {
    let seconds = millis / 1000;
    let minutes = millis / (60 * 1000);
    let hours   = millis / (60 * 60 * 1000);

    format!("{} {} {}",
        l10n.ngettext("hour",   "hours",   hours   as u64),
        l10n.ngettext("minute", "minutes", minutes as u64),
        l10n.ngettext("second", "seconds", seconds as u64))
}

fn ocr_imgs_to_pdf(
    log_fn: &dyn Fn(usize, String),
    progress_range: &ProgressRange,
    page_count: usize,
    tess_settings: TessSettings,
    input_path: PathBuf,
    output_path: PathBuf,
    l10n: l10n::Translations
) -> Result<(), Box<dyn Error>> {
    let progress_delta = progress_range.delta();
    let mut progress_value: usize = progress_range.min;
    log_fn(progress_value, l10n.ngettext("Performing OCR to PDF on one image", "Performing OCR to PDF on few images", page_count as u64));

    let api = tesseract_init(tess_settings.lang, tess_settings.data_dir);

    for i in 0..page_count {
        let page_num = i + 1;
        progress_value = progress_range.min + (page_num * progress_delta / page_count);
        let page_num_text = page_num.to_string();
        log_fn(progress_value, l10n.gettext_fmt("Performing OCR on page {0}", vec![&page_num_text]));
        let src = input_path.join(format!("page-{}.png", page_num));
        let dest = output_path.join(format!("page-{}", page_num));
        ocr_img_to_pdf(api, src, dest)?;
    }

    tesseract_delete(api);

    Ok(())
}

fn tesseract_init(ocr_lang: &str, tessdata_dir: &str) -> *mut tesseract_plumbing::tesseract_sys::TessBaseAPI {
    let c_lang = CString::new(ocr_lang).unwrap();
    let lang = c_lang.as_bytes().as_ptr() as *mut std::os::raw::c_char;

    let c_datapath = CString::new(tessdata_dir).unwrap();
    let datapath = c_datapath.as_bytes().as_ptr() as *mut std::os::raw::c_char;

    let c_user_defined_dpi_var_name = CString::new("user_defined_dpi").unwrap();
    let user_defined_dpi_var_name = c_user_defined_dpi_var_name.as_bytes().as_ptr() as *mut std::os::raw::c_char;

    let c_user_defined_dpi_var_value = CString::new("72").unwrap();
    let user_defined_dpi_var_value = c_user_defined_dpi_var_value.as_bytes().as_ptr() as *mut std::os::raw::c_char;

    unsafe {
        let api = tesseract_plumbing::tesseract_sys::TessBaseAPICreate();
        tesseract_plumbing::tesseract_sys::TessBaseAPIInit3(api, datapath, lang);
        tesseract_plumbing::tesseract_sys::TessBaseAPISetVariable(api, user_defined_dpi_var_name, user_defined_dpi_var_value);

        api
    }
}

fn tesseract_delete(api: *mut tesseract_plumbing::tesseract_sys::TessBaseAPI) {
    unsafe {
        tesseract_plumbing::tesseract_sys::TessBaseAPIEnd(api);
        tesseract_plumbing::tesseract_sys::TessBaseAPIDelete(api);
    }
}

fn ocr_img_to_pdf(
    api: *mut tesseract_plumbing::tesseract_sys::TessBaseAPI,
    input_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), Box<dyn Error>> {
    let c_inputname = CString::new(input_path.display().to_string().as_str())?;
    let inputname = c_inputname.as_bytes().as_ptr() as *mut std::os::raw::c_char;

    let c_outputbase = CString::new(output_path.display().to_string().as_str())?;
    let outputbase = c_outputbase.as_bytes().as_ptr() as *mut std::os::raw::c_char;

    let c_input_name = CString::new(input_path.file_name().unwrap().to_str().unwrap()).unwrap();
    let input_name = c_input_name.as_bytes().as_ptr() as *mut std::os::raw::c_char;

    let do_not_care = CString::new("").unwrap().as_bytes().as_ptr() as *mut std::os::raw::c_char;

    unsafe {
        tesseract_plumbing::tesseract_sys::TessBaseAPISetInputName(api, input_name);
        tesseract_plumbing::tesseract_sys::TessBaseAPISetOutputName(api, outputbase);

        let renderer = tesseract_plumbing::tesseract_sys::TessPDFRendererCreate(
            outputbase,
            tesseract_plumbing::tesseract_sys::TessBaseAPIGetDatapath(api),
            0,
        );

        if !renderer.is_null() {
            let pix_path = c_inputname.as_c_str();
            let pix = tesseract_plumbing::leptonica_plumbing::Pix::read(pix_path)?;
            let lpix = *pix.as_ref();

            tesseract_plumbing::tesseract_sys::TessResultRendererBeginDocument(renderer, do_not_care);
            tesseract_plumbing::tesseract_sys::TessBaseAPIProcessPage(api, lpix, 1, inputname, do_not_care, 0, renderer);
            tesseract_plumbing::tesseract_sys::TessResultRendererEndDocument(renderer);

            lpix.drop_in_place();
        }

        tesseract_plumbing::tesseract_sys::TessDeleteResultRenderer(renderer);
    }

    Ok(())
}

fn log_plain(percent_complete: usize, data: String) {
    println!("{}% {}", percent_complete, data);
}

fn log_json(percent_complete: usize, data: String) {
    let progress_msg = ProgressMessage { percent_complete, data};

    if let Ok(progress_json) = serde_json::to_string(&progress_msg) {
        println!("{}", progress_json);
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct ProgressMessage {
    percent_complete: usize,
    data: String
}

struct ProgressRange {
    min: usize,
    max: usize
}

impl ProgressRange {
    fn new(min: usize, max: usize) -> Self {
        Self { min, max }
    }

    fn update(&mut self, new_min: usize, new_max: usize) {
        self.min = new_min;
        self.max = new_max;
    }

    fn delta(&self) -> usize {
        self.max - self.min
    }
}

fn split_pdf_pages_into_images(log_fn: &dyn Fn(usize, String), progress_range: &ProgressRange, page_count: usize, doc: PdfDocument, target_dpi: f32, dest_folder: PathBuf, l10n: l10n::Translations) -> Result<(), Box<dyn Error>> {
    let mut progress_value: usize = progress_range.min;

    log_fn(progress_value, l10n.ngettext("Extract PDF file into one image",
        "Extract PDF file into few images",
        page_count as u64));

    let progress_delta = progress_range.delta();

    for i in 0..page_count {
        let idx = (i + 1) as i32;
        let idx_text = idx.to_string();
        progress_value = progress_range.min + ((i + 1) * progress_delta / page_count);
        log_fn(progress_value, l10n.gettext_fmt("Extracting page {0} into a PNG image", vec![&idx_text]));
        
        let page = doc.load_page(idx - 1 )?;
        let matrix = Matrix::new_scale(target_dpi/72.0, target_dpi/72.0);
        let pixmap = page.to_pixmap(&matrix, &Colorspace::device_rgb(), 0.0, true)?;
        let dest_path = dest_folder.join(format!("page-{}.png", idx));
        let dest = dest_path.display().to_string();
        pixmap.save_as(&dest, ImageFormat::PNG)?;
    }

    Ok(())
}

fn pdf_combine_pdfs(log_fn: &dyn Fn(usize, String), progress_range: &ProgressRange, page_count: usize, input_dir_path: PathBuf, output_path: PathBuf, l10n: l10n::Translations) -> Result<(), Box<dyn Error>> {
    log_fn(progress_range.min,
        l10n.ngettext("Combining one PDF document",
            "Combining few PDF documents",
            page_count as u64));

    let step_count = 7;
    let mut step_num = 1;
    let progress_delta = progress_range.delta();

    // step 1/7
    let mut progress_value = progress_range.min + (step_num * progress_delta / step_count);
    log_fn(progress_value, l10n.gettext("Collecting PDF pages"));

    let mut output_doc = PdfDocument::new();
    let mut c = 0;
    
    for i in 0..page_count {        
        let src_path = input_dir_path.join(format!("page-{}.pdf", i + 1));
        let src_location = src_path.display().to_string();
        let src_doc = PdfDocument::open(&src_location)?;

        for page_num in 0..src_doc.page_count()? {
            let page = src_doc.find_page(page_num)?;
            let p = output_doc.graft_object(&page)?;
            output_doc.insert_page(c, &p)?;
            c += 1;
        }
    }

    // step 7/7 Save the merged PDF
    step_num += 1;
    progress_value = progress_range.min + (step_num * progress_delta / step_count);
    log_fn(progress_value, l10n.gettext("Saving PDF"));
    
    let mut binding = PdfWriteOptions::default();
    let options = binding.set_pretty(false).set_compress(true).set_garbage_level(4);

    let output_loc = output_path.display().to_string();

    if let Err(ex) = output_doc.save_with_options(&output_loc, *options) {
        return Err(l10n.gettext_fmt("Could not save PDF file to {0}. {1}.", vec![&output_loc, &ex.to_string()]).into());
    }

    if std::fs::metadata(&output_path).is_err() {
        return Err(l10n.gettext_fmt("Could not save PDF file to {0}.", vec![&output_loc]).into());
    }

    Ok(())
}

fn imgs_to_pdf(log_fn: &dyn Fn(usize, String), progress_range: &ProgressRange, page_count: usize, input_path: PathBuf, output_path: PathBuf, target_dpi: f32, l10n: l10n::Translations) -> Result<(), Box<dyn Error>> {
    let progress_delta = progress_range.delta();
    let mut progress_value: usize = progress_range.min;

    log_fn(progress_value, l10n.ngettext("Saving one PNG image to PDF",
        "Saving few PNG images to PDF",
        page_count as u64));

    for i in 0..page_count {
        let idx = i + 1;
        let idx_text = idx.to_string();
        progress_value = progress_range.min + (idx * progress_delta / page_count);
        log_fn(progress_value, l10n.gettext_fmt("Saving PNG image {0} to PDF", vec![&idx_text]));
        let src = input_path.join(format!("page-{}.png", &idx));
        let dest = output_path.join(format!("page-{}.pdf", &idx));
        img_to_pdf(target_dpi, src, dest)?;
    }

    Ok(())
}

fn img_to_pdf(target_dpi: f32, src_path: PathBuf, dest_path: PathBuf) -> Result<(), Box<dyn Error>> {
    let path_string = src_path.display().to_string();
    let dest_string = dest_path.display().to_string();
    let doc = Document::open(&path_string)?;
    let img = Image::from_file(&path_string)?;
    let options = format!("resolution={},height={},compress=true", target_dpi as i32, img.height());

    let mut writer = DocumentWriter::new(
        &dest_string,
        "pdf",
        &options
    )?;

    for i in 0..doc.page_count()? {
        let page = doc.load_page(i)?;
        let mediabox = page.bounds()?;
        let device = writer.begin_page(mediabox)?;
        page.run(&device, &Matrix::IDENTITY)?;
        writer.end_page(device)?;
    }

    Ok(())
}
