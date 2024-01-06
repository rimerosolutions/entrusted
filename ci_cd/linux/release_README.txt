Entrusted README
==============================

* Requirements

** Software
- Please install Podman (preferred) or Docker
  - For Podman: https://podman.io/getting-started/installation
  - For Docker: https://docs.docker.com/desktop/linux/install/

** Libraries

The list below is for a Debian-based Linux distribution.
Package names may by slightly different for your Linux distribution.

- libc6 (>= 2.34) or a recent version of musl (if using musl builds)
- libcairo2 (>= 1.6.0) 
- libfontconfig1 (>= 2.12.6) 
- libgcc-s1 (>= 4.2) 
- libglib2.0-0 (>= 2.12.0) 
- libpango-1.0-0 (>= 1.44.3) 
- libpangocairo-1.0-0 (>= 1.14.0) 
- libwayland-client0 (>= 1.0.2) 
- libx11-6 
- libxcursor1 (>> 1.1.2) 
- libxfixes3 
- libxinerama1 (>= 2:1.1.4)

* About the binaries

- "entrusted-gui" is the graphical user interface (GUI)
- "entrusted-cli" is the command-line user interface (CLI)
- "entrusted-webserver" is a program that provides a shared service for document conversions
  - It has a minimal Web interface running by default on port 13000
  - It assumes that the entrusted-cli binary is added to your PATH variable
- "entrusted-webclient" is a program that communicates with "entrusted-webserver" via HTTP

* References

For more information, please visit the homepage: https://github.com/rimerosolutions/entrusted
