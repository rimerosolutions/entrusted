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
use std::io::{BufReader, Cursor};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Clone, Debug)]
enum ConversionType {
    None,
    LibreOffice(String),
    Convert,
}

struct TessSettings<'a> {
    lang: &'a str,
    data_dir: &'a str,
}

macro_rules! timed {
    ($e:expr) => {{
        let timer = Instant::now();
        let ret = $e();
        let millis = timer.elapsed().as_millis();
        println!("Elapsed time: {}.", elapsed_time_string(millis));

        ret
    }};
}

const TESS_DATA_DIR: &str = "/usr/share/tessdata";

fn main() -> Result<(), Box<dyn Error>> {
    timed!(|| {
        let skip_ocr = match env::var("OCR") {
            Ok(ocr_set_value) => {
                match ocr_set_value.as_str() {
                    "1" => false,
                    _ => true
                }
            },
            Err(ex) => {
                return Err(ex.into());
            }
        };
        let raw_input_path = PathBuf::from("/tmp/input_file");
        let output_file_path = PathBuf::from("/tmp/safe-output-compressed.pdf");
        let output_dir_path = PathBuf::from("/tmp/");
        let safe_dir_path = PathBuf::from("/safezone/safe-output-compressed.pdf");
        let input_file_path = input_as_pdf_to_pathbuf_uri(raw_input_path)?;
        let page_count = split_pdf_pages_into_images(input_file_path, output_dir_path.clone())?;

        if skip_ocr {
            imgs_to_pdf(page_count, output_dir_path.clone(), output_dir_path.clone())?;
        } else {
            let ocr_lang = env::var("OCR_LANGUAGE")?;            
            let tess_settings = TessSettings {
                lang: ocr_lang.as_str(),
                data_dir: TESS_DATA_DIR,
            };

            ocr_imgs_to_pdf(page_count, tess_settings, output_dir_path.clone(), output_dir_path.clone())?;
        }

        pdf_combine_pdfs(page_count, output_dir_path.clone(), output_file_path.clone())?;
        move_file_to_dir(output_file_path, safe_dir_path)
    })?;

    Ok(())
}

fn move_file_to_dir(src_file_path: PathBuf, dest_dir_path: PathBuf) -> Result<(), Box<dyn Error>> {
    fs::copy(&src_file_path, dest_dir_path)?;
    fs::remove_file(src_file_path)?;

    Ok(())
}

