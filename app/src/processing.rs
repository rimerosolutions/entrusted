use std::{fs, io};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use file_format::FileFormat;
use mupdf::{Colorspace, Context, Document, ImageFormat, Matrix};
use mupdf::pdf:: {PdfDocument, PdfWriteOptions};
use mupdf::pdf::document::{Permission, Encryption};
use mupdf::document_writer::DocumentWriter;
use uuid::Uuid;

use crate::{error, l10n};
use crate::common::{EventSender, AppEvent, VisualQuality};

pub struct ExecCtx {
    doc_uuid: Uuid,
    root_tmp_dir: PathBuf,
    input_path: PathBuf,
    output_path: PathBuf,
    visual_quality: VisualQuality,
    ocr_lang_code: Option<String>,
    password_decrypt: Option<String>,
    password_encrypt: Option<String>,
    trans: l10n::Translations,
    tx: Box<dyn EventSender>
}

impl ExecCtx {
    pub fn new(doc_uuid: Uuid,
               root_tmp_dir: PathBuf,
               input_path: PathBuf,
               output_path: PathBuf,
               visual_quality: VisualQuality,
               ocr_lang_code: Option<String>,
               password_decrypt: Option<String>,
               password_encrypt: Option<String>,
               trans: l10n::Translations,
               tx: Box<dyn EventSender>) -> Self {
        Self {
            doc_uuid,
            root_tmp_dir,
            input_path,
            output_path,
            visual_quality,
            ocr_lang_code,
            password_decrypt,
            password_encrypt,
            trans,
            tx
        }
    }
}

fn progressed(doc_id: Uuid, tx: &Box<dyn EventSender>, value: usize, message: String) {
    let _ = tx.send(AppEvent::ConversionProgressed(doc_id, value, message));
}

pub fn execute(ctx: ExecCtx, office_opt: &Option<String>, tessdata_opt: &Option<String>, stop_requested: Arc<AtomicBool>) -> Result<Option<PathBuf>, error::Failure> {
    let timer = Instant::now();

    let document_password = ctx.password_decrypt;
    let target_image_size = ctx.visual_quality.image_max_size();
    let doc_uuid          = ctx.doc_uuid;
    let trans              = ctx.trans;
    let tx                = ctx.tx;
    let root_tmp_dir      = ctx.root_tmp_dir;
    let raw_input_path    = ctx.input_path;
    let safe_dir_path     = ctx.output_path;

    let output_dir_path  = root_tmp_dir.clone();
    let output_file_path = root_tmp_dir.join(format!("{}.pdf", doc_uuid));


    let mut context = Context::get();

    // step 1 (0%-20%)
    let mut progress_range = ProgressRange::new(0, 20);
    let input_file_path = input_as_pdf(office_opt, doc_uuid, &root_tmp_dir, target_image_size, &tx, &progress_range, &raw_input_path, document_password.clone(), trans.clone())?;

    let doc = PdfDocument::open(&input_file_path)?;
    let page_count = doc.page_count()? as usize;

    // step 2 (20%-90%)
    progress_range.update(20, 90);
    let mut progress_value: usize = progress_range.min;
    progressed(doc_uuid, &tx, progress_value, trans.ngettext("Extract PDF file into one image",
                                                            "Extract PDF file into few images",
                                                            page_count as u64));

    let (template_img_to_pdf, ocr_lang_opt) = if let Some(v) = ctx.ocr_lang_code {
        if let Some(ref tessdata_dir) = tessdata_opt {
            if fs::metadata(tessdata_dir).is_err() {
                return Err(error::Failure::FeatureMissing("OCR support is not available. No trained data folder was found!".to_string()));
            }
        } else {
            return Err(error::Failure::FeatureMissing("OCR support is not available. No trained data folder was found!".to_string()));
        }

        let ocr_lang_text = v.as_str();
        let selected_langcodes: Vec<&str> = ocr_lang_text.split('+').collect();

        for selected_langcode in selected_langcodes.iter() {
            if !l10n::ocr_lang_key_by_name(&trans).contains_key(selected_langcode) {
                return Err(error::Failure::InvalidInput(trans.gettext_fmt("Unknown language code for the ocr-lang parameter: {0}. Hint: Try 'eng' for English.", vec![selected_langcode])));
            }
        }

        ("Performing OCR on page {0}", Some(ocr_lang_text.to_string()))
    } else {
        ("Saving PNG image {0} to PDF", None)
    };

    let progress_delta = progress_range.delta();

    for i in 0..page_count {
        if stop_requested.load(Ordering::SeqCst) {
            return Ok(None);
        }

        let page_num = i + 1;
        progress_value = progress_range.min + (page_num * progress_delta / page_count);
        let page_num_text = page_num.to_string();

        progressed(doc_uuid, &tx, progress_value, trans.gettext_fmt("Extracting page {0} into a PNG image", vec![&page_num_text]));
        let doc_img = page_to_pixmap(i, &doc, target_image_size)?;

        progressed(doc_uuid, &tx, progress_value, trans.gettext_fmt(template_img_to_pdf, vec![&page_num_text]));
        doc_to_pdf(i, &ocr_lang_opt, tessdata_opt, &doc_img, &output_dir_path)?;
    }

    // step 4 (90%-98%)
    if stop_requested.load(Ordering::SeqCst) {
        return Ok(None);
    }

    progress_range.update(90, 98);
    merge_pdfs(doc_uuid, &tx, &progress_range, page_count, &output_dir_path, &output_file_path, &ctx.password_encrypt,  &trans)?;

    // step 5 (98%-98%)
    progress_range.update(98, 98);
    move_file_to_dir(doc_uuid, &tx, &progress_range, &output_file_path, &safe_dir_path, &trans)?;

    context.shrink_store(100);

    let millis = timer.elapsed().as_millis();
    progressed(doc_uuid, &tx, 100, format!("{}: {}", trans.gettext("Elapsed time"), elapsed_time_string(millis, &trans)));

    Ok(Some(safe_dir_path.clone()))
}

