FROM docker.io/alpine:latest as builder

RUN apk -U upgrade && \
    apk add \
    binutils \
    leptonica-dev \
    icu-data-full \
    tesseract-ocr-dev \
    poppler-dev \
    cairo-dev \
    clang gcc clang-dev gcc cargo rust \
    tiff-dev \
    jpeg-dev \
    giflib-dev \
    libwebp-dev \
    openjpeg-dev \
    musl-dev \
    curl

WORKDIR /home/rust/entrusted-container
COPY . .
RUN cargo install --path entrusted_container

FROM docker.io/alpine:latest

LABEL maintainer="Yves Zoundi, Rimero Solutions Inc."
LABEL org.opencontainers.image.title="Entrusted document sanitizer container image"
LABEL org.opencontainers.image.description="Conversion of potentially suspicious documents to trusted PDFs, inside a container"
LABEL org.opencontainers.image.authors="Yves Zoundi"
LABEL org.opencontainers.image.vendor="Rimero Solutions Inc."
LABEL org.opencontainers.image.documentation="https://github.com/rimerosolutions/entrusted/tree/main/entrusted_container"
LABEL org.opencontainers.image.licenses="GPLv3"
LABEL org.opencontainers.image.version="0.1.3"
LABEL org.opencontainers.image.url="https://github.com/rimerosolutions/entrusted/tree/main/entrusted_container"
LABEL org.opencontainers.image.source="https://github.com/rimerosolutions/entrusted.git"

RUN apk -U upgrade && \
    apk add \
    leptonica-dev \
    icu-data-full \
    libreoffice \
    tesseract-ocr-dev \
    poppler-dev \
    cairo-dev \
    terminus-font ttf-inconsolata ttf-dejavu \
    tesseract-ocr \
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

COPY --from=builder /root/.cargo/bin/entrusted-container /usr/local/bin/

# Add the unprivileged user
RUN adduser -h /home/entrusted -s /bin/sh -D entrusted
USER entrusted

# /tmp/input_file is where the first convert expects the input file to be, and
# /tmp where it will write the pixel files
#
# /entrusted is where the second script expects files to be put by the first one
#
# /safezone is where the wrapper eventually moves the sanitized files.
VOLUME /entrusted /tmp/input_file /safezone