fn input_as_pdf_to_pathbuf_uri(raw_input_path: PathBuf) -> Result<PathBuf, Box<dyn Error>> {
    let conversion_by_mimetype: HashMap<&str, ConversionType> = [
        ("application/pdf", ConversionType::None),
        (
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            ConversionType::LibreOffice("writer_pdf_Export".to_string()),
        ),
        (
            "application/vnd.ms-word.document.macroEnabled.12",
            ConversionType::LibreOffice("writer_pdf_Export".to_string()),
        ),
        (
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            ConversionType::LibreOffice("calc_pdf_Export".to_string()),
        ),
        (
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            ConversionType::LibreOffice("impress_pdf_Export".to_string()),
        ),
        ("application/msword", ConversionType::LibreOffice("writer_pdf_Export".to_string())),
        (
            "application/vnd.ms-excel",
            ConversionType::LibreOffice("calc_pdf_Export".to_string()),
        ),
        (
            "application/vnd.ms-powerpoint",
            ConversionType::LibreOffice("impress_pdf_Export".to_string()),
        ),
        (
            "application/vnd.oasis.opendocument.text",
            ConversionType::LibreOffice("writer_pdf_Export".to_string()),
        ),
        (
            "application/vnd.oasis.opendocument.graphics",
            ConversionType::LibreOffice("impress_pdf_Export".to_string()),
        ),

        (
            "application/vnd.oasis.opendocument.presentation",
            ConversionType::LibreOffice("impress_pdf_Export".to_string()),
        ),
        (
            "application/vnd.oasis.opendocument.spreadsheet",
            ConversionType::LibreOffice("calc_pdf_Export".to_string()),
        ),
        ("image/jpeg", ConversionType::Convert),
        ("image/gif", ConversionType::Convert),
        ("image/png", ConversionType::Convert),
        ("image/tiff", ConversionType::Convert),
        ("image/x-tiff", ConversionType::Convert),
    ]
        .iter()
        .cloned()
        .collect();

    if !raw_input_path.exists() {
        return Err(format!("The file {} doesn't exists!", raw_input_path.display()).into());
    }

    let kind = infer::get_from_path(&raw_input_path)?;
    let mime_type: &str;

    if let Some(kind) = kind {
        mime_type = kind.mime_type();
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
                    println!("+ Copying PDF input to {}.", filename_pdf);
                    fs::copy(raw_input_path, PathBuf::from(filename_pdf.clone()))?;
                }
                ConversionType::Convert => {
                    println!("+ Converting input image to PDF");
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
                ConversionType::LibreOffice(_) => {
                    return Err("+ Converting to PDF using LibreOffice is not supported yet!".into());
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
    page_count: usize,
    tess_settings: TessSettings,
    input_path: PathBuf,
    output_path: PathBuf,
) -> Result<(), Box<dyn Error>> {
    println!("+ Performing OCR to PDF on {} images.", page_count);
    let api = tesseract_init(tess_settings.lang, tess_settings.data_dir);

    for i in 0..page_count {
        let page_num = i + 1;
        println!("++ Performing OCR on page {}.", page_num);
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

fn split_pdf_pages_into_images(src_path: PathBuf, dest_folder: PathBuf) -> Result<usize, Box<dyn Error>> {
    let input_file_param = format!("{}", src_path.display());
    let doc = Document::from_file(&input_file_param, None)?;
    let page_num = doc.n_pages();

    println!("+ Saving PDF to {} PNG images.", page_num);

    let antialias_setting = cairo::Antialias::Fast;
    let mut font_options = cairo::FontOptions::new()?;
    font_options.set_antialias(antialias_setting);
    font_options.set_hint_metrics(cairo::HintMetrics::Default);
    font_options.set_hint_style(cairo::HintStyle::Slight);

    let image_dpi: f64 = 150.0;
    let target_dpi: f64 = 72.0;
    let zoom_ratio = image_dpi / target_dpi;

    for i in 0..page_num {
        let page_num = i + 1;

        if let Some(page) = doc.page(i) {
            println!("++ Saving page {} to PNG.", page_num);

            let dest_path = dest_folder.join(format!("page-{}.png", page_num));
            let (w, h) = page.size();
            let sw = (w * zoom_ratio) as i32;
            let sh = (h * zoom_ratio) as i32;

            let surface_png = ImageSurface::create(Format::Rgb24, sw, sh)?;

            let ctx = Context::new(&surface_png)?;
            ctx.set_source_rgb(1.0, 1.0, 1.0);
            ctx.scale(image_dpi / target_dpi, image_dpi / target_dpi);
            ctx.set_antialias(antialias_setting);
            ctx.set_font_options(&font_options);
            ctx.paint()?;

            page.render(&ctx);
            surface_png.write_to_png(&mut fs::File::create(dest_path)?)?;
        }
    }

    Ok(page_num as usize)
}

fn pdf_combine_pdfs(page_count: usize, input_dir_path: PathBuf, output_path: PathBuf) -> Result<(), Box<dyn Error>> {
    println!("+ Combining {} PDF document(s).", page_count);

    let mut documents: Vec<lopdf::Document> = Vec::with_capacity(page_count);

    for i in 0..page_count {
        let src_path = input_dir_path.join(format!("page-{}.pdf", i + 1));
        let document: lopdf::Document = lopdf::Document::load(src_path)?;
        documents.push(document);
    }

    // Define a starting max_id (will be used as start index for object_ids)
    let mut max_id = 1;
˜    let mut pagenum = 1;
    // Collect all Documents Objects grouped by a map
    let mut documents_pages = BTreeMap::new();
    let mut documents_objects = BTreeMap::new();
    let mut document = lopdf::Document::with_version("1.5");

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

    // Process all objects except "Page" type
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

    // Iter over all "Page" and collect with the parent "Pages" created before
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

    document.compress();

    // Save the merged PDF
    document.save(output_path)?;

    Ok(())
}

fn imgs_to_pdf(page_count: usize, input_path: PathBuf, output_path: PathBuf) -> Result<(), Box<dyn Error>> {
    println!("+ Saving {} PNG images to PDFs.", page_count);

    for i in 0..page_count {
        let idx = i + 1;
        println!("++ Saving PNG image {} to PDF.", idx);
        let src = input_path.join(format!("page-{}.png", idx));
        let dest = output_path.join(format!("page-{}.pdf", idx));
        img_to_pdf(image::ImageFormat::Png, src, dest)?;
    }

    Ok(())
}

fn img_to_pdf(src_format: image::ImageFormat, src_path: PathBuf, dest_path: PathBuf) -> Result<(), Box<dyn Error>> {
    let f = fs::File::open(src_path)?;
    let reader = BufReader::new(f);
    let img = image::load(reader, src_format)?;
    let mut buffer: Vec<u8> = vec![];

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
