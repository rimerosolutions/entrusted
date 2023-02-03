FROM docker.io/rust:1.67.0-bullseye

RUN echo "deb [trusted=yes] https://notesalexp.org/tesseract-ocr5/bullseye/ bullseye main" >> /etc/apt/sources.list \
    && DEBIAN_FRONTEND=noninteractive apt-get -o "Acquire::https::Verify-Peer=false" update \
    && DEBIAN_FRONTEND=noninteractive apt-get -o "Acquire::https::Verify-Peer=false" install --no-install-recommends -y \
    libleptonica-dev \
    libtesseract-dev \
    libreofficekit-dev \
    libpoppler-dev \
    libcairo2-dev \
    libclang-11-dev \
    llvm \
    gcc \
    libtiff-dev \
    libjpeg-dev \
    libgif-dev \
    libwebp-dev \
    libjpeg-dev \
    curl \
    libpoppler-glib-dev \
    && apt clean


