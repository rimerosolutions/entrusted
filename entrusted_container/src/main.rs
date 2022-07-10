use cairo::{Context, Format, ImageSurface, PdfSurface};
use std::env;
use image;
use infer;
use lopdf;
use poppler::Document;
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::ffi::CString;
use std::fs;
use cfb;
use serde::{Deserialize, Serialize};
use std::io::{BufReader, Cursor};
use std::io::prelude::*;
use std::path::PathBuf;
use std::time::Instant;
use zip;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use libreoffice_rs::{Office, LibreOfficeKitOptionalFeatures, urls};
use entrusted_l10n as l10n;

const SIG_LEGACY_OFFICE: [u8; 8] = [ 208, 207, 17, 224, 161, 177, 26, 225 ];
const LOCATION_LIBREOFFICE_PROGRAM: &str = "/usr/lib/libreoffice/program";

const IMAGE_DPI: f64   = 150.0;
const TARGET_DPI : f64 = 72.0;
const ZOOM_RATIO: f64  = IMAGE_DPI / TARGET_DPI;

const ENV_VAR_ENTRUSTED_DOC_PASSWD: &str = "ENTRUSTED_DOC_PASSWD";
const ENV_VAR_LOG_FORMAT: &str           = "LOG_FORMAT";
const ENV_VAR_OCR: &str                  = "OCR";
const ENV_VAR_OCR_LANGUAGE: &str         = "OCR_LANGUAGE";

