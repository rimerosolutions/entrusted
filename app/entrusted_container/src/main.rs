use cairo::{Context, Format, ImageSurface, PdfSurface};
use std::env;

use poppler::Document;
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::ffi::CString;
use std::fs;
use serde::{Deserialize, Serialize};
use std::io::{BufReader, Cursor, Seek, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use libreoffice_rs::{Office, LibreOfficeKitOptionalFeatures, urls};

use entrusted_l10n as l10n;

mod mimetypes;

const DEFAULT_DIR_TESSERACT_TESSDATA: &str  = "/usr/share/tesseract-ocr/4.00/tessdata";
const DEFAULT_DIR_LIBREOFFICE_PROGRAM: &str = "/usr/lib/libreoffice/program";

// TODO command line parameters of the program instead of vars?
const ENV_VAR_ENTRUSTED_DOC_PASSWD: &str              = "ENTRUSTED_DOC_PASSWD";
const ENV_VAR_LOG_FORMAT: &str                        = "ENTRUSTED_LOG_FORMAT";
const ENV_VAR_OCR_LANGUAGE: &str                      = "ENTRUSTED_OCR_LANGUAGE";
const ENV_VAR_ENTRUSTED_VISUAL_QUALITY: &str          = "ENTRUSTED_VISUAL_QUALITY";
const ENV_VAR_ENTRUSTED_TESSERACT_TESSDATA_DIR: &str  = "ENTRUSTED_TESSERACT_TESSDATA_DIR";
const ENV_VAR_ENTRUSTED_LIBREOFFICE_PROGRAM_DIR: &str = "ENTRUSTED_LIBREOFFICE_PROGRAM_DIR";

// A4 150PPI/DPI
// See https://www.a4-size.com/a4-size-in-pixels/?size=a4&unit=px&ppi=150
const IMAGE_SIZE_QUALITY_LOW: (f64, f64)    = (794.0  , 1123.0);
const IMAGE_SIZE_QUALITY_MEDIUM: (f64, f64) = (1240.0 , 1754.0);
const IMAGE_SIZE_QUALITY_HIGH: (f64, f64)   = (4961.0 , 7016.0);

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

#[derive(Clone, Debug)]
enum ConversionType {
    None,
    LibreOffice(&'static str), // file_extension
    Convert,
}

struct TessSettings<'a> {
    lang: &'a str,     // tesseract lang code
    data_dir: &'a str, // tesseract tessdata folder
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

    let document_password = if let Ok(passwd) = env::var(ENV_VAR_ENTRUSTED_DOC_PASSWD) {
        if !passwd.is_empty() {
            Some(passwd)
        } else {
            None
        }
    } else {
        None
    };

    let image_quality = if let Ok(env_img_quality) = env::var(ENV_VAR_ENTRUSTED_VISUAL_QUALITY) {
        let img_quality = env_img_quality.to_lowercase();
        let img_quality_str = img_quality.as_str();

        match img_quality_str {
            "low"    => IMAGE_SIZE_QUALITY_LOW,
            "medium" => IMAGE_SIZE_QUALITY_MEDIUM,
            "high"   => IMAGE_SIZE_QUALITY_HIGH,
            _        => IMAGE_SIZE_QUALITY_MEDIUM
        }
    } else {
        IMAGE_SIZE_QUALITY_MEDIUM
    };

    let logger: Box<dyn ConversionLogger> = if let Ok(dgz_logformat_value) = env::var(ENV_VAR_LOG_FORMAT) {
        match dgz_logformat_value.as_str() {
            "json" => {
                Box::new(JsonConversionLogger)
            },
            _ => Box::new(PlainConversionLogger)
        }
    } else {
        Box::new(PlainConversionLogger)
    };

    let ret = || -> Result<(), Box<dyn Error>> {
        let raw_input_path   = Path::new("/tmp/input_file");
        let output_file_path = Path::new("/tmp/safe-output-compressed.pdf");
        let output_dir_path  = Path::new("/tmp/");
        let safe_dir_path    = Path::new("/safezone/safe-output-compressed.pdf");

        // step 1 (0%-20%)
        let mut progress_range = ProgressRange::new(0, 20);
        let input_file_path = input_as_pdf_to_pathbuf_uri(&*logger, &progress_range, raw_input_path, document_password.clone(), l10n.clone())?;

        let doc = if let Some(passwd) = document_password {
            // We only care about originally encrypted PDF files
            // If the document was in another format, then it's already decrypted at this stage
            // Providing a password for a non-encrypted document doesn't fail which removes the need for additional logic and state handling
            Document::from_file(&input_file_path.display().to_string(), Some(&passwd))?
        } else {
            Document::from_file(&input_file_path.display().to_string(), None)?
        };

        let page_count = doc.n_pages() as usize;

        // step 2 (20%-45%)
        progress_range.update(20, 45);
        split_pdf_pages_into_images(&*logger, &progress_range, page_count, doc, image_quality, output_dir_path, l10n.clone())?;

        // step 3 (45%-90%)
        progress_range.update(45, 90);

        if let Ok(ocr_lang)= env::var(ENV_VAR_OCR_LANGUAGE) {
            let ocr_lang_text = ocr_lang.as_str();
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

            ocr_imgs_to_pdf(&*logger, &progress_range, page_count, tess_settings, output_dir_path, output_dir_path, l10n.clone())?;
        } else {
            imgs_to_pdf(&*logger, &progress_range, page_count, output_dir_path, output_dir_path, l10n.clone())?;
        }

        // step 4 (90%-98%)
        progress_range.update(90, 98);
        pdf_combine_pdfs(&*logger, &progress_range, page_count, output_dir_path, output_file_path, l10n.clone())?;

        // step 5 (98%-98%)
        progress_range.update(98, 98);
        move_file_to_dir(&*logger, &progress_range, output_file_path, safe_dir_path, l10n.clone())
    };

    let mut exit_code = 0;

    let msg: String = if let Err(ex) = ret() {
        exit_code = 1;
        l10n.gettext_fmt("Conversion failed with reason: {0}", vec![&ex.to_string()])
    } else {
        l10n.gettext("Conversion succeeded!")
    };

    logger.log(99, msg);

    let millis = timer.elapsed().as_millis();
    logger.log(100, format!("{}: {}", l10n.gettext("Elapsed time"), elapsed_time_string(millis, l10n)));

    std::process::exit(exit_code);
}

fn move_file_to_dir(logger: &dyn ConversionLogger, progress_range: &ProgressRange, src_file_path: &Path, dest_dir_path: &Path, l10n: l10n::Translations) -> Result<(), Box<dyn Error>> {
    if let Err(ex) = fs::copy(src_file_path, dest_dir_path) {
        logger.log(progress_range.min, l10n.gettext_fmt("Failed to copy file from {0} to {1}", vec![&src_file_path.display().to_string(), &dest_dir_path.display().to_string()]));
        return Err(ex.into());
    }

    if let Err(ex) = fs::remove_file(src_file_path) {
        logger.log(progress_range.min, l10n.gettext_fmt("Failed to remove file from {0}.", vec![&src_file_path.display().to_string()]));
        return Err(ex.into());
    }

    logger.log(progress_range.min, l10n.gettext("Moving output files to their final destination"));

    Ok(())
}

fn input_as_pdf_to_pathbuf_uri(logger: &dyn ConversionLogger, _: &ProgressRange, raw_input_path: &Path, opt_passwd: Option<String>, l10n: l10n::Translations) -> Result<PathBuf, Box<dyn Error>> {
    let conversion_by_mimetype: HashMap<&str, ConversionType> = [
        ("application/pdf", ConversionType::None),
        (
            "application/rtf",
            ConversionType::LibreOffice("rtf"),
        ),
        (
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            ConversionType::LibreOffice("docx"),
        ),
        (
            "application/vnd.ms-word.document.macroEnabled.12",
            ConversionType::LibreOffice("docm"),
        ),
        (
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            ConversionType::LibreOffice("xlsx"),
        ),
        (
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            ConversionType::LibreOffice("pptx"),
        ),
        ("application/msword", ConversionType::LibreOffice("doc")),
        (
            "application/vnd.ms-excel",
            ConversionType::LibreOffice("xls"),
        ),
        (
            "application/vnd.ms-powerpoint",
            ConversionType::LibreOffice("ppt"),
        ),
        (
            "application/vnd.oasis.opendocument.text",
            ConversionType::LibreOffice("odt"),
        ),
        (
            "application/vnd.oasis.opendocument.graphics",
            ConversionType::LibreOffice("odg"),
        ),

        (
            "application/vnd.oasis.opendocument.presentation",
            ConversionType::LibreOffice("odp"),
        ),
        (
            "application/vnd.oasis.opendocument.spreadsheet",
            ConversionType::LibreOffice("ods"),
        ),
        ("image/jpeg",   ConversionType::Convert),
        ("image/gif",    ConversionType::Convert),
        ("image/png",    ConversionType::Convert),
        ("image/tiff",   ConversionType::Convert),
        ("image/x-tiff", ConversionType::Convert),
    ]
        .iter()
        .cloned()
        .collect();

    if !raw_input_path.exists() {
        return Err(l10n.gettext_fmt("Cannot find file at {0}", vec![&raw_input_path.display().to_string()]).into());
    }

    if let Some(mime_type) = mimetypes::detect_from_path(raw_input_path)? {
        if let Some(conversion_type) = conversion_by_mimetype.get(mime_type) {
            if let Some(parent_dir) = raw_input_path.parent() {
                let filename_pdf: String = {
                    if let Some(basename) = raw_input_path.file_stem().and_then(|i| i.to_str()) {
                        let input_name = format!("{}_input.pdf", basename);
                        parent_dir.join(input_name.as_str()).display().to_string()
                    } else {
                        return Err(l10n.gettext_fmt("Could not determine basename for file {0}", vec![&raw_input_path.display().to_string()]).into());
                    }
                };

                match conversion_type {
                    ConversionType::None => {
                        logger.log(5, l10n.gettext_fmt("Copying PDF input to {0}", vec![&filename_pdf]));
                        fs::copy(raw_input_path, &filename_pdf)?;
                    }
                    ConversionType::Convert => {
                        logger.log(5, l10n.gettext("Converting input image to PDF"));

                        let img_format = match mime_type {
                            "image/png"    => Ok(image::ImageFormat::Png),
                            "image/jpeg"   => Ok(image::ImageFormat::Jpeg),
                            "image/gif"    => Ok(image::ImageFormat::Gif),
                            "image/tiff"   => Ok(image::ImageFormat::Tiff),
                            "image/x-tiff" => Ok(image::ImageFormat::Tiff),
                            unknown_img_t  => Err(l10n.gettext_fmt("Unsupported image type {0}", vec![unknown_img_t])),
                        }?;
                        img_to_pdf(img_format, raw_input_path, Path::new(&filename_pdf))?;
                    }
                    ConversionType::LibreOffice(fileext) => {
                        logger.log(5, l10n.gettext("Converting to PDF using LibreOffice"));
                        let new_input_loc = format!("/tmp/input.{}", fileext);
                        let new_input_path = Path::new(&new_input_loc);
                        fs::copy(raw_input_path, new_input_path)?;

                        let libreoffice_program_dir = if let Ok(env_libreoffice_program_dir) = env::var(ENV_VAR_ENTRUSTED_LIBREOFFICE_PROGRAM_DIR) {
                            env_libreoffice_program_dir
                        } else {
                            DEFAULT_DIR_LIBREOFFICE_PROGRAM.to_string()
                        };

                        let mut office = Office::new(&libreoffice_program_dir)?;
                        let input_uri = urls::local_into_abs(&new_input_path.display().to_string())?;
                        let password_was_set = AtomicBool::new(false);
                        let failed_password_input = Arc::new(AtomicBool::new(false));

                        if let Some(passwd) = opt_passwd {
                            if let Err(ex) = office.set_optional_features([LibreOfficeKitOptionalFeatures::LOK_FEATURE_DOCUMENT_PASSWORD]) {
                                return Err(l10n.gettext_fmt("Failed to enable password-protected Office document features! {0}", vec![&ex.to_string()]).into());
                            }

                            if let Err(ex) = office.register_callback({
                                let mut office = office.clone();
                                let failed_password_input = failed_password_input.clone();
                                let input_uri = input_uri.clone();

                                move |_, _| {
                                    if !password_was_set.load(Ordering::Acquire) {
                                        let _ = office.set_document_password(input_uri.clone(), &passwd);
                                        password_was_set.store(true, Ordering::Release);
                                    } else if !failed_password_input.load(Ordering::Acquire) {
                                        failed_password_input.store(true, Ordering::Release);
                                        let _ = office.unset_document_password(input_uri.clone());
                                    }
                                }
                            }) {
                                return Err(l10n.gettext_fmt("Failed to handle password-protected Office document features! {0}", vec![&ex.to_string()]).into());
                            }
                        }

                        let res_document_saved: Result<(), Box<dyn Error>> = match office.document_load(input_uri) {
                            Ok(mut doc) => {
                                if doc.save_as(&filename_pdf, "pdf", None) {
                                    Ok(())
                                } else {
                                    Err(l10n.gettext_fmt("Could not save document as PDF: {0}", vec![&office.get_error()]).into())
                                }
                            },
                            Err(ex) =>  {
                                let err_reason = if failed_password_input.load(Ordering::Relaxed) {
                                    l10n.gettext("Password input failed!")
                                } else {
                                    ex.to_string()
                                };

                                Err(err_reason.into())
                            }
                        };

                        if let Err(ex) = res_document_saved {
                            return Err(l10n.gettext_fmt("Could not export input document as PDF! {0}", vec![&ex.to_string()]).into());
                        }
                    }
                }

                Ok(PathBuf::from(format!("file://{}", filename_pdf)))
            } else {
                Err(l10n.gettext("Cannot find input parent directory!").into())
            }
        } else {
            Err(l10n.gettext_fmt("Unsupported mime type: {0}", vec![mime_type]).into())
        }
    } else {
        Err(l10n.gettext("Mime type error! Does the input have a 'known' file extension?").into())
    }
}

#[inline]
fn elapsed_time_string(millis: u128, l10n: l10n::Translations) -> String {
    let mut diff = millis;
    let secs_in_millis = 1000;
    let mins_in_millis = secs_in_millis * 60;
    let hrs_in_millis = mins_in_millis * 60;
    let hours = diff / hrs_in_millis;
    diff %= hrs_in_millis;
    let minutes = diff / mins_in_millis;
    diff %= mins_in_millis;
    let seconds = diff / secs_in_millis;

    format!("{} {} {}",
            l10n.ngettext("hour",   "hours",   hours   as u64),
            l10n.ngettext("minute", "minutes", minutes as u64),
            l10n.ngettext("second", "seconds", seconds as u64))
}

fn ocr_imgs_to_pdf(
    logger: &dyn ConversionLogger,
    progress_range: &ProgressRange,
    page_count: usize,
    tess_settings: TessSettings,
    input_path: &Path,
    output_path: &Path,
    l10n: l10n::Translations
) -> Result<(), Box<dyn Error>> {
    let progress_delta = progress_range.delta();
    let mut progress_value: usize = progress_range.min;
    logger.log(progress_value, l10n.ngettext("Performing OCR to PDF on one image", "Performing OCR to PDF on few images", page_count as u64));

    let api = tesseract_init(tess_settings.lang, tess_settings.data_dir);

    for i in 0..page_count {
        let page_num = i + 1;
        progress_value = progress_range.min + (page_num * progress_delta / page_count);
        let page_num_text = page_num.to_string();
        logger.log(progress_value, l10n.gettext_fmt("Performing OCR on page {0}", vec![&page_num_text]));
        let src = input_path.join(format!("page-{}.png", page_num));
        let dest = output_path.join(format!("page-{}", page_num));
        ocr_img_to_pdf(api, &src, &dest)?;
    }

    tesseract_delete(api);

    Ok(())
}

// TODO similar concept of preset for paper size, etc., but for the user defined DPI hard-coded to 72
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
    input_path: &Path,
    output_path: &Path,
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

trait ConversionLogger {
    fn log(&self, percent_complete: usize, data: String);
}

struct PlainConversionLogger;
struct JsonConversionLogger;

impl ConversionLogger for PlainConversionLogger {
    fn log(&self, percent_complete: usize, data: String) {
        println!("{}% {}", percent_complete, data);
    }
}

impl ConversionLogger for JsonConversionLogger {
    fn log(&self, percent_complete: usize, data: String) {
        let progress_msg = ProgressMessage { percent_complete, data};

        if let Ok(progress_json) = serde_json::to_string(&progress_msg) {
            println!("{}", progress_json);
        }
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

fn split_pdf_pages_into_images(logger: &dyn ConversionLogger, progress_range: &ProgressRange, page_count: usize, doc: Document, target_size: (f64, f64), dest_folder: &Path, l10n: l10n::Translations) -> Result<(), Box<dyn Error>> {
    let mut progress_value: usize = progress_range.min;

    logger.log(progress_value, l10n.ngettext("Extract PDF file into one image",
                                             "Extract PDF file into few images",
                                             page_count as u64));

    let antialias_setting = cairo::Antialias::Fast;
    let mut font_options = cairo::FontOptions::new()?;
    font_options.set_antialias(antialias_setting);
    font_options.set_hint_metrics(cairo::HintMetrics::Default);
    font_options.set_hint_style(cairo::HintStyle::Slight);

    let progress_delta = progress_range.delta();

    for i in 0..page_count {
        let idx = i + 1;

        if let Some(page) = doc.page(i as i32) {
            let idx_text = idx.to_string();
            progress_value = progress_range.min + (idx * progress_delta / page_count) as usize;
            logger.log(progress_value, l10n.gettext_fmt("Extracting page {0} into a PNG image", vec![&idx_text]));

            let dest_path = dest_folder.join(format!("page-{}.png", idx));
            let current_size = page.size();

            let (ratio, (new_width, new_height)) = scaling_data(current_size, target_size);

            let surface_png = ImageSurface::create(Format::Rgb24, new_width as i32, new_height as i32)?;
            let ctx = Context::new(&surface_png)?;

            ctx.scale(ratio, ratio);
            ctx.set_source_rgb(1.0, 1.0, 1.0);
            ctx.set_antialias(antialias_setting);
            ctx.set_font_options(&font_options);
            ctx.paint()?;

            page.render(&ctx);
            surface_png.write_to_png(&mut fs::File::create(dest_path)?)?;
        }
    }

    Ok(())
}

fn scaling_data(size_current: (f64, f64), size_target: (f64, f64)) -> (f64, (f64, f64)) {
    let ratio_width = size_target.0 / size_current.0;
    let ratio_height = size_target.1 / size_current.1;

    let mut ratio = if ratio_width > ratio_height {
        ratio_height
    } else {
        ratio_width
    };

    let (mut new_width, mut new_height) = (size_current.0 * ratio, size_current.1 * ratio);

    if (new_width as i32) == 0 || (new_height as i32) == 0 {
        ratio = 1.0;
        new_width = size_current.0;
        new_height = size_current.1;
    }

    (ratio, (new_width, new_height))
}

fn pdf_combine_pdfs(logger: &dyn ConversionLogger, progress_range: &ProgressRange, page_count: usize, input_dir_path: &Path, output_path: &Path, l10n: l10n::Translations) -> Result<(), Box<dyn Error>> {
    logger.log(progress_range.min,
               l10n.ngettext("Combining one PDF document",
                             "Combining few PDF documents",
                             page_count as u64));

    let mut documents: Vec<lopdf::Document> = Vec::with_capacity(page_count);

    let step_count = 7;
    let mut step_num = 1;
    let progress_delta = progress_range.delta();

    // step 1/7
    let mut progress_value = progress_range.min + (step_num * progress_delta / step_count) as usize;
    logger.log(progress_value, l10n.gettext("Collecting PDF pages"));

    for i in 0..page_count {
        let src_path = input_dir_path.join(format!("page-{}.pdf", i + 1));
        let document: lopdf::Document = lopdf::Document::load(src_path)?;
        documents.push(document);
    }

    // Define a starting max_id (will be used as start index for object_ids)
    let mut max_id = 1;
    let mut pagenum = 1;

    // Collect all Documents Objects grouped by a map
    let mut documents_pages = BTreeMap::new();
    let mut documents_objects = BTreeMap::new();
    let mut document = lopdf::Document::with_version("1.5");

    // step 2/7
    step_num += 1;
    progress_value = progress_range.min + (step_num * progress_delta / step_count) as usize;
    logger.log(progress_value, l10n.gettext("Updating bookmarks and page numbering"));

    for mut doc in documents {
        let mut first = false;
        doc.renumber_objects_with(max_id);
        max_id = doc.max_id + 1;

        doc.get_pages()
            .into_iter()
            .map(|(_, object_id)| {
                if !first {
                    let bookmark = lopdf::Bookmark::new(format!("Page_{}", pagenum), [0.0, 0.0, 1.0], 0, object_id);
                    document.add_bookmark(bookmark, None);
                    first = true;
                    pagenum += 1;
                }

                (object_id, doc.get_object(object_id).unwrap().to_owned())
            })
            .for_each(|(key, value)| {
                documents_pages.insert(key, value);
            });

        documents_objects.extend(doc.objects);
    }

    // Catalog and Pages are mandatory
    let mut catalog_object: Option<(lopdf::ObjectId, lopdf::Object)> = None;
    let mut pages_object: Option<(lopdf::ObjectId, lopdf::Object)>   = None;

    // step 3/7 Process all objects except "Page" type
    step_num += 1;
    progress_value = progress_range.min + (step_num * progress_delta / step_count) as usize;
    logger.log(progress_value, l10n.gettext("Processing PDF structure"));

    for (object_id, object) in documents_objects.iter() {
        // We have to ignore "Page" (as are processed later), "Outlines" and "Outline" objects
        // All other objects should be collected and inserted into the main Document
        match object.type_name().unwrap_or("") {
            "Catalog" => {
                // Collect a first "Catalog" object and use it for the future "Pages"
                catalog_object = Some((if let Some((id, _)) = catalog_object { id } else { *object_id }, object.clone()));
            }
            "Pages" => {
                // Collect and update a first "Pages" object and use it for the future "Catalog"
                // We have also to merge all dictionaries of the old and the new "Pages" object
                if let Ok(dictionary) = object.as_dict() {
                    let mut dictionary = dictionary.clone();
                    if let Some((_, ref object)) = pages_object {
                        if let Ok(old_dictionary) = object.as_dict() {
                            dictionary.extend(old_dictionary);
                        }
                    }

                    pages_object = Some((
                        if let Some((id, _)) = pages_object { id } else { *object_id },
                        lopdf::Object::Dictionary(dictionary),
                    ));
                }
            }
            "Page" => {}     // Ignored, processed later and separately
            "Outlines" => {} // Ignored, not supported yet
            "Outline" => {}  // Ignored, not supported yet
            _ => {
                document.objects.insert(*object_id, object.clone());
            }
        }
    }

    // If no "Pages" found abort
    if pages_object.is_none() {
        return Err(l10n.gettext("No page found while combinding PDF pages!").into());
    }

    // step 4/7 Iter over all "Page" and collect with the parent "Pages" created before
    step_num += 1;
    progress_value = progress_range.min + (step_num * progress_delta / step_count) as usize;
    logger.log(progress_value, l10n.gettext("Updating PDF dictionnary"));

    for (object_id, object) in documents_pages.iter() {
        if let Ok(dictionary) = object.as_dict() {
            let mut dictionary = dictionary.clone();

            if let Some(parent_obj) = pages_object.as_ref() {
                dictionary.set("Parent", parent_obj.0);
            }

            document.objects.insert(*object_id, lopdf::Object::Dictionary(dictionary));
        }
    }

    // If no "Catalog" found abort
    if catalog_object.is_none() {
        return Err(l10n.gettext("Root catalog was not found!").into());
    }

    // step 5/7 Merge objects
    step_num += 1;
    progress_value = progress_range.min + (step_num * progress_delta / step_count) as usize;
    logger.log(progress_value, l10n.gettext("Combining PDF objects"));

    if let (Some(catalog_object), Some(pages_object)) = (catalog_object, pages_object) {
        // Build a new "Pages" with updated fields
        if let Ok(dictionary) = pages_object.1.as_dict() {
            let mut dictionary = dictionary.clone();

            // Set new pages count
            dictionary.set("Count", documents_pages.len() as u32);

            // Set new "Kids" list (collected from documents pages) for "Pages"
            dictionary.set(
                "Kids",
                documents_pages
                    .into_iter()
                    .map(|(object_id, _)| lopdf::Object::Reference(object_id))
                    .collect::<Vec<_>>(),
            );

            document.objects.insert(pages_object.0, lopdf::Object::Dictionary(dictionary));
        }

        // Build a new "Catalog" with updated fields
        if let Ok(dictionary) = catalog_object.1.as_dict() {
            let mut dictionary = dictionary.clone();
            dictionary.set("Pages", pages_object.0);
            dictionary.set("PageMode", "UseOutlines");
            dictionary.remove(b"Outlines"); // Outlines not supported in merged PDFs

            document.objects.insert(catalog_object.0, lopdf::Object::Dictionary(dictionary));
        }

        document.trailer.set("Root", catalog_object.0);

        // Update the max internal ID as wasn't updated before due to direct objects insertion
        document.max_id = document.objects.len() as u32;

        // Reorder all new Document objects
        document.renumber_objects();

        //Set any Bookmarks to the First child if they are not set to a page
        document.adjust_zero_pages();

        //Set all bookmarks to the PDF Object tree then set the Outlines to the Bookmark content map.
        if let Some(n) = document.build_outline() {
            if let Ok(lopdf::Object::Dictionary(ref mut dict)) = document.get_object_mut(catalog_object.0) {
                dict.set("Outlines", lopdf::Object::Reference(n));
            }
        }
    }

    // step 6/7 Compress the document
    step_num += 1;
    progress_value = progress_range.min + (step_num * progress_delta / step_count) as usize;
    logger.log(progress_value, l10n.gettext("Compressing PDF"));

    document.prune_objects();
    document.delete_zero_length_streams();
    document.compress();

    // step 7/7 Save the merged PDF
    step_num += 1;
    progress_value = progress_range.min + (step_num * progress_delta / step_count) as usize;
    logger.log(progress_value, l10n.gettext("Saving PDF"));

    if let Err(ex) = document.save(output_path) {
        return Err(l10n.gettext_fmt("Could not save PDF file to {0}. {1}.", vec![&output_path.display().to_string(), &ex.to_string()]).into());
    }

    if std::fs::metadata(output_path).is_err() {
        return Err(l10n.gettext_fmt("Could not save PDF file to {0}.", vec![&output_path.display().to_string()]).into());
    }

    Ok(())
}

fn imgs_to_pdf(logger: &dyn ConversionLogger, progress_range: &ProgressRange, page_count: usize, input_path: &Path, output_path: &Path, l10n: l10n::Translations) -> Result<(), Box<dyn Error>> {
    let progress_delta = progress_range.delta();
    let mut progress_value: usize = progress_range.min;

    logger.log(progress_value, l10n.ngettext("Saving one PNG image to PDF",
                                             "Saving few PNG images to PDF",
                                             page_count as u64));

    for i in 0..page_count {
        let idx = i + 1;
        let idx_text = idx.to_string();
        progress_value = progress_range.min + (idx * progress_delta / page_count) as usize;
        logger.log(progress_value, l10n.gettext_fmt("Saving PNG image {0} to PDF", vec![&idx_text]));
        let src = input_path.join(format!("page-{}.png", &idx));
        let dest = output_path.join(format!("page-{}.pdf", &idx));
        img_to_pdf(image::ImageFormat::Png, &src, &dest)?;
    }

    Ok(())
}

fn img_to_pdf(src_format: image::ImageFormat, src_path: &Path, dest_path: &Path) -> Result<(), Box<dyn Error>> {
    let f = fs::File::open(src_path)?;
    let file_len = f.metadata()?.len() as usize;
    let reader = BufReader::new(f);
    let img = image::load(reader, src_format)?;
    let mut buffer: Vec<u8> = Vec::with_capacity(file_len);
    let buffer_cursor = &mut Cursor::new(&mut buffer);

    img.write_to(buffer_cursor, image::ImageOutputFormat::Png)?;
    buffer_cursor.flush()?;
    buffer_cursor.rewind()?;

    let surface_png = ImageSurface::create_from_png(buffer_cursor)?;
    let (w, h) = (surface_png.width() as f64, surface_png.height() as f64);
    let surface_pdf = PdfSurface::new(w, h, dest_path)?;
    let ctx = Context::new(&surface_pdf)?;

    ctx.set_source_rgb(1.0, 1.0, 1.0);
    ctx.set_source_surface(&surface_png, 0.0, 0.0)?;
    ctx.paint()?;
    ctx.identity_matrix();
    ctx.set_source_surface(&surface_pdf, 0.0, 0.0)?;
    ctx.show_page()?;
    ctx.save()?;

    Ok(())
}
