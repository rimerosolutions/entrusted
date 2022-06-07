fn main() {
    #[cfg(target_os = "windows")] {
        use winres;
        let mut res = winres::WindowsResource::new();
        res.set_icon("images/Dangerzone_icon.ico");

        if cfg!(unix) {
            // paths for X64 on archlinux
            res.set_toolkit_path("/usr/x86_64-w64-mingw32/bin");
            // ar tool for mingw in toolkit path
            res.set_ar_path("ar");
            // windres tool
            res.set_windres_path("/usr/bin/x86_64-w64-mingw32-windres");
        }
        
        res.compile().unwrap();
    }
}