macro_rules! incl_gettext_files {
    ( $( $x:expr ),* ) => {
        {
            let mut ret = HashMap::new();
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
    LibreOffice(String, String), // libreoffice_filter, file_extension
    Convert,
}

struct TessSettings<'a> {
    lang: &'a str,
    data_dir: &'a str,
}

const TESS_DATA_DIR: &str = "/usr/share/tessdata";

fn main() -> Result<(), Box<dyn Error>> {
    let timer = Instant::now();

    l10n::load_translations(incl_gettext_files!("en", "fr"));

    let locale = match env::var(l10n::ENV_VAR_ENTRUSTED_LANGID) {
        Ok(selected_locale) => selected_locale,
        Err(_) => l10n::sys_locale()
    };
    let l10n = l10n::new_translations(locale);

    let skip_ocr = match env::var(ENV_VAR_OCR) {
        Ok(ocr_set_value) => {
            match ocr_set_value.as_str() {
                "1" => false,
                _ => true
            }
        },
        Err(_) => {
            true
        }
    };

    let document_password = if let Ok(passwd) = env::var(ENV_VAR_ENTRUSTED_DOC_PASSWD) {
        if !passwd.is_empty() {
            Some(passwd)
        } else {
            None
        }
    } else {
        None
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
        let raw_input_path = PathBuf::from("/tmp/input_file");
        let output_file_path = PathBuf::from("/tmp/safe-output-compressed.pdf");
        let output_dir_path = PathBuf::from("/tmp/");
        let safe_dir_path = PathBuf::from("/safezone/safe-output-compressed.pdf");

        // step 1 0%
        let mut progress_range = ProgressRange::new(0, 20);
        let input_file_path = input_as_pdf_to_pathbuf_uri(&logger, progress_range, raw_input_path, document_password.clone(), l10n.clone())?;

        let input_file_param = format!("{}", input_file_path.display());
        let doc = if let Some(passwd) = document_password {
            // We only care about originally encrypted PDF files
            // If the document was in another format, then it's already decrypted at this stage
            // Providing a password for a non-encrypted document doesn't fail which removes the need for additional logic and state handling
            Document::from_file(&input_file_param, Some(&passwd))?
        } else {
            Document::from_file(&input_file_param, None)?
        };

        let page_count = doc.n_pages() as usize;

        // step 2 (20%)
        progress_range = ProgressRange::new(20, 45);
        split_pdf_pages_into_images(&logger, progress_range, page_count, doc, &output_dir_path, l10n.clone())?;

        // step 3 (40%)
        progress_range = ProgressRange::new(45, 90);

        if skip_ocr {
            imgs_to_pdf(&logger, progress_range, page_count, &output_dir_path, &output_dir_path, l10n.clone())?;
        } else {
            let ocr_lang = env::var(ENV_VAR_OCR_LANGUAGE)?;
            let ocr_lang_text = ocr_lang.as_str();

            if !l10n::ocr_lang_key_by_name(&l10n).contains_key(&ocr_lang_text) {
                return Err(l10n.gettext_fmt("Unknown language code for the ocr-lang parameter: {0}. Hint: Try 'eng' for English.", vec![ocr_lang_text]).into());
            }

            let tess_settings = TessSettings {
                lang: ocr_lang_text,
                data_dir: TESS_DATA_DIR,
            };

            ocr_imgs_to_pdf(&logger, progress_range, page_count, tess_settings, &output_dir_path, &output_dir_path, l10n.clone())?;
        }

        // step 4 (60%)
        progress_range = ProgressRange::new(90, 98);
        pdf_combine_pdfs(&logger, progress_range, page_count, &output_dir_path, &output_file_path, l10n.clone())?;

        // step 5 (80%)
        move_file_to_dir(&output_file_path, &safe_dir_path)
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

fn move_file_to_dir(src_file_path: &PathBuf, dest_dir_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    fs::copy(&src_file_path, &dest_dir_path)?;
    fs::remove_file(&src_file_path)?;

    Ok(())
}

fn input_as_pdf_to_pathbuf_uri(logger: &Box<dyn ConversionLogger>, _: ProgressRange, raw_input_path: PathBuf, opt_passwd: Option<String>, l10n: l10n::Translations) -> Result<PathBuf, Box<dyn Error>> {
    let conversion_by_mimetype: HashMap<&str, ConversionType> = [
        ("application/pdf", ConversionType::None),
        (
            "application/rtf",
            ConversionType::LibreOffice("writer_pdf_Export".to_string(), "rtf".to_string()),
        ),
        (
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            ConversionType::LibreOffice("writer_pdf_Export".to_string(), "docx".to_string()),
        ),
        (
            "application/vnd.ms-word.document.macroEnabled.12",
            ConversionType::LibreOffice("writer_pdf_Export".to_string(), "docm".to_string()),
        ),
        (
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            ConversionType::LibreOffice("calc_pdf_Export".to_string(),"xlsx".to_string()),
        ),
        (
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            ConversionType::LibreOffice("impress_pdf_Export".to_string(), "pptx".to_string()),
        ),
        ("application/msword", ConversionType::LibreOffice("writer_pdf_Export".to_string(), "doc".to_string())),
        (
            "application/vnd.ms-excel",
            ConversionType::LibreOffice("calc_pdf_Export".to_string(), "xls".to_string()),
        ),
        (
            "application/vnd.ms-powerpoint",
            ConversionType::LibreOffice("impress_pdf_Export".to_string(), "ppt".to_string()),
        ),
        (
            "application/vnd.oasis.opendocument.text",
            ConversionType::LibreOffice("writer_pdf_Export".to_string(), "odt".to_string()),
        ),
        (
            "application/vnd.oasis.opendocument.graphics",
            ConversionType::LibreOffice("impress_pdf_Export".to_string(), "odg".to_string()),
        ),

        (
            "application/vnd.oasis.opendocument.presentation",
            ConversionType::LibreOffice("impress_pdf_Export".to_string(), "odp".to_string()),
        ),
        (
            "application/vnd.oasis.opendocument.spreadsheet",
            ConversionType::LibreOffice("calc_pdf_Export".to_string(), "ods".to_string()),
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

    let kind = infer::get_from_path(&raw_input_path)?;
    let mut mime_type: &str;

    fn of_interest_openxml(name: &str) -> bool {
        name == "_rels/.rels" || name == "[Content_Types].xml"
    }

    fn of_interest_opendocument(name: &str) -> bool {
        name == "mimetype" || name == "content.xml"
    }

    fn office_file_of_interest(name: &str) -> bool {
        of_interest_opendocument(name) || of_interest_openxml(name)
    }

    fn probe_mimetype_zip <'a>(reader: &mut BufReader<fs::File>) -> Result<&'a str, Box<dyn Error>> {
        let mut zip = zip::ZipArchive::new(reader)?;
        let probe_count_expected = 2;
        let mut probe_count_odt = 0;
        let mut probe_count_ooxml = 0;
        let mut ret_odt  = "";
        let mut ret_ooxml  = "";

        // Lots of ownership annoyances with the 'zip' crate dependency
        // Otherwise we would look directly for specific files of interest
        for i in 0..zip.len() {
            if let Ok(zipfile) = zip.by_index(i) {
                let zipfile_name: &str = zipfile.name();

                if office_file_of_interest(zipfile_name) {
                    if of_interest_opendocument(zipfile_name) {
                        if zipfile.name() == "mimetype" {
                            let mut zip_reader = BufReader::new(zipfile);
                            let mut tmp_buf = String::new();
                            zip_reader.read_to_string(&mut tmp_buf)?;

                            if tmp_buf.find("application/vnd.oasis.opendocument.text").is_some() {
                                ret_odt = "application/vnd.oasis.opendocument.text";
                            } else if tmp_buf.find("application/vnd.oasis.opendocument.spreadsheet").is_some() {
                                ret_odt = "application/vnd.oasis.opendocument.spreadsheet";
                            } else if tmp_buf.find("application/vnd.oasis.opendocument.presentation").is_some() {
                                ret_odt = "application/vnd.oasis.opendocument.presentation";
                            } else if tmp_buf.find("application/vnd.oasis.opendocument.graphics").is_some() {
                                ret_odt = "application/vnd.oasis.opendocument.graphics";
                            } 
                        }

                        probe_count_odt += 1;
                    } else if of_interest_openxml(zipfile_name) {
                        if zipfile_name == "_rels/.rels" {
                            let mut zip_reader = BufReader::new(zipfile);
                            let mut tmp_buf = String::new();
                            zip_reader.read_to_string(&mut tmp_buf)?;

                            if tmp_buf.find("ppt/presentation.xml").is_some() {
                                ret_ooxml = "application/vnd.openxmlformats-officedocument.presentationml.presentation";
                            } else if tmp_buf.find("word/document.xml").is_some() {
                                ret_ooxml = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
                            } else if tmp_buf.find("xl/workbook.xml").is_some() {
                                ret_ooxml = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
                            }
                        }

                        probe_count_ooxml += 1;
                    }
                }

                if probe_count_odt == probe_count_expected {
                    return Ok(ret_odt);
                } else if probe_count_ooxml == probe_count_expected {
                    return Ok(ret_ooxml);
                }
            }
        }

        Ok("application/zip")
    }

    fn probe_mimetype_ole <'a>(buffer: &Vec<u8>) -> &'a str {
        if bytecomp(buffer.to_vec(), SIG_LEGACY_OFFICE.iter().collect()) {
            if let Ok(file) = cfb::CompoundFile::open(Cursor::new(buffer)) {
                return match file.root_entry().clsid().to_string().as_str() {
                    "00020810-0000-0000-c000-000000000046" | "00020820-0000-0000-c000-000000000046" => {
                        "application/vnd.ms-excel"
                    },
                    "00020906-0000-0000-c000-000000000046" => "application/msword",
                    "64818d10-4f9b-11cf-86ea-00aa00b929e8" => "application/vnd.ms-powerpoint",
                    _ => "application/x-ole-storage",
                };
            }
        }

        "application/x-ole-storage"
    }

    fn bytecomp(input: Vec<u8>, sig: Vec<&u8>) -> bool {
        let input_len = input.len();

        for i in 0..sig.len() {
            if i > input_len || input[i] != *sig[i] {
                return false;
            }
        }

        true
    }

    if let Some(kind) = kind {
        mime_type = kind.mime_type();

        if mime_type == "application/zip" || mime_type == "application/x-ole-storage" {
            if let Ok(f) = fs::File::open(&raw_input_path) {
                let file_len = raw_input_path.metadata()?.len() as usize;
                let mut reader = BufReader::new(f);
                let mut buffer: Vec<u8> = Vec::with_capacity(file_len);
                reader.read_to_end(&mut buffer)?;

                if mime_type == "application/zip" {
                    if let Ok(new_mime_type) = probe_mimetype_zip(&mut reader) {
                        mime_type = new_mime_type;
                    }
                } else if mime_type == "application/x-ole-storage" {
                    mime_type = probe_mimetype_ole(&buffer);
                }
            } else {
                return Err(l10n.gettext_fmt("Cannot find file at {0}", vec![&raw_input_path.display().to_string()]).into());
            }
        }
    } else {
        return Err(l10n.gettext("Mime type error! Does the input have a 'known' file extension?").into());
    }

    if !conversion_by_mimetype.contains_key(mime_type) {
        return Err(l10n.gettext_fmt("Unsupported mime type: {0}", vec![mime_type]).into());
    }

    let filename_pdf: String;

    if let Some(conversion_type) = conversion_by_mimetype.get(mime_type) {
        if let Some(parent_dir) = raw_input_path.parent() {
            if let Some(basename_opt) = raw_input_path.file_stem() {
                if let Some(basename) = basename_opt.to_str() {
                    let input_name = format!("{}_input.pdf", basename);
                    filename_pdf = format!("{}", parent_dir.join(input_name.as_str()).display());
                } else {
                    return Err(l10n.gettext_fmt("Could not determine basename for file {0}", vec![&raw_input_path.display().to_string()]).into());
                }
            } else {
                return Err(l10n.gettext_fmt("Could not determine basename for file {0}", vec![&raw_input_path.display().to_string()]).into());
            }

            match conversion_type {
                ConversionType::None => {
                    logger.log(5, l10n.gettext_fmt("Copying PDF input to {0}", vec![&filename_pdf]));
                    fs::copy(&raw_input_path, PathBuf::from(&filename_pdf))?;
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
                    img_to_pdf(img_format, &raw_input_path, &PathBuf::from(&filename_pdf))?;
                }
                ConversionType::LibreOffice(output_filter, fileext) => {
                    logger.log(5, l10n.gettext_fmt("Converting to PDF using LibreOffice with filter: {0}", vec![&output_filter]));
                    let new_input_path = PathBuf::from(format!("/tmp/input.{}", fileext));
                    fs::copy(&raw_input_path, &new_input_path)?;

                    let mut office = Office::new(LOCATION_LIBREOFFICE_PROGRAM)?;
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
                                } else {
                                    if !failed_password_input.load(Ordering::Acquire) {
                                        failed_password_input.store(true, Ordering::Release);
                                        let _ = office.unset_document_password(input_uri.clone());
                                    }
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
        } else {
            return Err(l10n.gettext("Cannot find input parent directory!").into());
        }
    } else {
        return Err(l10n.gettext_fmt("Unsupported mime type: {0}", vec![mime_type]).into());
    }

    Ok(PathBuf::from(format!("file://{}", filename_pdf)))
}

#[inline]
fn elapsed_time_string(millis: u128, l10n: l10n::Translations) -> String {
    let mut diff = millis;
    let secs_in_millis = 1000;
    let mins_in_millis = secs_in_millis * 60;
    let hrs_in_millis = mins_in_millis * 60;
    let hours = diff / hrs_in_millis;
    diff = diff % hrs_in_millis;
    let minutes = diff / mins_in_millis;
    diff = diff % mins_in_millis;
    let seconds = diff / secs_in_millis;

    format!("{} {} {}",
            l10n.ngettext("hour",   "hours",   hours   as u64),
            l10n.ngettext("minute", "minutes", minutes as u64),
            l10n.ngettext("second", "seconds", seconds as u64))
}

fn ocr_imgs_to_pdf(
    logger: &Box<dyn ConversionLogger>,
    progress_range: ProgressRange,
    page_count: usize,
    tess_settings: TessSettings,
    input_path: &PathBuf,
    output_path: &PathBuf,
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
        ocr_img_to_pdf(api, src, dest)?;
    }

    tesseract_delete(api);

    Ok(())
}

fn tesseract_init(ocr_lang: &str, tessdata_dir: &str) -> *mut tesseract_plumbing::tesseract_sys::TessBaseAPI {
    let c_lang = CString::new(ocr_lang).unwrap();
    let lang = c_lang.as_bytes().as_ptr() as *mut u8 as *mut i8;

    let c_datapath = CString::new(tessdata_dir).unwrap();
    let datapath = c_datapath.as_bytes().as_ptr() as *mut u8 as *mut i8;

    let c_user_defined_dpi_var_name = CString::new("user_defined_dpi").unwrap();
    let user_defined_dpi_var_name = c_user_defined_dpi_var_name.as_bytes().as_ptr() as *mut u8 as *mut i8;

    let c_user_defined_dpi_var_value = CString::new("72").unwrap();
    let user_defined_dpi_var_value = c_user_defined_dpi_var_value.as_bytes().as_ptr() as *mut u8 as *mut i8;

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
    let c_inputname = CString::new(input_path.clone().into_os_string().into_string().unwrap().as_str())?;
    let inputname = c_inputname.as_bytes().as_ptr() as *mut u8 as *mut i8;

    let c_outputbase = CString::new(output_path.into_os_string().into_string().unwrap())?;
    let outputbase = c_outputbase.as_bytes().as_ptr() as *mut u8 as *mut i8;

    let c_input_name = CString::new(input_path.clone().file_name().unwrap().to_str().unwrap()).unwrap();
    let input_name = c_input_name.as_bytes().as_ptr() as *mut u8 as *mut i8;

    let do_not_care = CString::new("").unwrap().as_bytes().as_ptr() as *mut u8 as *mut i8;

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
        let progress_json = serde_json::to_string(&progress_msg).unwrap();
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

    fn delta(&self) -> usize {
        self.max - self.min
    }
}

fn split_pdf_pages_into_images(logger: &Box<dyn ConversionLogger>, progress_range: ProgressRange, page_count: usize, doc: Document, dest_folder: &PathBuf, l10n: l10n::Translations) -> Result<(), Box<dyn Error>> {
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
            let (w, h) = page.size();
            let sw = (w * ZOOM_RATIO) as i32;
            let sh = (h * ZOOM_RATIO) as i32;

            let surface_png = ImageSurface::create(Format::Rgb24, sw, sh)?;
            let ctx = Context::new(&surface_png)?;

            ctx.set_source_rgb(1.0, 1.0, 1.0);
            ctx.scale(ZOOM_RATIO, ZOOM_RATIO);
            ctx.set_antialias(antialias_setting);
            ctx.set_font_options(&font_options);
            ctx.paint()?;

            page.render(&ctx);
            surface_png.write_to_png(&mut fs::File::create(dest_path)?)?;
        }
    }

    Ok(())
}

fn pdf_combine_pdfs(logger: &Box<dyn ConversionLogger>, progress_range: ProgressRange, page_count: usize, input_dir_path: &PathBuf, output_path: &PathBuf, l10n: l10n::Translations) -> Result<(), Box<dyn Error>> {
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

        documents_pages.extend(
            doc.get_pages()
                .into_iter()
                .map(|(_, object_id)| {
                    if !first {
                        let bookmark = lopdf::Bookmark::new(String::from(format!("Page_{}", pagenum)), [0.0, 0.0, 1.0], 0, object_id);
                        document.add_bookmark(bookmark, None);
                        first = true;
                        pagenum += 1;
                    }

                    (object_id, doc.get_object(object_id).unwrap().to_owned())
                })
                .collect::<BTreeMap<lopdf::ObjectId, lopdf::Object>>(),
        );
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
            if let Ok(x) = document.get_object_mut(catalog_object.0) {
                if let lopdf::Object::Dictionary(ref mut dict) = x {
                    dict.set("Outlines", lopdf::Object::Reference(n));
                }
            }
        }
    }

    // step 6/7 Compress the document
    step_num += 1;
    progress_value = progress_range.min + (step_num * progress_delta / step_count) as usize;
    logger.log(progress_value, l10n.gettext("Compressing PDF"));
    document.compress();

    // step 7/7 Save the merged PDF
    step_num += 1;
    progress_value = progress_range.min + (step_num * progress_delta / step_count) as usize;
    logger.log(progress_value, l10n.gettext("Saving PDF"));
    document.save(output_path)?;

    Ok(())
}

fn imgs_to_pdf(logger: &Box<dyn ConversionLogger>, progress_range: ProgressRange, page_count: usize, input_path: &PathBuf, output_path: &PathBuf, l10n: l10n::Translations) -> Result<(), Box<dyn Error>> {
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
        let src = input_path.join(format!("page-{}.png", idx));
        let dest = output_path.join(format!("page-{}.pdf", idx));
        img_to_pdf(image::ImageFormat::Png, &src, &dest)?;
    }

    Ok(())
}

fn img_to_pdf(src_format: image::ImageFormat, src_path: &PathBuf, dest_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let file_len = src_path.metadata()?.len() as usize;
    let f = fs::File::open(src_path)?;
    let reader = BufReader::new(f);
    let img = image::load(reader, src_format)?;
    let mut buffer: Vec<u8> = Vec::with_capacity(file_len);

    img.write_to(&mut buffer, image::ImageOutputFormat::Png)?;

    let mut c = Cursor::new(buffer);
    let surface_png = ImageSurface::create_from_png(&mut c)?;
    let (w, h) = (surface_png.width(), surface_png.height());
    let surface_pdf = PdfSurface::new(w as f64, h as f64, dest_path)?;
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
