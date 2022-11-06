ARG ALPINE_VERSION=3.16.2
FROM docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION} as builder
LABEL stage=entrusted_container_builder

WORKDIR /home/rust/entrusted-container
COPY . .

RUN PKG_CONFIG_ALLOW_CROSS=1 LO_INCLUDE_PATH=/usr/include/LibreOfficeKit cargo install --path entrusted_container --root /usr/local \
    && cargo clean --manifest-path entrusted_container/Cargo.toml

FROM docker.io/alpine:${ALPINE_VERSION}

LABEL maintainer="Rimero Solutions Inc."
LABEL org.opencontainers.image.title="Entrusted document sanitizer container image"
LABEL org.opencontainers.image.description="Conversion of potentially suspicious documents to trusted PDFs, inside a container"
LABEL org.opencontainers.image.authors="Yves Zoundi"
LABEL org.opencontainers.image.vendor="Rimero Solutions Inc."
LABEL org.opencontainers.image.documentation="https://github.com/rimerosolutions/entrusted/tree/main/entrusted_container"
LABEL org.opencontainers.image.licenses="GPLv3"
LABEL org.opencontainers.image.version="0.2.6"
LABEL org.opencontainers.image.url="https://github.com/rimerosolutions/entrusted/tree/main/entrusted_container"
LABEL org.opencontainers.image.source="https://github.com/rimerosolutions/entrusted.git"

RUN apk -U upgrade && \
    apk add \
    leptonica-dev \
    icu-data-full \
    libreoffice-draw \
    libreoffice-writer \
    libreoffice-calc \
    libreoffice-impress \
    tesseract-ocr-dev \
    poppler-dev \
    cairo-dev \
    terminus-font ttf-inconsolata ttf-dejavu \
    tesseract-ocr-data-afr \
    tesseract-ocr-data-ara \
    tesseract-ocr-data-aze \
    tesseract-ocr-data-bel \
    tesseract-ocr-data-ben \
    tesseract-ocr-data-bul \
    tesseract-ocr-data-cat \
    tesseract-ocr-data-ces \
    tesseract-ocr-data-chi_sim \
    tesseract-ocr-data-chi_tra \
    tesseract-ocr-data-chr \
    tesseract-ocr-data-dan \
    tesseract-ocr-data-deu \
    tesseract-ocr-data-ell \
    tesseract-ocr-data-enm \
    tesseract-ocr-data-epo \
    tesseract-ocr-data-equ \
    tesseract-ocr-data-est \
    tesseract-ocr-data-eus \
    tesseract-ocr-data-fin \
    tesseract-ocr-data-fra \
    tesseract-ocr-data-frk \
    tesseract-ocr-data-frm \
    tesseract-ocr-data-glg \
    tesseract-ocr-data-grc \
    tesseract-ocr-data-heb \
    tesseract-ocr-data-hin \
    tesseract-ocr-data-hrv \
    tesseract-ocr-data-hun \
    tesseract-ocr-data-ind \
    tesseract-ocr-data-isl \
    tesseract-ocr-data-ita \
    tesseract-ocr-data-ita_old \
    tesseract-ocr-data-jpn \
    tesseract-ocr-data-kan \
    tesseract-ocr-data-kat \
    tesseract-ocr-data-kor \
    tesseract-ocr-data-lav \
    tesseract-ocr-data-lit \
    tesseract-ocr-data-mal \
    tesseract-ocr-data-mkd \
    tesseract-ocr-data-mlt \
    tesseract-ocr-data-msa \
    tesseract-ocr-data-nld \
    tesseract-ocr-data-nor \
    tesseract-ocr-data-pol \
    tesseract-ocr-data-por \
    tesseract-ocr-data-ron \
    tesseract-ocr-data-rus \
    tesseract-ocr-data-slk \
    tesseract-ocr-data-slv \
    tesseract-ocr-data-spa \
    tesseract-ocr-data-spa_old \
    tesseract-ocr-data-sqi \
    tesseract-ocr-data-srp \
    tesseract-ocr-data-swa \
    tesseract-ocr-data-swe \
    tesseract-ocr-data-tam \
    tesseract-ocr-data-tel \
    tesseract-ocr-data-tgl \
    tesseract-ocr-data-tha \
    tesseract-ocr-data-tur \
    tesseract-ocr-data-ukr \
    tesseract-ocr-data-vie

COPY --from=builder /usr/local/bin/entrusted-container /usr/local/bin/entrusted-container

RUN find /usr/lib/libreoffice -type f -name "*.py" -exec rm -rf {} \; \
    && rm -rf /usr/lib/python3.10 \
    && rm -rf /usr/include/* \
    && rm -rf /usr/lib/libreoffice/share/Scripts/* \
    && rm -rf /usr/lib/libreoffice/*.html && rm -rf /usr/lib/libreoffice/*.fodt \
    && rm -rf /usr/lib/libreoffice/program/classes/*.jar

# Add the unprivileged user
RUN adduser -h /home/entrusted -s /bin/sh -D entrusted
USER entrusted

# ENTRYPOINT ["/usr/local/bin/entrusted-container"]

# /tmp/input_file is where the first convert expects the input file to be, and
# /tmp where it will write the pixel files
#
# /entrusted is where the second script expects files to be put by the first one
#
# /safezone is where the wrapper eventually moves the sanitized files.
# VOLUME /tmp/input_file /safezone