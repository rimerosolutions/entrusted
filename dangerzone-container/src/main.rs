use std::collections::HashMap;
use std::error::Error;
use std::process::{Command, Stdio};
use std::env;
use std::str;
use std::path::PathBuf;
use infer;
use std::fs;

#[derive(Clone, Debug)]
enum ConversionType {
    None,
    LibreOffice(String),
    Convert
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        return Err("Please provide a single argument: 'document-to-pixels' or 'pixels-to-pdf'".into())
    }

    let cmd = &args[1];

    match cmd.as_str() {
        "document-to-pixels" => document_to_pixels(),
        "pixels-to-pdf" => pixels_to_pdf(),
        _ => Err(format!("Unknown command {}", cmd).into())
    }
}

fn document_to_pixels() -> Result<(), Box<dyn Error>> {
    let conversion_by_mimetype: HashMap<&str, ConversionType> = [
        ("application/pdf"                                                           , ConversionType::None),
        ("application/vnd.openxmlformats-officedocument.wordprocessingml.document"   , ConversionType::LibreOffice("writer_pdf_Export".to_string())),
        ("application/vnd.ms-word.document.macroEnabled.12"                          , ConversionType::LibreOffice("writer_pdf_Export".to_string())),
        ("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"         , ConversionType::LibreOffice("calc_pdf_Export".to_string())),
        ("application/vnd.openxmlformats-officedocument.presentationml.presentation" , ConversionType::LibreOffice("impress_pdf_Export".to_string())),
        ("application/msword"                                                        , ConversionType::LibreOffice("writer_pdf_Export".to_string())),
        ("application/vnd.ms-excel"                                                  , ConversionType::LibreOffice("calc_pdf_Export".to_string())),
        ("application/vnd.ms-powerpoint"                                             , ConversionType::LibreOffice("impress_pdf_Export".to_string())),
        ("application/vnd.oasis.opendocument.text"                                   , ConversionType::LibreOffice("writer_pdf_Export".to_string())),
        ("application/vnd.oasis.opendocument.graphics"                               , ConversionType::LibreOffice("impress_pdf_Export".to_string())),
        ("application/vnd.oasis.opendocument.presentation"                           , ConversionType::LibreOffice("impress_pdf_Export".to_string())),
        ("application/vnd.oasis.opendocument.spreadsheet"                            , ConversionType::LibreOffice("calc_pdf_Export".to_string())),
        ("image/jpeg"                                                                , ConversionType::Convert),
        ("image/gif"                                                                 , ConversionType::Convert),
        ("image/png"                                                                 , ConversionType::Convert),
        ("image/tiff"                                                                , ConversionType::Convert),
        ("image/x-tiff"                                                              , ConversionType::Convert),
    ].iter().cloned().collect();

    let kind = infer::get_from_path("/tmp/input_file")
        .expect("Input file read successfully")
        .expect("File type is known");

    let mime_type = kind.mime_type();

    if !conversion_by_mimetype.contains_key(mime_type) {
        let msg = format!("Invalid mime type: {}", mime_type);
        return Err(msg.into());
    }

    let mut filename_pdf = "/tmp/input_file".to_string();

    if let Some(conversion_type) = conversion_by_mimetype.get(mime_type) {
        match conversion_type {
            ConversionType::None => {
                exec(Command::new("cp")
                     .arg("/tmp/input_file")
                     .arg("/tmp/input_file.pdf"),
                     "Cannot copy input with file extension, as required by some programs!")?;
                filename_pdf = "/tmp/input_file.pdf".to_string();
            },
            ConversionType::Convert => {
                println!("Converting to PDF using GraphicsMagick.");
                exec(Command::new("gm")
                     .arg("convert")
                     .arg("/tmp/input_file")
                     .arg("/tmp/input_file.pdf"),
                     "Failed to execute 'gm' process!")?;
                filename_pdf = "/tmp/input_file.pdf".to_string();
            },
            ConversionType::LibreOffice(output_filter) => {
                println!("Converting to PDF using LibreOffice");
                exec(Command::new("libreoffice")
                     .arg("--headless")
                     .arg("--convert-to")
                     .arg(format!("pdf:{}", output_filter))
                     .arg("--outdir")
                     .arg("/tmp")
                     .arg("/tmp/input_file"),
                     "Failed to execute 'libreoffice' process!")?;

                filename_pdf = "/tmp/input_file.pdf".to_string();
            }
        }
    }

    println!("Separating document into pages.");
    exec(Command::new("pdftk")
         .arg(filename_pdf)
         .arg("burst")
         .arg("output")
         .arg("/tmp/page-%d.pdf"),
         "Failed to execute 'pdftk' process!")?;

    let page_count = file_count("/tmp", "page-", ".pdf")?;

    println!("Pages found for pdf file: {}.", page_count);

    for i in 1..(page_count + 1) {
        filename_pdf = format!("/tmp/page-{}.pdf", i);
        let filename_png = format!("/tmp/page-{}.png", i);
        let filename_rgb = format!("/tmp/page-{}.rgb", i);
        let filename_dim = format!("/tmp/page-{}.dim", i);
        let filename_base = format!("/tmp/page-{}", i);

        println!("Converting page {} to pixels.", i);
        exec(Command::new("pdftocairo")
             .arg(filename_pdf)
             .arg("-png")
             .arg("-singlefile")
             .arg(filename_base),
             "Failed to execute 'pdftocairo' process!")?;

        let image_dimensions_output = Command::new("gm")
            .arg("identify")
            .arg("-ping")
            .arg("-format")
            .arg("%w,%h")
            .arg(filename_png.clone())
            .output()
            .expect("Failed to execute 'identify' process!");

        fs::write(filename_dim.clone(), str::from_utf8(&image_dimensions_output.stdout)?)?;

        exec(Command::new("gm")
             .arg("convert")
             .arg(filename_png.clone())
             .arg("-depth")
             .arg("8")
             .arg(format!("rgb:{}", filename_rgb)),
             "Failed to convert from PNG to pixels!")?;

        println!("Removing png file {}", filename_png.clone());

        fs::remove_file(filename_png.clone())?;
    }

    for f in fs::read_dir("/tmp")? {
        let file_path = f?.path();

        if let Some(filename_ostr) = file_path.file_name() {
            if let Some(filename) = filename_ostr.to_str() {
                if filename.starts_with("page-") {
                    if filename.ends_with(".rgb") || filename.ends_with(".dim") {
                        move_file_to_dir(fs::canonicalize(&file_path)?, "/dangerzone/")?;
                    }
                }
            }
        }
    }

    Ok(())
}

