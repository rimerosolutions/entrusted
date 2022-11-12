FROM docker.io/uycyjnzgntrn/debian:bullseye-rust-1.64.0 as builder
LABEL stage=entrusted_container_builder

WORKDIR /home/rust/entrusted-container
COPY . .

RUN PKG_CONFIG_ALLOW_CROSS=1 LO_INCLUDE_PATH=/usr/include/LibreOfficeKit cargo install --path entrusted_container --root /usr/local \
    && cargo clean --manifest-path entrusted_container/Cargo.toml

FROM docker.io/debian:bullseye-slim

LABEL maintainer="Yves Zoundi, Rimero Solutions Inc."
LABEL org.opencontainers.image.title="Entrusted document sanitizer container image"
LABEL org.opencontainers.image.description="Conversion of potentially suspicious documents to trusted PDFs, inside a container"
LABEL org.opencontainers.image.authors="Yves Zoundi"
LABEL org.opencontainers.image.vendor="Rimero Solutions Inc."
LABEL org.opencontainers.image.documentation="https://github.com/rimerosolutions/entrusted/tree/main/app/entrusted_container"
LABEL org.opencontainers.image.licenses="GPLv3"
LABEL org.opencontainers.image.version="0.2.6"
LABEL org.opencontainers.image.url="https://github.com/rimerosolutions/entrusted/tree/main/app/entrusted_container"
LABEL org.opencontainers.image.source="https://github.com/rimerosolutions/entrusted.git"

RUN DEBIAN_FRONTEND=noninteractive apt update && apt upgrade -y && apt install --no-install-recommends -y \
    libleptonica-dev \
    libtesseract-dev \
    libreofficekit-dev \
    libreoffice \
    libpoppler-dev \
    libcairo2-dev \
    libreoffice \
    tesseract-ocr-all \
    fonts-dejavu libpoppler-glib-dev && apt clean

COPY --from=builder /usr/local/bin/entrusted-container /usr/local/bin/entrusted-container

RUN find /usr/lib/libreoffice -type f -name "*.py" -exec rm -rf {} \; \
    && rm -rf /usr/lib/python* \
    && rm -rf /usr/include/* /usr/share/icons/* \
    && rm -rf /usr/games /var/cache/apt /var/lib/apt/lists \
    && rm -rf /usr/bin/apt* && rm -rf /usr/bin/dpkg* && rm -rf /usr/bin/dpkg* && rm -rf /usr/bin/dpkg*\
    && rm -rf /usr/share/doc /usr/share/java /usr/share/man /usr/share/info  /var/log \
    && rm -rf /usr/lib/libreoffice/share/Scripts/* \
    && rm -rf /usr/lib/libreoffice/share/gallery \
    && rm -rf /usr/lib/libreoffice/*.html && rm -rf /usr/lib/libreoffice/*.fodt \
    && rm -rf /usr/lib/libreoffice/program/classes/*.jar

# Add the unprivileged user
RUN useradd -ms /bin/bash entrusted
USER entrusted

# /tmp/input_file is where the first convert expects the input file to be, and
# /tmp where it will write the pixel files
#
# /entrusted is where the second script expects files to be put by the first one
#
# /safezone is where the wrapper eventually moves the sanitized files.
# VOLUME /entrusted /tmp/input_file /safezone
