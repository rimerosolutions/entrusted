% ENTRUSTED-GUI(1) Version 0.2.2 | Document Sanitizer Tool GUI

NAME
====

**entrusted-gui** — Desktop client for "Entrusted"

DESCRIPTION
===========

The Entrusted GUI program is a graphical aplication that converts
potentially suspicious files to safe PDF documents.

 - The solution requires a container engine (i.e. podman, docker)
 - You can optionally use optical character recognition (OCR):
  - You need to select text in the PDF
  - You need to be able to perform full-text search in the PDF
 - Most image and Office document formats are supported:
  - Images (.jpeg, .jpg, .gif, .tiff, .png)
  - Documents (spreadsheets, text documents, presentations)

FILES
=====

*$XDG_CONFIG_HOME/config.toml*

:   Per-user optional configuration file

ENVIRONMENT
===========

**ENTRUSTED_LANGID**

:   A custom language to use for the program (if supported).
    Language detection falls back to English for unsupported locales.

BUGS
====

See GitHub Issues: https://github.com/rimerosolutions/entrusted/issues

AUTHOR
======

Yves Zoundi <yves_zoundi@hotmail.com>
