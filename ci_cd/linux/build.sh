#!/usr/bin/env sh
set -x

PREVIOUSDIR="$(echo $PWD)"
SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${SCRIPTDIR}/../../app)"
APPVERSION=$(grep "^version" ${PROJECTDIR}/entrusted_client/Cargo.toml  | cut -d"=" -f2 | xargs)
CPU_ARCHS="amd64 aarch64"
RUST_CI_VERSION="1.72.0"

for CPU_ARCH in ${CPU_ARCHS} ; do
    ARTIFACTSDIR="${PROJECTDIR}/../artifacts/entrusted-${APPVERSION}-linux-${CPU_ARCH}"
    test -d ${ARTIFACTSDIR} && rm -rf ${ARTIFACTSDIR}
done

for CPU_ARCH in $CPU_ARCHS ; do
    ARTIFACTSDIR="${PROJECTDIR}/../artifacts/entrusted-${APPVERSION}-linux-${CPU_ARCH}"
    PKG_FILE_DEB="${PROJECTDIR}/../artifacts/entrusted-${APPVERSION}-linux-${CPU_ARCH}.deb"
    PKG_FILE_RPM="${PROJECTDIR}/../artifacts/entrusted-${APPVERSION}-linux-${CPU_ARCH}.rpm"
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
        podman run --rm --platform linux/${DEB_ARCH} --volume "${PROJECTDIR}":/src  -v "${PROJECTDIR}/../artifacts":/artifacts -v "${PROJECTDIR}/../ci_cd":/ci_cd docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION} /bin/sh -c "cd /src && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 RUSTFLAGS='-C link-arg=-lgcc -C link-arg=-lX11' cargo build --target ${RUST_TARGET_NOTSTATIC} --release --features=gui  --manifest-path /src/entrusted_client/Cargo.toml --bin entrusted-gui; ln -sf /usr/bin/gcc /usr/bin/aarch64-linux-musl-gcc && ln -sf /usr/bin/ar /usr/bin/musl-ar && ln -sf /usr/bin/ar /usr/bin/aarch64-linux-musl-ar; mkdir -p /tmp/appdir/usr/bin /tmp/appdir/usr/share/icons && cp /ci_cd/linux/xdg/* /tmp/appdir/ && cd /src && cp /src/entrusted_client/target/${RUST_TARGET_NOTSTATIC}/release/entrusted-gui /tmp/appdir/ && cp /src/images/Entrusted_icon.png /tmp/appdir/usr/share/icons/entrusted-gui.png && ARCH=${APPIMAGE_ARCH} linuxdeploy --appdir /tmp/appdir --desktop-file /tmp/appdir/entrusted-gui.desktop --icon-file /tmp/appdir/usr/share/icons/entrusted-gui.png --output appimage && mv *.AppImage /artifacts/entrusted-${APPVERSION}-linux-${CPU_ARCH}/entrusted-gui && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 CC_AARCH64_UNKNOWN_LINUX_MUSL=musl-gcc CXX_AARCH64_UNKNOWN_LINUX_MUSL=musl-g++ AR_AARCH64_UNKNOWN_LINUX_MUSL=ar CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc RUSTFLAGS='-C link-arg=-lgcc -C target-feature=+crt-static' cargo build --target ${RUST_TARGET_STATIC} --release --manifest-path /src/entrusted_webserver/Cargo.toml && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 CC_AARCH64_UNKNOWN_LINUX_MUSL=musl-gcc CXX_AARCH64_UNKNOWN_LINUX_MUSL=musl-g++ AR_AARCH64_UNKNOWN_LINUX_MUSL=ar CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc RUSTFLAGS='-C link-arg=-lgcc -C target-feature=+crt-static' cargo build --target ${RUST_TARGET_STATIC} --release --manifest-path /src/entrusted_webclient/Cargo.toml && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 CC_AARCH64_UNKNOWN_LINUX_MUSL=musl-gcc CXX_AARCH64_UNKNOWN_LINUX_MUSL=musl-g++ AR_AARCH64_UNKNOWN_LINUX_MUSL=ar CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc RUSTFLAGS='-C link-arg=-lgcc -C target-feature=+crt-static' cargo build --target ${RUST_TARGET_STATIC} --release --manifest-path /src/entrusted_client/Cargo.toml --bin entrusted-cli && cp /src/entrusted_webclient/target/${RUST_TARGET_STATIC}/release/entrusted-webclient /src/entrusted_webserver/target/${RUST_TARGET_STATIC}/release/entrusted-webserver /src/entrusted_client/target/${RUST_TARGET_STATIC}/release/entrusted-cli /artifacts/entrusted-${APPVERSION}-linux-${CPU_ARCH}/ && strip /src/entrusted_webclient/target/${RUST_TARGET_STATIC}/release/entrusted-webclient && strip /src/entrusted_webserver/target/${RUST_TARGET_STATIC}/release/entrusted-webserver && strip /src/entrusted_client/target/${RUST_TARGET_STATIC}/release/entrusted-cli"
        
        retVal=$?
        if [ $retVal -ne 0 ]; then
            echo "Failure to create ${CPU_ARCH} Linux binaries"
            exit 1
        fi        
    else
        podman run --platform linux/${DEB_ARCH} --rm --privileged -v "${PROJECTDIR}":/src -v "${PROJECTDIR}/../artifacts":/artifacts -v "${PROJECTDIR}/../ci_cd":/ci_cd docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION} /bin/sh -c "cd /src && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 cargo build --target ${RUST_TARGET_NOTSTATIC} --release --features=gui  --manifest-path /src/entrusted_client/Cargo.toml --bin entrusted-gui && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 cargo build --release --target ${RUST_TARGET_STATIC} --manifest-path /src/entrusted_client/Cargo.toml --bin entrusted-cli && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target ${RUST_TARGET_STATIC} --manifest-path /src/entrusted_webserver/Cargo.toml && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target ${RUST_TARGET_STATIC} --manifest-path /src/entrusted_webclient/Cargo.toml ; strip /src/entrusted_webclient/target/${RUST_TARGET_STATIC}/release/entrusted-webclient && strip /src/entrusted_webserver/target/${RUST_TARGET_STATIC}/release/entrusted-webserver && strip /src/entrusted_client/target/${RUST_TARGET_STATIC}/release/entrusted-cli; cp /src/entrusted_webclient/target/${RUST_TARGET_STATIC}/release/entrusted-webclient /src/entrusted_webserver/target/${RUST_TARGET_STATIC}/release/entrusted-webserver /src/entrusted_client/target/${RUST_TARGET_STATIC}/release/entrusted-cli /artifacts/entrusted-${APPVERSION}-linux-${CPU_ARCH}/ && mkdir -p /tmp/appdir/usr/bin /tmp/appdir/usr/share/icons && cp /ci_cd/linux/xdg/* /tmp/appdir/ && cd /src && cp /src/entrusted_client/target/${RUST_TARGET_NOTSTATIC}/release/entrusted-gui /tmp/appdir/ && cp /src/images/Entrusted_icon.png /tmp/appdir/usr/share/icons/entrusted-gui.png && ARCH=${APPIMAGE_ARCH} linuxdeploy --appdir /tmp/appdir --desktop-file /tmp/appdir/entrusted-gui.desktop --icon-file /tmp/appdir/usr/share/icons/entrusted-gui.png --output appimage && mv *.AppImage /artifacts/entrusted-${APPVERSION}-linux-${CPU_ARCH}/entrusted-gui"

        retVal=$?
        if [ $retVal -ne 0 ]; then
            echo "Failure to create Linux ${CPU_ARCH} Linux binaries"
            exit 1
        fi
    fi

    podman run --rm --platform linux/${DEB_ARCH} --volume "${PROJECTDIR}":/src  -v "${PROJECTDIR}/../artifacts":/artifacts  -v "${PROJECTDIR}/../ci_cd":/ci_cd docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION} /bin/sh -c "cd /src && apt update && /ci_cd/linux/redhat.sh ${APPVERSION} /artifacts/entrusted-${APPVERSION}-linux-${CPU_ARCH}.rpm /src/images /artifacts/entrusted-${APPVERSION}-linux-${CPU_ARCH} ${RPM_ARCH} && /ci_cd/linux/debian.sh ${APPVERSION} /artifacts/entrusted-${APPVERSION}-linux-${CPU_ARCH}.deb /src/images /artifacts/entrusted-${APPVERSION}-linux-${CPU_ARCH} ${DEB_ARCH}"
        
    retVal=$?
    if [ $retVal -ne 0 ]; then
        echo "Failure to create Linux RPM and Debian packages for ${CPU_ARCH}"
        exit 1
    fi
        
    cp ${SCRIPTDIR}/release_README.txt ${ARTIFACTSDIR}/README.txt

    cd ${ARTIFACTSDIR}/.. && tar cvf entrusted-${APPVERSION}-linux-${CPU_ARCH}.tar entrusted-${APPVERSION}-linux-${CPU_ARCH}
done

cd ${SCRIPTDIR}
