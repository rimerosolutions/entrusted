FROM docker.io/uycyjnzgntrn/debian:bullseye-rust-1.64.0 as builder
LABEL stage=entrusted_container_builder

WORKDIR /home/rust/entrusted-container
COPY . .

# Build the main binary
RUN CARGO_NET_RETRY=10 CARGO_NET_GIT_FETCH_WITH_CLI=true PKG_CONFIG_ALLOW_CROSS=1 LO_INCLUDE_PATH=/usr/include/LibreOfficeKit \
    cargo install --path entrusted_container --root /usr/local \
    && cargo clean --manifest-path entrusted_container/Cargo.toml \
    && strip --strip-unneeded /usr/local/bin/entrusted-container

FROM docker.io/debian:bullseye-slim

LABEL maintainer="Yves Zoundi, Rimero Solutions Inc."
LABEL org.opencontainers.image.title="Entrusted document sanitizer container image"
LABEL org.opencontainers.image.description="Conversion of potentially suspicious documents to trusted PDFs, inside a container"
LABEL org.opencontainers.image.authors="Yves Zoundi"
LABEL org.opencontainers.image.vendor="Rimero Solutions Inc."
LABEL org.opencontainers.image.documentation="https://github.com/rimerosolutions/entrusted/tree/main/app/entrusted_container"
LABEL org.opencontainers.image.licenses="GPLv3"
LABEL org.opencontainers.image.version="0.3.1"
LABEL org.opencontainers.image.url="https://github.com/rimerosolutions/entrusted/tree/main/app/entrusted_container"
LABEL org.opencontainers.image.source="https://github.com/rimerosolutions/entrusted.git"

# Minimal runtime dependencies
RUN echo "deb [trusted=yes] https://notesalexp.org/tesseract-ocr5/bullseye/ bullseye main" >> /etc/apt/sources.list \
    && DEBIAN_FRONTEND=noninteractive apt-get -o "Acquire::https::Verify-Peer=false" update \
    && DEBIAN_FRONTEND=noninteractive apt-get -o "Acquire::https::Verify-Peer=false" install --no-install-recommends -y \
    libleptonica-dev \
    libtesseract-dev \
    libreoffice-writer \
    libreoffice-draw \
    libreoffice-impress \
    libreoffice-calc \
    libpoppler-dev \
    libcairo2-dev \
    tesseract-ocr-all \
    fonts-dejavu \
    libpoppler-glib-dev \
    && apt-get clean

# Copy the main binary from the build container
COPY --from=builder /usr/local/bin/entrusted-container /usr/local/bin/entrusted-container

# Remove what we do not care about
RUN /bin/sh -c "find /usr -type f -name '*.py' -o -name '*.zip' -o -name '*.gz' -o -name '*.png' -o -name '*.xpm' -o -name '*.jar' -o -name '*.pyc' -o -name '*.pdf' -exec rm -rf {} \;" \
    && useradd -ms /bin/dash entrusted \
    && rm /usr/bin/ambiguous_words \
    && rm /usr/bin/classifier_tester \
    && rm /usr/bin/cntraining \
    && rm /usr/bin/combine_lang_model \
    && rm /usr/bin/combine_tessdata \
    && rm /usr/bin/dawg2wordlist \
    && rm /usr/bin/lstmeval \
    && rm /usr/bin/lstmtraining \
    && rm /usr/bin/merge_unicharsets \
    && rm /usr/bin/mftraining \
    && rm /usr/bin/set_unicharset_properties \
    && rm /usr/bin/shapeclustering \
    && rm /usr/bin/tesseract \
    && rm /usr/bin/text2image \
    && rm /usr/bin/unicharset_extractor \
    && rm /usr/bin/wordlist2dawg \
    && rm -rf /usr/share/common-licenses /usr/share/doc-base \
    && rm -rf /usr/share/perl* /usr/bin/perl* /usr/lib/x86_64-linux-gnu/perl* \
    && rm -rf /usr/lib/python* /usr/bin/python* /usr/share/python* \
    && rm -rf /usr/sbin/*add* /usr/sbin/*del* /usr/local/include \
    && rm -rf /usr/include /usr/share/icons /usr/share/applications /usr/share/pixmaps \
    && rm -rf /usr/games /var/cache/apt /var/lib/apt/lists /usr/sbin/dpkg* /usr/lib/dpkg* /var/lib/dpkg \
    && rm -rf /usr/bin/apt* /usr/lib/apt/* /usr/bin/dpkg* /usr/bin/debconf* /var/cache/debconf \
    && rm -rf /usr/share/doc /usr/share/java /usr/share/man /usr/share/info  /var/log \
    && rm -rf /usr/lib/libreoffice/share/Scripts/* /usr/lib/libreoffice/share/palette/* /usr/lib/libreoffice/share/wizards/* \
    && rm -rf /usr/lib/libreoffice/share/theme_definitions /usr/lib/libreoffice/share/autocorr \
    && rm -rf /usr/lib/libreoffice/program/*.png \
    && rm -rf /usr/lib/libreoffice/share/template  /usr/lib/libreoffice/share/gallery /usr/lib/libreoffice/share/tipoftheday \
    && rm -rf /usr/lib/libreoffice/*.html /usr/lib/libreoffice/*.fodt \
    && rm -rf /usr/lib/libreoffice/program/shell/* \
    && rm -rf /usr/lib/libreoffice/basic/Tutorials /usr/lib/libreoffice/share/basic/Tutorials /usr/lib/libreoffice/basic/ImportWizard \
    && rm -rf /usr/lib/libreoffice/program/classes/*.jar /usr/lib/libreoffice/program/wizards \
    && rm -rf /usr/local/games /usrlocal/include /usr/local/etc /usr/local/lib /usr/local/man /usr/local/share /usr/local/sbin /usr/local/src \
    && rm -rf /usr/bin/libreoffice /usr/bin/lp_solve /usr/bin/lowriter /usr/bin/loweb /usr/bin/loimpress /usr/bin/localc \
    && rm -rf /usr/bin/py* /usr/bin/pod2* /usr/bin/loffice /usr/bin/lofromtemplate \
    && rm -rf /usr/lib/libreoffice/share/basic/FormWizard /usr/lib/libreoffice/share/basic/Template

# Drop to unprivileged user
USER entrusted

ENV ENTRUSTED_LIBREOFFICE_PROGRAM_DIR /usr/lib/libreoffice/program
ENV ENTRUSTED_TESSERACT_TESSDATA_DIR  /usr/share/tesseract-ocr/5/tessdata

# We're not declaring all volumes explicity. input_file might get interpreted as a directory for squashed images somehow
# Need to revisit the above claim in the future, based on old observations /tmp/input_file is not explicit....
#
# /tmp/input_file is where the first convert expects the input file to be, and /tmp where it will write the pixel files
#
# /safezone is where the wrapper eventually moves the sanitized files.
VOLUME /safezone
