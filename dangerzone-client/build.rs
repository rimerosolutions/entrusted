fn main() {
   // Hide the console on GUI startup for MS Windows
   if cfg!(any(target_os="windows")) {
      println!("cargo:rustc-link-arg=/SUBSYSTEM:WINDOWS");
      println!("cargo:rustc-link-arg=/ENTRY:mainCRTStartup");
   }
}