fn doc_to_pdf(
    i: usize,
    ocr_lang_opt: &Option<String>,
    tessdata_opt: &Option<String>,
    doc: &Document,
    output_path: &Path,
) -> Result<(), error::Failure> {
    let dest = output_path.join(format!("page-{}.pdf", i)).display().to_string();
    let dest = dest.as_str();

    let mut writer = {
        if let Some(lang_code) = ocr_lang_opt {
            if let Some(ref data_dir) = tessdata_opt {
                let ret_options = format!("ocr-language={},compression=flate,ocr-datadir={}", lang_code, data_dir);
                DocumentWriter::with_ocr(dest, &ret_options)?
            } else {
                let err_message = "OCR support is not available! No trained data folder was provided.".to_string();
                return Err(error::Failure::FeatureMissing(err_message));
            }
        } else {
            DocumentWriter::new(dest, "pdf", "compress")?
        }
    };

    let page = doc.load_page(0)?;
    let mediabox = page.bounds()?;
    let device = writer.begin_page(mediabox)?;
    page.run(&device, &Matrix::IDENTITY)?;
    writer.end_page(device)?;

    Ok(())
}

fn move_file_to_dir<P: AsRef<Path>>(doc_id: Uuid, tx: &Box<dyn EventSender>, progress_range: &ProgressRange, src_file_path: P, dest_dir_path: P, l10n: &l10n::Translations) -> Result<(), error::Failure> {
    if let Err(ex) = fs::copy(&src_file_path, &dest_dir_path) {
        progressed(doc_id, tx, progress_range.min, l10n.gettext_fmt("Failed to copy file from {0} to {1}", vec![&src_file_path.as_ref().display().to_string(), &dest_dir_path.as_ref().display().to_string()]));
        return Err(ex.into());
    }

    if let Err(ex) = fs::remove_file(&src_file_path) {
        progressed(doc_id, tx, progress_range.min, l10n.gettext_fmt("Failed to remove file from {0}.", vec![&src_file_path.as_ref().display().to_string()]));
        return Err(ex.into());
    }

    progressed(doc_id, tx, progress_range.min, l10n.gettext("Moving output files to their final destination"));

    Ok(())
}

