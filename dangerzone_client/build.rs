fn main() {
    if let Ok(target_sys) = std::env::var("CARGO_CFG_TARGET_OS") {
        if target_sys == "windows" {
            embed_resource::compile("icon.rc");
        }
    }
}
