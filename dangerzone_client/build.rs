fn main() {
    #[cfg(target_os = "windows")] {
        use embed_resource;
        embed_resource::compile("icon.rc");
    }
}
