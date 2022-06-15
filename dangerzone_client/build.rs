use polib::{mo_file, po_file};
use std::error::Error;
use std::path::Path;
use std::fs;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=translations");

    let dir = Path::new("translations");
    
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let input = path.join("LC_MESSAGES").join("messages.po");

        if input.exists() {
            println!("cargo:info={}", format!("Processing translation PO file: {}", &input.display()));
                
            let output = path.join("LC_MESSAGES").join("messages.mo");
            let catalog = po_file::parse(Path::new(&input));

            if let Err(ex) = catalog {
                return Err(format!("Failed to parse PO file {}.\n{}", &input.display(), ex.to_string()).into());
            }

            let catalogc = catalog.unwrap();

            if let Err(ex) = mo_file::write(&catalogc, Path::new(&output)) {
                return Err(format!("Failed to compile MO file for {}.\n{}", &input.display(), ex.to_string()).into());
            }
        }
    }

    if let Ok(target_sys) = std::env::var("CARGO_CFG_TARGET_OS") {
        if target_sys == "windows" {
            embed_resource::compile("icon.rc");
        }
    }
    
    Ok(())
}
