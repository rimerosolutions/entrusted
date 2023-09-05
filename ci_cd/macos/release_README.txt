Entrusted README
==============================

* Requirements

Please install Docker Desktop:  https://docs.docker.com/desktop/mac/install/

Docker Desktop is required to avoid Apple applications sandboxing issues: 
- 'Entrusted' might not be able to locate your Docker installation, because it cannot see or call programs at random locations
- The Apple applications sandbox essentially isolate programs from each other on your machine, on top of other things

* About the binaries

The binaries are NOT signed under MacOS, so you might get a warning about the application when you try to run it.

Most users only care about the Entrusted Desktop GUI application (dmg installer).
The dmg installer can be found at https://github.com/rimerosolutions/entrusted/releases

Executables:
- "Entrusted.app" is the graphical user interface (GUI)
- "entrusted-cli" is a CLI program that is the equivalent of the GUI in command-line mode
- "entrusted-webserver" is a CLI program that provides a shared service for document conversions, it has a minimal Web interface
- "entrusted-webclient" is a CLI program that communicates with "entrusted-webserver" via HTTP

* References

For more information, please visit the homepage: https://github.com/rimerosolutions/entrusted