fn pixels_to_pdf() -> Result<(), Box<dyn Error>> {
    let page_count = file_count("/dangerzone", "page-", ".rgb")?;
    println!("Document has {} pages.", page_count);

    for page in 1..(page_count + 1) {
        let filename_base = format!("/dangerzone/page-{}", page);
        let filename_rgb = format!("{}.rgb", filename_base);
        let filename_dim = format!("{}.dim", filename_base);
        let filename_png = format!("/tmp/page-{}.png", page);
        let filename_ocr = format!("/tmp/page-{}", page);
        let filename_pdf = format!("/tmp/page-{}.pdf", page);

        let image_size = fs::read_to_string(filename_dim).expect("Unable to read image dimensions file!");
        let split = image_size.split(",").collect::<Vec<&str>>();
        let width = split[0];
        let height = split[1];

        match env::var("OCR") {
            Ok(ocr_set_value) => {
                match ocr_set_value.as_str() {
                    "1" => {
                        println!("Converting page {} from pixels to searchable PDF.", page);

                        exec(Command::new("gm")
                             .arg("convert")
                             .arg("-size")
                             .arg(format!("{}x{}", width, height))
                             .arg("-depth")
                             .arg("8")
                             .arg(format!("rgb:{}", filename_rgb))
                             .arg(format!("png:{}", filename_png)),
                             "Failed to execute 'gm' process!")?;

                        exec(Command::new("tesseract")
                             .arg(filename_png)
                             .arg(filename_ocr)
                             .arg("-l")
                             .arg(env::var("OCR_LANGUAGE")?)
                             .arg("--dpi")
                             .arg("70")
                             .arg("pdf"),
                             "Failed to execute 'tesseract' process!")?;
                    },
                    _ => {
                        println!("Converting page {} from pixels to non-searchable PDF.", page);
                        
                        exec(Command::new("gm")
                             .arg("convert")
                             .arg("-size")
                             .arg(format!("{}x{}", width, height))
                             .arg("-depth")
                             .arg("8")
                             .arg(format!("rgb:{}", filename_rgb))
                             .arg(format!("pdf:{}", filename_pdf)),
                             "Failed to execute 'gm' process!")?;
                    }
                }
            },
            Err(_) => {
                return Err("Could not read OCR environment variable. Please check your command!".into());
            }
        };

    }

    println!("Merging {} pages into a single PDF.", page_count);
    let mut pdfunite_args = Vec::with_capacity(page_count + 1);

    for page in 1..(page_count + 1) {
        pdfunite_args.push(format!("/tmp/page-{}.pdf", page));
    }

    pdfunite_args.push("/tmp/safe-output.pdf".to_string());

    exec(Command::new("pdfunite")
         .args(pdfunite_args),
         "Failed to execute 'pdfunite' process!")?;

    println!("Compressing PDF.");

    exec(Command::new("ps2pdf")
         .arg("/tmp/safe-output.pdf")
         .arg("/tmp/safe-output-compressed.pdf"),
         "Failed to execute 'ps2pdf' process!")?;

    move_file_to_dir(PathBuf::from("/tmp/safe-output.pdf"), "/safezone/")?;
    move_file_to_dir(PathBuf::from("/tmp/safe-output-compressed.pdf"), "/safezone/")?;

    Ok(())
}

