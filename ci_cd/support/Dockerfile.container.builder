FROM docker.io/alpine:3.16.2

RUN apk -U upgrade && \
    apk add \
    binutils \
    leptonica-dev \
    icu-data-full \
    tesseract-ocr-dev \
    libreofficekit \
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
