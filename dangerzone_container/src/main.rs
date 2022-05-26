use cairo::{Context, Format, ImageSurface, PdfSurface};
use std::env;
use image;
use infer;
use lopdf;
use std::process::{Command, Stdio};
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

const SIG_LEGACY_OFFICE: [u8; 8] = [ 208, 207, 17, 224, 161, 177, 26, 225 ];

const IMAGE_DPI: f64   = 150.0;
const TARGET_DPI : f64 = 72.0;
const ZOOM_RATIO: f64  = IMAGE_DPI / TARGET_DPI;

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

fn mkdirp(p: PathBuf) -> Result<(), Box<dyn Error>> {
    if !p.exists() {
        let dir_created = fs::create_dir(p.clone());

        match dir_created {
            Err(ex) => {
                Err(format!("Cannot create directory: {:?}! {}", p, ex.to_string()).into())
            },
            _ => Ok(())
        }
    } else {
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let timer = Instant::now();

    let skip_ocr = match env::var("OCR") {
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

    let logger: Box<dyn ConversionLogger> = match env::var("LOG_FORMAT") {
        Ok(dgz_logformat_value) => {
            match dgz_logformat_value.as_str() {
                "json" => {
                    Box::new(JsonConversionLogger)
                },
                _ => Box::new(PlainConversionLogger)
            }
        },
        Err(_) => Box::new(PlainConversionLogger)
    };

    let ret = {
        let raw_input_path = PathBuf::from("/tmp/input_file");
        let output_file_path = PathBuf::from("/tmp/safe-output-compressed.pdf");
        let output_dir_path = PathBuf::from("/tmp/");
        let safe_dir_path = PathBuf::from("/safezone/safe-output-compressed.pdf");

        // step 1 0%
        let mut progress_range = ProgressRange::new(0, 20);
        let input_file_path = input_as_pdf_to_pathbuf_uri(&logger, progress_range, raw_input_path)?;

        let input_file_param = format!("{}", input_file_path.display());
        let doc = Document::from_file(&input_file_param, None)?;
        let page_count = doc.n_pages() as usize;

        // step 2 (20%)
        progress_range = ProgressRange::new(15, 40);
        split_pdf_pages_into_images(&logger, progress_range, doc, output_dir_path.clone())?;

        // step 3 (40%)
        progress_range = ProgressRange::new(40, 90);

        if skip_ocr {
            imgs_to_pdf(&logger, progress_range, page_count, output_dir_path.clone(), output_dir_path.clone())?;
        } else {
            let ocr_lang = env::var("OCR_LANGUAGE")?;

            let tess_settings = TessSettings {
                lang: ocr_lang.as_str(),
                data_dir: TESS_DATA_DIR,
            };

            ocr_imgs_to_pdf(&logger, progress_range, page_count, tess_settings, output_dir_path.clone(), output_dir_path.clone())?;
        }

        // step 4 (60%)
        progress_range = ProgressRange::new(90, 98);
        pdf_combine_pdfs(&logger, progress_range, page_count, output_dir_path.clone(), output_file_path.clone())?;

        // step 5 (80%)
        move_file_to_dir(output_file_path, safe_dir_path)
    };

    let exit_code = if ret.is_ok() {
        0
    } else {
        1
    };
    
    let msg: String = if let Err(ex) = ret {
        format!("Conversion failed! {}", ex.to_string())
    } else {
        format!("Conversion succeeded!")
    };

    logger.log(99, msg);

    let millis = timer.elapsed().as_millis();
    logger.log(100, format!("Elapsed time: {}.", elapsed_time_string(millis)));
    
    std::process::exit(exit_code);
}

fn move_file_to_dir(src_file_path: PathBuf, dest_dir_path: PathBuf) -> Result<(), Box<dyn Error>> {
    fs::copy(&src_file_path, dest_dir_path)?;
    fs::remove_file(src_file_path)?;

    Ok(())
}

fn input_as_pdf_to_pathbuf_uri(logger: &Box<dyn ConversionLogger>, _: ProgressRange, raw_input_path: PathBuf) -> Result<PathBuf, Box<dyn Error>> {    
    let conversion_by_mimetype: HashMap<&str, ConversionType> = [
        ("application/pdf", ConversionType::None),
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
        return Err(format!("The file {} doesn't exists!", raw_input_path.display()).into());
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
            if let Ok(f) = fs::File::open(raw_input_path.clone()) {
                let file_len = raw_input_path.clone().metadata()?.len() as usize;
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
                return Err(format!("Cannot find input file at '{}'.", raw_input_path.clone().display()).into());
            }
        }
    } else {
        return Err("Cannot determine input mime-type! Does the input have a known file extension?".into());
    }

    if !conversion_by_mimetype.contains_key(mime_type) {
        return Err(format!("Unsupported mime type: {}.", mime_type).into());
    }

    let filename_pdf: String;

    if let Some(conversion_type) = conversion_by_mimetype.get(mime_type) {
        if let Some(parent_dir) = raw_input_path.parent() {
            if let Some(basename_opt) = raw_input_path.file_stem() {
                if let Some(basename) = basename_opt.to_str() {
                    let input_name = format!("{}_input.pdf", basename);
                    filename_pdf = format!("{}", parent_dir.join(input_name.as_str()).display());
                } else {
                    return Err(format!("Cannot determine input basename for file {}!", raw_input_path.display()).into());
                }
            } else {
                return Err(format!("Cannot determine input basename for file {}!", raw_input_path.display()).into());
            }

            match conversion_type {
                ConversionType::None => {
                    logger.log(5, format!("+ Copying PDF input to {}.", filename_pdf));

                    fs::copy(raw_input_path, PathBuf::from(filename_pdf.clone()))?;
                }
                ConversionType::Convert => {
                    logger.log(5, format!("+ Converting input image to PDF"));

                    let img_format = match mime_type {
                        "image/png"    => Ok(image::ImageFormat::Png),
                        "image/jpeg"   => Ok(image::ImageFormat::Jpeg),
                        "image/gif"    => Ok(image::ImageFormat::Gif),
                        "image/tiff"   => Ok(image::ImageFormat::Tiff),
                        "image/x-tiff" => Ok(image::ImageFormat::Tiff),
                        _ => Err("Unsupported image type"),
                    }?;
                    img_to_pdf(img_format, raw_input_path, PathBuf::from(filename_pdf.clone()))?;
                }
                ConversionType::LibreOffice(output_filter, fileext) => {
                    logger.log(5, format!("Converting to PDF using LibreOffice with filter: {}", output_filter));
                    let new_input_path = PathBuf::from(format!("/tmp/input.{}", fileext));
                    fs::copy(&raw_input_path, &new_input_path)?;
                    let output_dir_libreoffice = "/tmp/libreoffice";
                    mkdirp(PathBuf::from(output_dir_libreoffice))?;

                    if let Some(raw_input_path_dir) = raw_input_path.parent() {
                        let exec_status = Command::new("libreoffice")
                            .current_dir(raw_input_path_dir)
                            .arg("--headless")
                            .arg("--convert-to")
                            .arg(format!("pdf:{}", output_filter))
                            .arg("--outdir")
                            .arg(output_dir_libreoffice)
                            .arg(new_input_path)
                            .stdout(Stdio::inherit())
                            .stderr(Stdio::inherit())
                            .status()?;

                        if !exec_status.success() {
                            return Err("Failed to execute 'libreoffice' process!".into());
                        }
                    }

                    let mut pdf_file_moved = false;

                    for f in fs::read_dir(output_dir_libreoffice)? {
                        let f_path = f?.path();
                        let f_path_name = format!("{}", f_path.display());

                        if f_path_name.ends_with(".pdf") {
                            move_file_to_dir(f_path, PathBuf::from(filename_pdf.clone()))?;
                            pdf_file_moved = true;
                            break;
                        }
                    }

                    if !pdf_file_moved {
                        return Err("Could not find office document PDF result!".into());
                    }
                }
            }
        } else {
            return Err("Cannot find input parent directory!".into());
        }
    } else {
        return Err(format!("Unsupported mime type: {}.", mime_type).into());
    }

    Ok(PathBuf::from(format!("file://{}", filename_pdf)))
}

#[inline]
fn elapsed_time_string(millis: u128) -> String {
    let mut diff = millis;
    let secs_in_millis = 1000;
    let mins_in_millis = secs_in_millis * 60;
    let hrs_in_millis = mins_in_millis * 60;
    let hours = diff / hrs_in_millis;
    diff = diff % hrs_in_millis;
    let minutes = diff / mins_in_millis;
    diff = diff % mins_in_millis;
    let seconds = diff / secs_in_millis;

    format!("{} hour(s) {} minute(s) {} seconds(s)", hours, minutes, seconds)
}

fn ocr_imgs_to_pdf(
    logger: &Box<dyn ConversionLogger>,
    progress_range: ProgressRange,
    page_count: usize,
    tess_settings: TessSettings,
    input_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), Box<dyn Error>> {
    let progress_delta = progress_range.delta();
    let mut progress_value: usize = progress_range.min;
    logger.log(progress_value, format!("+ Performing OCR to PDF on {} images.", page_count));

    let api = tesseract_init(tess_settings.lang, tess_settings.data_dir);

    for i in 0..page_count {
        let page_num = i + 1;
        progress_value = progress_range.min + (page_num * progress_delta / page_count);
        logger.log(progress_value, format!("++ Performing OCR on page {}.", page_num));
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

fn split_pdf_pages_into_images(logger: &Box<dyn ConversionLogger>, progress_range: ProgressRange, doc: Document, dest_folder: PathBuf) -> Result<(), Box<dyn Error>> {
    let page_num = doc.n_pages();
    let mut progress_value: usize = progress_range.min;

    logger.log(progress_value, format!("+ Saving PDF to {} PNG images.", page_num));

    let antialias_setting = cairo::Antialias::Fast;
    let mut font_options = cairo::FontOptions::new()?;
    font_options.set_antialias(antialias_setting);
    font_options.set_hint_metrics(cairo::HintMetrics::Default);
    font_options.set_hint_style(cairo::HintStyle::Slight);

    let progress_delta = progress_range.delta();

    for i in 0..page_num {
        let page_num = i + 1;

        if let Some(page) = doc.page(i) {
            progress_value = progress_range.min + (i * progress_delta as i32 / page_num) as usize;
            logger.log(progress_value, format!("++ Saving page {} to PNG.", page_num));

            let dest_path = dest_folder.join(format!("page-{}.png", page_num));
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

fn pdf_combine_pdfs(logger: &Box<dyn ConversionLogger>, progress_range: ProgressRange, page_count: usize, input_dir_path: PathBuf, output_path: PathBuf) -> Result<(), Box<dyn Error>> {
    logger.log(progress_range.min, format!("+ Combining {} PDF document(s).", page_count));

    let mut documents: Vec<lopdf::Document> = Vec::with_capacity(page_count);
    let step_count = 7;
    let mut step_num = 1;

    let progress_delta = progress_range.delta();

    // step 1/7
    let mut progress_value = progress_range.min + (step_num * progress_delta / step_count) as usize;
    logger.log(progress_value, format!("++ Collecting documents to merge."));
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
    logger.log(progress_value, format!("++ Updating documents bookmarks and numbering."));

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
    let mut pages_object: Option<(lopdf::ObjectId, lopdf::Object)> = None;


    // step 3/7 Process all objects except "Page" type
    step_num += 1;
    progress_value = progress_range.min + (step_num * progress_delta / step_count) as usize;
    logger.log(progress_value, format!("++ Process objects."));

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
        return Err("No pages found!".into());
    }

    // step 4/7 Iter over all "Page" and collect with the parent "Pages" created before
    step_num += 1;
    progress_value = progress_range.min + (step_num * progress_delta / step_count) as usize;
    logger.log(progress_value, format!("++ Update dictionary."));

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
        return Err("Catalog root not found!".into());
    }

    // step 5/7 Merge objects
    step_num += 1;
    progress_value = progress_range.min + (step_num * progress_delta / step_count) as usize;
    logger.log(progress_value, format!("++ Merging objects"));

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
    logger.log(progress_value, format!("++ Compressing PDF."));
    document.compress();

    // step 7/7 Save the merged PDF
    step_num += 1;
    progress_value = progress_range.min + (step_num * progress_delta / step_count) as usize;
    logger.log(progress_value, format!("++ Saving PDF."));
    document.save(output_path)?;

    Ok(())
}

fn imgs_to_pdf(logger: &Box<dyn ConversionLogger>, progress_range: ProgressRange, page_count: usize, input_path: PathBuf, output_path: PathBuf) -> Result<(), Box<dyn Error>> {
    let progress_delta = progress_range.delta();
    let mut progress_value: usize = progress_range.min;

    logger.log(progress_value, format!("+ Saving {} PNG images to PDFs.", page_count));

    for i in 0..page_count {
        let idx = i + 1;
        progress_value = progress_range.min + (i * progress_delta / page_count) as usize;
        logger.log(progress_value, format!("++ Saving PNG image {} to PDF.", idx));
        let src = input_path.join(format!("page-{}.png", idx));
        let dest = output_path.join(format!("page-{}.pdf", idx));
        img_to_pdf(image::ImageFormat::Png, src, dest)?;
    }

    Ok(())
}

fn img_to_pdf(src_format: image::ImageFormat, src_path: PathBuf, dest_path: PathBuf) -> Result<(), Box<dyn Error>> {
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