fn move_file_to_dir(path: PathBuf, dest_dir: &str) -> Result<(), Box<dyn Error>> {
    let src_path = fs::canonicalize(&path)?;

    if let Some(filename_ostr) = path.file_name() {
        if let Some(filename) = filename_ostr.to_str() {
            let dest_path: PathBuf = [dest_dir, filename].iter().collect();

            fs::copy(&src_path, &dest_path)?;

            let result = fs::remove_file(&src_path);

            match result {
                Ok(()) => return Ok(()),
                Err(e) => return Err(e.into())
            }
        }
    }

    Err(format!("Cannot move file from {:?} to {}!", path, dest_dir).into())
}

fn file_count(dir_name: &str, file_prefix: &str, file_suffix: &str) -> Result<usize, Box<dyn Error>> {
    let mut counter = 0;

    for f in fs::read_dir(dir_name)? {
        let file_path = f?.path();

        if let Some(filename_ostr) = file_path.file_name() {
            if let Some(filename) = filename_ostr.to_str() {
                if filename.starts_with(file_prefix) && filename.ends_with(file_suffix) {
                    counter = counter + 1;
                }
            }
        }
    }

    Ok(counter)
}

fn exec(cmd: &mut Command, error_msg: &str) -> Result<(), Box<dyn Error>>{
    let mut cmdline: Vec<&str> = vec![];

    if let Some(program_str) = cmd.get_program().to_str() {
        cmdline.push(program_str);
    } else {
        return Err(format!("Cannot extract program executable from command from {:?}.", cmd.get_program()).into());
    }

    for cmd_arg in cmd.get_args() {
        if let Some(cmd_str) = cmd_arg.to_str() {
            cmdline.push(cmd_str);
        } else {
            return Err(format!("Cannot extract argument value from {:?}.", cmd_arg).into());
        }
    }

    println!("\n[CMD_REMOTE]: {}", cmdline.join(" "));

    cmd.stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .expect(error_msg);

    Ok(())
}
