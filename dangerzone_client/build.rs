fn main() {
    #[cfg(target_os = "windows")] {
        use winres;
        let mut res = winres::WindowsResource::new();
        res.set_icon("images/Dangerzone_icon.ico");
        res.compile().unwrap();
    }
}
