FROM docker.io/rust:1.72.0-bookworm

RUN DEBIAN_FRONTEND=noninteractive apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install --no-install-recommends -y \
    libleptonica-dev \
    libtesseract-dev \
    libreofficekit-dev \
    libpoppler-dev \
    libcairo2-dev \
    libclang-dev \
    llvm \
    gcc \
    libtiff-dev \
    libjpeg-dev \
    libgif-dev \
    libwebp-dev \
    libjpeg-dev \
    curl \
    libpoppler-glib-dev \
    && DEBIAN_FRONTEND=noninteractive apt-get clean