fn input_as_pdf(office_opt: &Option<String>, doc_id: Uuid, root_tmp_dir: &Path, _: (f32, f32), tx: &Box<dyn EventSender>, _: &ProgressRange, raw_input_path: &Path, opt_passwd: Option<String>, l10n: l10n::Translations) -> Result<String, error::Failure> {
    if !raw_input_path.exists() {
        let msg = l10n.gettext_fmt("Cannot find file at {0}", vec![&raw_input_path.display().to_string()]);
        return Err(io::Error::other(msg).into());
    }

    let file_format = FileFormat::from_file(raw_input_path)?;

    if let Some(mime_type) = file_format.short_name() {
        let filename_pdf: String = {
            if let Some(basename) = raw_input_path.file_stem().and_then(|i| i.to_str()) {
                let input_name = format!("{}.pdf", basename);
                root_tmp_dir.join(input_name.as_str()).display().to_string()
            } else {
                let msg = l10n.gettext_fmt("Could not determine basename for file {0}", vec![&raw_input_path.display().to_string()]);
                return Err(io::Error::other(msg).into());
            }
        };

        let path_loc = raw_input_path.display().to_string();
        let path_src = path_loc.as_str();
        let path_dest = filename_pdf.as_str();

        match mime_type {
            "PDF" => {
                progressed(doc_id, tx, 5, l10n.gettext_fmt("Copying PDF input to {0}", vec![&filename_pdf]));

                if let Some(passwd) = opt_passwd {
                    let mut ret_doc = PdfDocument::open(path_src)?;

                    if ret_doc.needs_password()? {
                        if ret_doc.authenticate(&passwd).is_err() {
                            return Err(error::Failure::InvalidInput(l10n.gettext("Wrong document password")));
                        }

                        let mut binding = PdfWriteOptions::default();
                        let options = binding.set_pretty(false).set_encryption(Encryption::None).set_garbage_level(4);
                        ret_doc.save_with_options(path_dest, *options)?;
                    } else {
                        fs::copy(path_src, path_dest)?;
                    }
                } else {
                    fs::copy(path_src, path_dest)?;
                }
            },
            "EPUB" | "MOBI" | "CBZ" | "FB2" | "BMP" | "PNM" | "PNG" | "JPEG" | "GIF" | "TIFF" => {
                progressed(doc_id, tx, 5, l10n.gettext("Converting input to PDF for images and ebooks processor."));
                img_to_pdf(path_src, path_dest)?;
            },
            "DOC" | "DOCX" | "ODG" | "ODP" | "ODS" | "ODT" | "PPT" | "PPTX" | "RTF" | "XLS" | "XLSX" => {
                progressed(doc_id, tx, 5, l10n.gettext("Converting input to PDF using office processor"));

                let file_extension = mime_type.to_lowercase();
                let now =  SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
                let new_input_loc = root_tmp_dir.join(format!("entrusted-{}.{}", now, file_extension));
                let new_input_path = Path::new(&new_input_loc);
                fs::copy(raw_input_path, new_input_path)?;

                officeproc::office_to_pdf(office_opt, opt_passwd, root_tmp_dir, new_input_path, &filename_pdf, &l10n)?;

                if let Err(ioex) = fs::remove_file(&new_input_loc) {
                    let msg = l10n.gettext_fmt("Failed to remove file from {0}!", vec![&new_input_loc.display().to_string()]);
                    return Err(io::Error::other(format!("{} {}", msg, ioex)).into());
                }
            },
            &_ => {
                return Err(error::Failure::InvalidInput(l10n.gettext("Mime type error! Does the input have a 'known' file extension?")));
            }
        }

        Ok(filename_pdf)
    } else {
        Err(error::Failure::InvalidInput(l10n.gettext("Mime type error! Does the input have a 'known' file extension?")))
    }
}

