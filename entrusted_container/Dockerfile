FROM docker.io/rust:1.58-bullseye as builder
LABEL stage=entrusted_container_builder

RUN DEBIAN_FRONTEND=noninteractive apt update && apt install -y \
    libleptonica-dev \
    libtesseract-dev \
    libreofficekit-dev \
    libpoppler-dev \
    libcairo2-dev \
    libclang-11-dev llvm gcc \
    libtiff-dev \
    libjpeg-dev \
    libgif-dev \
    libwebp-dev \
    libjpeg-dev \
    curl libpoppler-glib-dev && apt-get clean

WORKDIR /home/rust/entrusted-container
COPY . .
RUN LO_INCLUDE_PATH=/usr/include/LibreOfficeKit cargo build --release --manifest-path entrusted_container/Cargo.toml && cp entrusted_container/target/release/entrusted-container /home/rust/entrusted-container/executable && cargo clean --manifest-path entrusted_container/Cargo.toml

FROM docker.io/debian:bullseye-slim

LABEL maintainer="Yves Zoundi, Rimero Solutions Inc."
LABEL org.opencontainers.image.title="Entrusted document sanitizer container image"
LABEL org.opencontainers.image.description="Conversion of potentially suspicious documents to trusted PDFs, inside a container"
LABEL org.opencontainers.image.authors="Yves Zoundi"
LABEL org.opencontainers.image.vendor="Rimero Solutions Inc."
LABEL org.opencontainers.image.documentation="https://github.com/rimerosolutions/entrusted/tree/main/entrusted_container"
LABEL org.opencontainers.image.licenses="GPLv3"
LABEL org.opencontainers.image.version="0.2.1"
LABEL org.opencontainers.image.url="https://github.com/rimerosolutions/entrusted/tree/main/entrusted_container"
LABEL org.opencontainers.image.source="https://github.com/rimerosolutions/entrusted.git"

RUN DEBIAN_FRONTEND=noninteractive apt update && apt install -y --no-install-recommends \
    libleptonica-dev \
    libtesseract-dev \
    libreofficekit-dev \
    libreoffice \
    libpoppler-dev \
    libcairo2-dev \
    libreoffice \
    tesseract-ocr-all \
    fonts-terminus fonts-dejavu fonts-inconsolata libpoppler-glib-dev && apt-get clean && \
    ln -sf /usr/share/tesseract-ocr/4.00/tessdata /usr/share/tessdata

COPY --from=builder /home/rust/entrusted-container/executable /usr/local/bin/entrusted-container

# Add the unprivileged user
RUN useradd -ms /bin/bash entrusted
USER entrusted

# /tmp/input_file is where the first convert expects the input file to be, and
# /tmp where it will write the pixel files
#
# /entrusted is where the second script expects files to be put by the first one
#
# /safezone is where the wrapper eventually moves the sanitized files.
VOLUME /entrusted /tmp/input_file /safezone
