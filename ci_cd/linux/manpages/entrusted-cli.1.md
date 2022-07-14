% ENTRUSTED-CLI(1) Version 0.2.2 | Document Sanitizer Tool CLI

NAME
====

**entrusted-cli** — Command-line client for "Entrusted"

DESCRIPTION
===========

The Entrusted CLI program is a console aplication that converts
potentially suspicious files to safe PDF documents.

 - The solution requires a container engine (i.e. podman, docker)
 - You can optionally use optical character recognition (OCR):
  - You need to select text in the PDF
  - You need to be able to perform full-text search in the PDF
 - Most image and Office document formats are supported:
  - Images (.jpeg, .jpg, .gif, .tiff, .png)
  - Documents (spreadsheets, text documents, presentations)

OPTIONS
-------

`--input-filename`
  Input filename

`--output-filename`
  Optional output filename defaulting to <filename>-entrusted.pdf.

`--ocr-lang`
  Optional language for OCR (i.e. 'eng' for English)

`--container-image-name`
  Optional custom Docker or Podman image name

`--log-format`
  Log format (json or plain)

`--file-suffix`
  Default file suffix (entrusted)

`--passwd-prompt`
  Prompt for document password

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