#[inline]
fn elapsed_time_string(millis: u128, l10n: &l10n::Translations) -> String {
    let total_seconds = millis / 1000;
    let hours   = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    format!("{} {} {}",
            l10n.ngettext("hour",   "hours",   hours   as u64),
            l10n.ngettext("minute", "minutes", minutes as u64),
            l10n.ngettext("second", "seconds", seconds as u64))
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

fn page_to_pixmap(page_num: usize, doc: &PdfDocument, target_image_size: (f32, f32)) -> Result<Document, mupdf::error::Error> {
    let page = doc.load_page(page_num as i32)?;

    let mediabox = page.bounds()?;
    let scale_x = target_image_size.0 / mediabox.width();
    let scale_y = target_image_size.1 / mediabox.height();
    let scale_factor = scale_x.min(scale_y);
    let matrix = Matrix::new_scale(scale_factor, scale_factor);

    let pixmap = page.to_pixmap(&matrix, &Colorspace::device_rgb(), false, true)?;
    let buffer = pixmap.get_image_bytes(ImageFormat::PNG)?;

    Document::from_bytes(&buffer, "png")
}

fn merge_pdfs(doc_uuid: Uuid, tx: &Box<dyn EventSender>, progress_range: &ProgressRange, page_count: usize, input_dir_path: &Path, output_path: &Path, password_encrypt: &Option<String>, l10n: &l10n::Translations) -> Result<(), error::Failure> {
    progressed(doc_uuid,
               tx,
               progress_range.min,
               l10n.ngettext("Combining one PDF document",
                             "Combining few PDF documents",
                             page_count as u64));

    let progress_delta = progress_range.delta();

    let mut progress_value = progress_range.min;
    progressed(doc_uuid, tx, progress_value, l10n.gettext("Collecting PDF pages"));

    let mut output_doc = PdfDocument::new();
    let mut page_index = 0;

    for i in 0..page_count {
        let idx = i + 1;
        progress_value = progress_range.min + (idx * progress_delta / page_count);
        let idx_text = idx.to_string();
        progressed(doc_uuid, tx, progress_value, l10n.gettext_fmt("Combining page {0} into final PDF", vec![&idx_text]));

        let src_path = input_dir_path.join(format!("page-{}.pdf", i ));
        let src_location = src_path.display().to_string();
        let src_location = src_location.as_str();
        let src_doc = PdfDocument::open(src_location)?;

        for page_num in 0..src_doc.page_count()? {
            let page = src_doc.find_page(page_num)?;
            let graft_obj = output_doc.graft_object(&page)?;
            output_doc.insert_page(page_index, &graft_obj)?;
            page_index += 1;
        }
    }

    progressed(doc_uuid, tx, progress_value, l10n.gettext("Saving PDF"));

    let mut binding = PdfWriteOptions::default();
    let options = binding.set_compress(true)
        .set_compress_images(true)
        .set_compress_fonts(true)
        .set_garbage_level(4);

    if let Some(ref encryption_password) = password_encrypt {
        const PERMISSIONS_PDF_SECURITY: Permission = Permission::ACCESSIBILITY.union(Permission::PRINT).union(Permission::COPY).union(Permission::ANNOTATE);
        options.set_permissions(PERMISSIONS_PDF_SECURITY);
        options.set_encryption(Encryption::Aes256);
        options.set_owner_password(encryption_password);
        options.set_user_password(encryption_password);
    }

    let output_loc = output_path.display().to_string();
    let output_loc = output_loc.as_str();

    if let Err(ex) = output_doc.save_with_options(output_loc, *options) {
        let msg = l10n.gettext_fmt("Could not save PDF file to {0}. {1}.", vec![&output_loc, &ex.to_string()]);
        return Err(io::Error::new(io::ErrorKind::NotFound, msg).into());
    }

    if fs::metadata(output_path).is_err() {
        let msg = l10n.gettext_fmt("Could not save PDF file to {0}.", vec![&output_loc]);
        return Err(io::Error::new(io::ErrorKind::NotFound, msg).into());
    }

    Ok(())
}

fn img_to_pdf(path_src: &str, path_dest: &str) -> Result<(), mupdf::error::Error> {
    let doc = Document::open(path_src)?;
    let mut writer = DocumentWriter::new(path_dest, "pdf", "")?;
    
    for page in doc.pages()? {
        let page = page?;
        let mediabox = page.bounds()?;
        let device = writer.begin_page(mediabox)?;
        page.run(&device, &Matrix::IDENTITY)?;
        writer.end_page(device)?;
    }

    Ok(())
}

// TODO investigate onlyoffice
mod officeproc {
    use std::path::Path;

    use crate::{error, l10n};

    pub fn office_to_pdf<P: AsRef<Path>>(office_opt: &Option<String>, opt_passwd: Option<String>, root_tmp_dir: &Path, path_src: P, dest_path_name: &String, trans: &l10n::Translations) -> Result<(), error::Failure> {
        #[cfg(not(target_os = "macos"))] {
            generic::office_to_pdf(office_opt, opt_passwd, root_tmp_dir, path_src, dest_path_name, trans)
        }

        #[cfg(target_os = "macos")] {
            macos::office_to_pdf(office_opt, opt_passwd, root_tmp_dir, path_src, dest_path_name, trans)
        }
    }

    #[cfg(not(target_os = "macos"))]
    mod generic {
        use std::sync::atomic::Ordering;
        use std::rc::Rc;
        use std::io;
        use libreofficekit::{Office, OfficeOptionalFeatures, CallbackType, DocUrl};
        use std::sync::atomic::AtomicBool;
        use std::path::Path;
        use std::sync::{Mutex, OnceLock};

        use crate::{error, l10n};

        #[derive(Clone)]
        struct OfficePtr {
            office_opt: Option<Office>
        }

        impl OfficePtr {
            fn new(office_opt: Option<Office>) -> Self {
                Self { office_opt }
            }
        }

        unsafe impl Send for OfficePtr {}

        static CELL: OnceLock<Mutex<OfficePtr>> = OnceLock::new();

        fn init_office(libreoffice_program_dir: &str) -> Mutex<OfficePtr> {
            if let Ok(oo) = Office::new(libreoffice_program_dir) {
                Mutex::new(OfficePtr::new(Some(oo.clone())))
            } else {
                Mutex::new(OfficePtr::new(None))
            }
        }

        pub fn office_to_pdf<P: AsRef<Path>>(office_opt: &Option<String>, opt_passwd: Option<String>, _root_tmp_dir: &Path, path_src: P, dest_path_name: &String, trans: &l10n::Translations) -> Result<(), error::Failure> {
            let office = {
                match office_opt {
                    Some(office_program_dir) => {
                        let office_mutex = CELL.get_or_init(|| init_office(office_program_dir));

                        if let Ok(office_mutex) = office_mutex.lock() {
                            if let Some(ref oo) = office_mutex.office_opt {
                                oo.clone()
                            } else {
                                return Err(error::Failure::FeatureMissing(trans.gettext("Office document support is not available!")));
                            }
                        } else {
                            return Err(error::Failure::FeatureMissing(trans.gettext("Office document support is not available!")));
                        }
                    },
                    None => {
                        return Err(error::Failure::RuntimeError(trans.gettext("The Office processor environment is unknown!")));
                    }
                }
            };

            let input_uri = DocUrl::from_absolute_path(path_src.as_ref().display().to_string())?;

            if let Some(passwd) = opt_passwd {
                let needs_password = Rc::new(AtomicBool::new(false));

                if let Err(ex) = office.set_optional_features(OfficeOptionalFeatures::DOCUMENT_PASSWORD) {
                    let msg = trans.gettext_fmt("Failed to enable password-protected document features! {0}", vec![&ex.to_string()]);
                    return Err(error::Failure::RuntimeError(msg));
                }

                if let Err(ex) = office.register_callback({
                    let needs_password = needs_password.clone();
                    let input_uri = input_uri.clone();

                    move |office, ty, _| {
                        if let CallbackType::DocumentPassword = ty {
                            if needs_password.swap(true, Ordering::Relaxed) {
                                let _ = office.set_document_password(&input_uri, None);
                                return;
                            }

                            let _ = office.set_document_password(&input_uri, Some(&passwd));
                        }
                    }
                }) {
                    let msg = trans.gettext_fmt("Failed to enable password-protected document features! {0}", vec![&ex.to_string()]);
                    return Err(error::Failure::RuntimeError(msg));
                }
            }

            let res_document_saved: Result<(), Box<dyn std::error::Error>> = match office.document_load(&input_uri) {
                Ok(mut doc) => {
                    match DocUrl::from_absolute_path(dest_path_name) {
                        Err(ex) => {
                            let msg = ex.to_string();
                            Err(trans.gettext_fmt("Could not save document as PDF: {0}", vec![&msg]).into())
                        },
                        Ok(doc_url) => {
                            if let Err(ex) = doc.save_as(&doc_url, "pdf", None) {
                                Err(trans.gettext_fmt("Could not save document as PDF: {0}", vec![&ex.to_string()]).into())
                            } else {
                                Ok(())
                            }
                        }
                    }
                },
                Err(ex) =>  {
                    let error_message = ex.to_string();
                    let mut office_error = error_message.clone();

                    if error_message.contains("Unsupported URL") {
                        office_error = trans.gettext("The file appears to be corrupted or unsupported!");
                    }

                    if error_message.contains("loadComponentFromURL returned an empty reference") {
                        office_error = trans.gettext("The file appears to be password-protected or it might not have been successfully decrypted!\n Please provide a password for the file with the 'Configure selected' link in the 'Upload' screen.");
                    }

                    Err(office_error.into())
                }
            };

            let _ = office.trim_memory(2000);

            if let Err(ex) = res_document_saved {
                let msg = trans.gettext_fmt("Could not export input document as PDF! {0}", vec![&ex.to_string()]);
                return Err(io::Error::other(msg).into());
            }

            Ok(())
        }
    }

    // TODO delete not a good solution (need to consider writing bindings for OnlyOffice)
    #[cfg(target_os = "macos")]
    mod macos {
        use std::path::Path;
        use std::{fs, io};

        use crate::{error, l10n};

        fn office_filter_name(path_name: &str) -> &str {
            if path_name.ends_with(".pptx") {
                return "impress_pdf_Export";
            } else if path_name.ends_with(".ppt") {
                return "impress_pdf_Export";
            } else if path_name.ends_with(".odp") {
                return "impress_pdf_Export";
            } else if path_name.ends_with(".odg") {
                return "impress_pdf_Export";
            } else if path_name.ends_with(".xlsx") {
                return "calc_pdf_Export";
            } else if path_name.ends_with(".xls") {
                return "calc_pdf_Export";
            } else if path_name.ends_with(".ods") {
                return "calc_pdf_Export";
            }

            "writer_pdf_Export"
        }

        pub fn office_to_pdf<P: AsRef<Path>>(office_opt: &Option<String>, opt_passwd: Option<String>, root_tmp_dir: &Path, path_src: P, dest_path_name: &String, trans: &l10n::Translations) -> Result<(), error::Failure> {
            if opt_passwd.is_some() {
                return Err(error::Failure::FeatureMissing("Sadly, password-protected documents are not supported on macOS!".to_string()));
            }

            if let Some(office_program_dir) = office_opt {
                let new_input_path = path_src.as_ref().display().to_string();
                let output_dir_libreoffice = root_tmp_dir.join("office_outdir");
                fs::create_dir_all(&output_dir_libreoffice)?;

                let filter_name = office_filter_name(&new_input_path);

                // TODO go to parent directory of bundled install (needs custom build instead of default install)
                let exec_status = std::process::Command::new("/Applications/LibreOffice.app/Contents/MacOS/soffice")
                    .arg("--headless")
                    .arg("--convert-to")
                    .arg(format!("pdf:{}", filter_name))
                    .arg("--outdir")
                    .arg(&output_dir_libreoffice)
                    .arg(new_input_path)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()?;

                if !exec_status.success() {
                    return Err(io::Error::other(trans.gettext("Could not export input document to PDF")).into());
                }

                let mut pdf_file_moved = false;

                for file in fs::read_dir(output_dir_libreoffice)? {
                    let file_path = file?.path();
                    let file_name = file_path.display().to_string();

                    if file_name.ends_with(".pdf") {
                        fs::copy(file_path, dest_path_name)?;
                        pdf_file_moved = true;
                        break;
                    }
                }

                if !pdf_file_moved {
                    return Err(io::Error::other("Could not export input document to PDF".to_string()).into())
                }

                Ok(())
            } else {
                Err(error::Failure::FeatureMissing("Office document support is not available!".to_string()))
            }
        }
    }
}
