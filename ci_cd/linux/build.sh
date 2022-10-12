#!/usr/bin/env sh
set -x

PREVIOUSDIR="$(echo $PWD)"
SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${SCRIPTDIR}/../..)"
APPVERSION=$(awk -F ' = ' '$1 ~ /version/ { gsub(/[\"]/, "", $2); printf("%s",$2) }' ${PROJECTDIR}/entrusted_client/Cargo.toml)
#CPU_ARCHS="aarch64"
CPU_ARCHS="amd64 aarch64"

for CPU_ARCH in ${CPU_ARCHS} ; do
    ARTIFACTSDIR="${PROJECTDIR}/artifacts/entrusted-linux-${CPU_ARCH}-${APPVERSION}"
    test -d ${ARTIFACTSDIR} && rm -rf ${ARTIFACTSDIR}
done

for CPU_ARCH in $CPU_ARCHS ; do
    ARTIFACTSDIR="${PROJECTDIR}/artifacts/entrusted-linux-${CPU_ARCH}-${APPVERSION}"
    PKG_FILE_DEB="${PROJECTDIR}/artifacts/entrusted-linux-${CPU_ARCH}-${APPVERSION}.deb"
    PKG_FILE_RPM="${PROJECTDIR}/artifacts/entrusted-linux-${CPU_ARCH}-${APPVERSION}.rpm"
    RUST_TARGET_STATIC="x86_64-unknown-linux-musl"
    RUST_TARGET_NOTSTATIC="x86_64-unknown-linux-gnu"
    DEB_ARCH="amd64"
    RPM_ARCH="x86_64"
    APPIMAGE_ARCH="x86_64"

    if [ ${CPU_ARCH} != "amd64" ]
    then
        RUST_TARGET_STATIC="aarch64-unknown-linux-musl"
        RUST_TARGET_NOTSTATIC="aarch64-unknown-linux-gnu"
        DEB_ARCH="arm64"
        RPM_ARCH="aarch64"
        APPIMAGE_ARCH="arm_aarch64"
    fi

    mkdir -p ${ARTIFACTSDIR}

    test -d ${PROJECTDIR}/entrusted_container/target && rm -rf ${PROJECTDIR}/entrusted_container/target
    test -d ${PROJECTDIR}/entrusted_client/target    && rm -rf ${PROJECTDIR}/entrusted_client/target
    test -d ${PROJECTDIR}/entrusted_webclient/target && rm -rf ${PROJECTDIR}/entrusted_webclient/target
    test -d ${PROJECTDIR}/entrusted_webserver/target && rm -rf ${PROJECTDIR}/entrusted_webserver/target

    cd ${PROJECTDIR}

    echo "Building entrusted_client (entrusted-gui) for ${CPU_ARCH}"

    if [ ${CPU_ARCH} != "amd64" ]
    then
        podman run --rm --platform linux/${DEB_ARCH} --volume "${PROJECTDIR}":/src  -v "${PROJECTDIR}/artifacts":/artifacts docker.io/rust:1.63.0-bullseye /bin/sh -c "cd /src && apt update && apt install -y xorg-dev musl-dev musl-tools musl wget cmake git libpango1.0-dev libxft-dev  libx11-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxcb1-dev libxcursor-dev gcc g++ git libpango1.0-dev libcairo2-dev cpio libjpeg62-turbo rpm2cpio fuse patchelf desktop-file-utils squashfuse; rustup target add ${RUST_TARGET_STATIC}; rustup target add ${RUST_TARGET_NOTSTATIC}; ln -sf /usr/bin/gcc /usr/bin/aarch64-linux-musl-gcc && ln -sf /usr/bin/ar /usr/bin/aarch64-linux-musl-ar; mkdir -p /tmp/support && cd /tmp/support; cd /tmp/support && wget -c https://download.opensuse.org/repositories/OBS:/AppImage/toolchain/aarch64/linuxdeploy-1569677113.1be3327-98.2.aarch64.rpm && wget -c https://download.opensuse.org/repositories/OBS:/AppImage/toolchain/aarch64/linuxdeploy-plugin-appimage-1569677660.81ffb98-6.2.aarch64.rpm && wget -c https://download.opensuse.org/repositories/OBS:/AppImage/toolchain/aarch64/appimagetool-13-2.1.aarch64.rpm && wget -c https://download.opensuse.org/repositories/OBS:/AppImage/toolchain/aarch64/squashfs-tools-4.4+git.1-2.1.aarch64.rpm && rpm2cpio linuxdeploy-1569677113.1be3327-98.2.aarch64.rpm | cpio -idmv && rpm2cpio linuxdeploy-plugin-appimage-1569677660.81ffb98-6.2.aarch64.rpm | cpio -idmv && rpm2cpio appimagetool-13-2.1.aarch64.rpm | cpio -idmv && rpm2cpio squashfs-tools-4.4+git.1-2.1.aarch64.rpm | cpio -idmv; cp ./usr/bin/* /usr/bin/ && cp -r ./usr/lib/appimagetool /usr/lib/; wget -c https://ftp.lysator.liu.se/pub/opensuse/distribution/leap/15.4/repo/oss/aarch64/libjpeg8-8.2.2-150400.15.9.aarch64.rpm && rpm2cpio libjpeg8-8.2.2-150400.15.9.aarch64.rpm | cpio -idmv && cp ./usr/lib64/*.so* /usr/lib/;RUSTFLAGS='-C link-arg=-lgcc -C link-arg=-lX11' cargo build --target ${RUST_TARGET_NOTSTATIC} --release --features=gui  --manifest-path /src/entrusted_client/Cargo.toml --bin entrusted-gui; ln -sf /usr/bin/gcc /usr/bin/aarch64-linux-musl-gcc && ln -sf /usr/bin/ar /usr/bin/aarch64-linux-musl-ar; mkdir -p /tmp/appdir/usr/bin /tmp/appdir/usr/share/icons && cp /src/ci_cd/linux/xdg/* /tmp/appdir/ && cd /src && cp /src/entrusted_client/target/${RUST_TARGET_NOTSTATIC}/release/entrusted-gui /tmp/appdir/ && cp /src/images/Entrusted.png /tmp/appdir/usr/share/icons/entrusted-gui.png && ARCH=${APPIMAGE_ARCH} linuxdeploy --appdir /tmp/appdir --desktop-file /tmp/appdir/entrusted-gui.desktop --icon-file /tmp/appdir/usr/share/icons/entrusted-gui.png --output appimage && mv *.AppImage /artifacts/entrusted-linux-${CPU_ARCH}-${APPVERSION}/entrusted-gui && CC_AARCH64_UNKNOWN_LINUX_MUSL=musl-gcc CXX_AARCH64_UNKNOWN_LINUX_MUSL=musl-g++ AR_AARCH64_UNKNOWN_LINUX_MUSL=ar CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc RUSTFLAGS='-C link-arg=-lgcc -C target-feature=+crt-static' cargo build --target ${RUST_TARGET_STATIC} --release --manifest-path /src/entrusted_webserver/Cargo.toml && CC_AARCH64_UNKNOWN_LINUX_MUSL=musl-gcc CXX_AARCH64_UNKNOWN_LINUX_MUSL=musl-g++ AR_AARCH64_UNKNOWN_LINUX_MUSL=ar CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc RUSTFLAGS='-C link-arg=-lgcc -C target-feature=+crt-static' cargo build --target ${RUST_TARGET_STATIC} --release --manifest-path /src/entrusted_webclient/Cargo.toml && CC_AARCH64_UNKNOWN_LINUX_MUSL=musl-gcc CXX_AARCH64_UNKNOWN_LINUX_MUSL=musl-g++ AR_AARCH64_UNKNOWN_LINUX_MUSL=ar CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc RUSTFLAGS='-C link-arg=-lgcc -C target-feature=+crt-static' cargo build --target ${RUST_TARGET_STATIC} --release --manifest-path /src/entrusted_client/Cargo.toml --bin entrusted-cli && RUSTFLAGS='-C link-arg=-lgcc -C target-feature=+crt-static' cargo build --target ${RUST_TARGET_STATIC} --release --manifest-path /src/entrusted_client/Cargo.toml --bin entrusted-cli && cp /src/entrusted_webclient/target/${RUST_TARGET_STATIC}/release/entrusted-webclient /src/entrusted_webserver/target/${RUST_TARGET_STATIC}/release/entrusted-webserver /src/entrusted_client/target/${RUST_TARGET_STATIC}/release/entrusted-cli /artifacts/entrusted-linux-${CPU_ARCH}-${APPVERSION}/"
        
        retVal=$?
        if [ $retVal -ne 0 ]; then
            echo "Failure to create ${CPU_ARCH} Linux binaries"
            exit 1
        fi        
    else
        podman run --platform linux/${DEB_ARCH} --rm --privileged -v "${PROJECTDIR}":/src -v "${PROJECTDIR}/artifacts":/artifacts docker.io/rust:1.63.0-bullseye /bin/sh -c "cd /src && apt update && apt install -y libjpeg62-turbo-dev squashfs-tools xorg-dev musl-dev musl-tools musl wget cmake git libpango1.0-dev libxft-dev  libx11-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxcb1-dev libxcursor-dev gcc g++ git libpango1.0-dev libcairo2-dev cpio rpm2cpio fuse patchelf desktop-file-utils squashfuse; rustup target add ${RUST_TARGET_STATIC}; rustup target add ${RUST_TARGET_NOTSTATIC}; mkdir -p /tmp/support && cd /tmp/support; cd /tmp/support && wget -c https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage && chmod a+x linuxdeploy*.AppImage && mv linuxdeploy*.AppImage /usr/bin/linuxdeploy; cargo build --target ${RUST_TARGET_NOTSTATIC} --release --features=gui  --manifest-path /src/entrusted_client/Cargo.toml --bin entrusted-gui && cargo build --release --target ${RUST_TARGET_STATIC} --manifest-path /src/entrusted_client/Cargo.toml --bin entrusted-cli && RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target ${RUST_TARGET_STATIC} --manifest-path /src/entrusted_webserver/Cargo.toml && RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target ${RUST_TARGET_STATIC} --manifest-path /src/entrusted_webclient/Cargo.toml ; strip /src/entrusted_webclient/target/${RUST_TARGET_STATIC}/release/entrusted-webclient && strip /src/entrusted_webserver/target/${RUST_TARGET_STATIC}/release/entrusted-webserver && strip /src/entrusted_client/target/${RUST_TARGET_STATIC}/release/entrusted-cli; cp /src/entrusted_webclient/target/${RUST_TARGET_STATIC}/release/entrusted-webclient /src/entrusted_webserver/target/${RUST_TARGET_STATIC}/release/entrusted-webserver /src/entrusted_client/target/${RUST_TARGET_STATIC}/release/entrusted-cli /artifacts/entrusted-linux-${CPU_ARCH}-${APPVERSION}/ && mkdir -p /tmp/appdir/usr/bin /tmp/appdir/usr/share/icons && cp /src/ci_cd/linux/xdg/* /tmp/appdir/ && cd /src && cp /src/entrusted_client/target/${RUST_TARGET_NOTSTATIC}/release/entrusted-gui /tmp/appdir/ && cp /src/images/Entrusted.png /tmp/appdir/usr/share/icons/entrusted-gui.png && ARCH=${APPIMAGE_ARCH} linuxdeploy --appdir /tmp/appdir --desktop-file /tmp/appdir/entrusted-gui.desktop --icon-file /tmp/appdir/usr/share/icons/entrusted-gui.png --output appimage && mv *.AppImage /artifacts/entrusted-linux-${CPU_ARCH}-${APPVERSION}/entrusted-gui"

        retVal=$?
        if [ $retVal -ne 0 ]; then
            echo "Failure to create Linux ${CPU_ARCH} Linux binaries"
            exit 1
        fi
    fi

    podman run --rm --platform linux/${DEB_ARCH} --volume "${PROJECTDIR}":/src  -v "${PROJECTDIR}/artifacts":/artifacts docker.io/rust:1.63.0-bullseye /bin/sh -c "cd /src && apt update && apt install -y build-essential rpm pandoc rpm2cpio && /src/ci_cd/linux/redhat.sh ${APPVERSION} /artifacts/entrusted-linux-${CPU_ARCH}-${APPVERSION}.rpm /src/images /artifacts/entrusted-linux-${CPU_ARCH}-${APPVERSION} ${RPM_ARCH} && /src/ci_cd/linux/debian.sh ${APPVERSION} /artifacts/entrusted-linux-${CPU_ARCH}-${APPVERSION}.deb /src/images /artifacts/entrusted-linux-${CPU_ARCH}-${APPVERSION} ${DEB_ARCH}"
        
    retVal=$?
    if [ $retVal -ne 0 ]; then
        echo "Failure to create Linux RPM and Debian packages for ${CPU_ARCH}"
        exit 1
    fi
        
    cp ${SCRIPTDIR}/release_README.txt ${ARTIFACTSDIR}/README.txt

    cd ${ARTIFACTSDIR}/.. && tar cvf entrusted-linux-${CPU_ARCH}-${APPVERSION}.tar entrusted-linux-${CPU_ARCH}-${APPVERSION}
done

cd ${SCRIPTDIR}
