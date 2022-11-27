use polib::{mo_file, po_file};
use std::error::Error;
use std::path::Path;
use std::fs;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=translations");
    
    for entry in fs::read_dir(Path::new("translations"))? {
        let path = entry?.path();
        let input = path.join("LC_MESSAGES").join("messages.po");

        if input.exists() {
            println!("cargo:info=Processing translation PO file: {}", &input.display());
                
            let output = path.join("LC_MESSAGES").join("messages.mo");
            let catalog_ret = po_file::parse(Path::new(&input));

            match catalog_ret {
                Ok(catalog) => {                    
                    if let Err(ex) = mo_file::write(&catalog, Path::new(&output)) {
                        return Err(format!("Failed to compile MO file for {}.\n{}", &input.display(), ex).into());
                    }                    
                },
                Err(ex) => {
                    return Err(format!("Failed to parse PO file {}.\n{}", &input.display(), ex).into());
                }
            }
        }
    }
    
    Ok(())
}
