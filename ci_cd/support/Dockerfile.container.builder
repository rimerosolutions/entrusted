FROM docker.io/ubuntu:22.10

RUN apt update && apt upgrade -y && apt install --no-install-recommends -y \
    libleptonica-dev \
    libtesseract-dev \
    libreofficekit-dev \
    libpoppler-dev \
    libcairo2-dev rust-all ca-certificates \
    libclang-11-dev llvm gcc \
    libtiff-dev \
    libjpeg-dev \
    libgif-dev \
    libwebp-dev \
    libjpeg-dev \
    curl libpoppler-glib-dev && apt clean && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s --  --default-toolchain='1.64.0' -y
