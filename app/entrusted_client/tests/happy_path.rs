use std::collections::HashMap;
use cucumber::{gherkin::Step, given, then, when, World};
use assert_cmd::prelude::*;
use std::process::Command;
use std::env::temp_dir;
use std::fs;
use std::path::{Path, PathBuf};

#[given("a set of files to convert")]
async fn files_to_convert(files_to_convert: &mut FilesToConvert, step: &Step) {
    let output_folder = temp_dir().join("entrusted_client_tests");
    files_to_convert.output_folder = Some(output_folder.clone());

    if let Some(table) = step.table.as_ref() {
        for row in table.rows.iter().skip(1) { // NOTE: skip header
            let filename = &row[0];

            files_to_convert.files.entry(filename.clone())
                .or_insert(FileToConvert::new(filename.clone()));
        }
    }
}

#[when("files are converted")]
async fn files_are_converted(files_to_convert: &mut FilesToConvert) {
    let test_folder = Path::new("../../test_data");
    let output_folder = files_to_convert.output_folder.as_ref().unwrap().clone();

    if !output_folder.exists() {
        fs::create_dir_all(&output_folder).expect("Could not create output folder for tests");
    }

    for file_to_convert in files_to_convert.files.values_mut() {
        let test_file = test_folder.join(file_to_convert.filename.clone());
        let test_file_name = test_file.file_name().unwrap().to_str().unwrap();
        let output_file = output_folder.join(test_file_name.clone());

        assert!(test_file.exists(), "Cannot find test file at {}", test_file.display());

        let mut cmd = Command::cargo_bin("entrusted-cli").unwrap();

        cmd.arg("--input-filename");
        cmd.arg(test_file.display().to_string());
        cmd.arg("--output-filename");
        cmd.arg(output_file.display().to_string());

        file_to_convert.output_file = Some(output_file.clone());
        file_to_convert.assert_value  = Some(cmd.assert());
    }
}

#[then("the conversion is successful")]
async fn conversion_successful(files_to_convert: &mut FilesToConvert) {
    for file_to_convert in files_to_convert.files.values() {
        let p = file_to_convert.output_file.as_ref().unwrap();
        if fs::remove_file(p).is_err() {
            eprintln!("Could not delete temporary test file: {}", p.display());
        }
    }

    let p = files_to_convert.output_folder.as_ref().unwrap();
    if p.exists() {
        if fs::remove_dir(p).is_err() {
            eprintln!("Could not delete temporary test folder: {}", p.display());
        }
    } else {
        eprintln!("The output file was not created for {}!", p.display());
    }

    for file_to_convert in files_to_convert.files.values_mut() {
        file_to_convert.assert_value.take().unwrap().success();
    }
}

fn main() {
    futures::executor::block_on(FilesToConvert::run("tests/features/happy_path"));
}


#[derive(Debug)]
struct FileToConvert {
    filename: String,
    output_file: Option<PathBuf>,
    assert_value: Option<assert_cmd::assert::Assert>,
}

impl FileToConvert {
    fn new(filename: String) -> Self {
        Self {
            filename,
            output_file: None,
            assert_value: None
        }
    }
}

#[derive(Debug, World)]
struct FilesToConvert {
    files: HashMap<String, FileToConvert>,
    output_folder: Option<PathBuf>
}

impl Default for FilesToConvert {
    fn default() -> Self {
        Self {
            files: HashMap::new(),
            output_folder: None
        }
    }
}

