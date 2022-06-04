Dangerzone README
==============================

* Requirements

- Please install Podman (preferred) or Docker
  - For Podman: https://podman.io/getting-started/installation
  - For Docker: https://docs.docker.com/desktop/linux/install/
- For the UI, you need to have FUSE installed (https://www.howtoinstall.me/ubuntu/18-04/libfuse2/)
  - Often the package is called "fuse2" for most Linux distributions
  - If you have issues just google "fuse2" for your specific Linux distribution

* About the binaries

- "Dangerzone_GUI-x86_64.AppImage" is the graphical user interface (GUI)
- "dangerzone-cli" is a CLI program that is the equivalent of the GUI in command-line mode
- "dangerzone-httpserver" is a CLI program that provides a shared service for document conversions, it has a minimal Web interface
- "dangerzone-httpclient" is a CLI program that communicates with "dangerzone-httpserver" via HTTP

* References

For more information, please visit the homepage: https://github.com/rimerosolutions/dangerzone-rust
