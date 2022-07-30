use std::io::{Cursor, BufReader, Read};
use std::error::Error;
use std::path::PathBuf;
use std::fs;
use zip;
use cfb;

pub fn detect_from_path (path: &PathBuf) -> Result<Option<String>, Box<dyn Error>> {
    let mut data = [0u8; 8];
    let mut f: fs::File = fs::File::open(path)?;    
    f.read(&mut data)?;

    if is_png(&data) {
        return Ok(Some("image/png".to_string()));
    } else if is_gif(&data) {
        return Ok(Some("image/gif".to_string()));
    } else if is_jpeg(&data) {
        return Ok(Some("image/jpeg".to_string()));
    } else if is_tiff(&data) {
        return Ok(Some("image/tiff".to_string()));
    } else if is_rtf(&data) {
        return Ok(Some("application/rtf".to_string()));
    } else if is_pdf(&data) {
        return Ok(Some("application/pdf".to_string()));
    } else if is_zip(&data) {        
        let data = fs::read(path)?;
        return office_mime(&data);
    } else if is_cfb(&data) {
        let data = fs::read(path)?;
        return legacy_office_mime(&data);
    }

    Ok(None)
}

fn bytes_range(data: &[u8], lo: usize, hi: usize) -> Vec<u8> {
    let mut ret = Vec::with_capacity(hi - lo);

    for i in lo..hi {
        ret.push(data[i]);
    }

    ret
}

fn hex_encode_upper(data: &[u8]) -> String {
    let mut hex_vec = String::with_capacity(data.len() * 2);

    for i in 0..data.len() {
        let hex = format!("{:02x}", data[i]);
        hex_vec.push_str(&hex);
    }

    hex_vec.to_uppercase()
}

fn byte_range_matches(data: &[u8], lo: usize, hi: usize, sig: &str) -> bool {
    if data.len() < hi {
        return false;
    }

    let file_sig = bytes_range(data, lo, hi);
    let hex_file_sig = hex_encode_upper(&file_sig);
    let sig_trimmed = sig.replace(" ", "");

    return hex_file_sig == sig_trimmed
}

fn is_zip(data: &[u8]) -> bool {
    byte_range_matches(data, 0, 4, "50 4B 03 04")
}

fn is_cfb(data: &[u8]) -> bool {
    byte_range_matches(data, 0, 8, "D0 CF 11 E0 A1 B1 1A E1")
}

fn is_rtf(data: &[u8]) -> bool {
    byte_range_matches(data, 0, 5, "7B 5C 72 74 66")
}

fn is_pdf(data: &[u8]) -> bool {
    byte_range_matches(data, 0, 4, "25 50 44 46")
}

fn is_png(data: &[u8]) -> bool {
    byte_range_matches(data, 0, 8, "89 50 4E 47 0D 0A 1A 0A")
}

fn is_gif(data: &[u8]) -> bool {
    byte_range_matches(data, 0, 6, "47 49 46 38 39 61")
}

fn is_jpeg(data: &[u8]) -> bool {
    byte_range_matches(data, 0, 2, "FF D8")
}

fn is_tiff(data: &[u8]) -> bool {
    byte_range_matches(data, 0, 4, "49 49 2A 00")
}

fn office_mime(data: &[u8]) -> Result<Option<String>, Box<dyn Error>> {
    let reader = Cursor::new(data);
    let mut zip = zip::ZipArchive::new(reader)?;
    let probe_count_expected = 2;
    let mut probe_count_odt = 0;
    let mut probe_count_ooxml = 0;
    let mut ret_odt  = "";
    let mut ret_ooxml  = "";

    fn of_interest_openxml(name: &str) -> bool {
        name == "_rels/.rels" || name == "[Content_Types].xml"
    }

    fn of_interest_opendocument(name: &str) -> bool {
        name == "mimetype" || name == "content.xml"
    }

    fn office_file_of_interest(name: &str) -> bool {
        of_interest_opendocument(name) || of_interest_openxml(name)
    }

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
                return Ok(Some(ret_odt.to_string()));
            } else if probe_count_ooxml == probe_count_expected {
                return Ok(Some(ret_ooxml.to_string()));
            }
        }
    }

    Ok(None)
}

fn legacy_office_mime(data: &[u8]) -> Result<Option<String>, Box<dyn Error>> {
    match cfb::CompoundFile::open(Cursor::new(data)) {
        Ok(file) => {
            return match file.root_entry().clsid().to_string().as_str() {
                "00020810-0000-0000-c000-000000000046" | "00020820-0000-0000-c000-000000000046" => {
                    Ok(Some("application/vnd.ms-excel".to_string()))
                },
                "00020906-0000-0000-c000-000000000046" => Ok(Some("application/msword".to_string())),
                "64818d10-4f9b-11cf-86ea-00aa00b929e8" => Ok(Some("application/vnd.ms-powerpoint".to_string())),
                _ => Ok(None),
            };
        },
        Err(ex) => Err(ex.into())
    }
}
