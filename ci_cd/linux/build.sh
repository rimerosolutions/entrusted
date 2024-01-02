#!/usr/bin/env sh
set -x

PREVIOUSDIR="$(echo $PWD)"
SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${SCRIPTDIR}/../../app)"
APPVERSION=$(grep "^version" ${PROJECTDIR}/entrusted_client/Cargo.toml  | cut -d"=" -f2 | xargs)
CPU_ARCHS="amd64 aarch64"
LIBC_FLAVORS="glibc musl"
RUST_CI_VERSION="1.72.0"
ALPINE_VERSION="3.18.3"
CFLTK_VERSION="1.4.21"

for CPU_ARCH in ${CPU_ARCHS} ; do
    for LIBC_FLAVOR in ${LIBC_FLAVORS} ; do
        ARTIFACTSDIR="${PROJECTDIR}/../artifacts/entrusted-${APPVERSION}-linux-${LIBC_FLAVOR}-${CPU_ARCH}"
        test -d ${ARTIFACTSDIR} && rm -rf ${ARTIFACTSDIR}
        mkdir -p ${ARTIFACTSDIR}
    done
done

for CPU_ARCH in $CPU_ARCHS ; do    
    RUST_TARGET_STATIC="x86_64-unknown-linux-musl"
    RUST_TARGET_NOTSTATIC="x86_64-unknown-linux-gnu"
    DEB_ARCH="amd64"
    RPM_ARCH="x86_64"
    CFLTK_BUNDLE_URL="https://github.com/yveszoundi/cfltk-alpine-musl-bundle/releases/download/${CFLTK_VERSION}/lib_x86_64-alpine-linux-musl.tar.gz"

    if [ ${CPU_ARCH} != "amd64" ]
    then
        RUST_TARGET_STATIC="aarch64-unknown-linux-musl"
        RUST_TARGET_NOTSTATIC="aarch64-unknown-linux-gnu"
        DEB_ARCH="arm64"
        RPM_ARCH="aarch64"
        CFLTK_BUNDLE_URL="https://github.com/yveszoundi/cfltk-alpine-musl-bundle/releases/download/${CFLTK_VERSION}/lib_aarch64-alpine-linux-musl.tar.gz"
    fi

    test -d ${PROJECTDIR}/entrusted_container/target && rm -rf ${PROJECTDIR}/entrusted_container/target
    test -d ${PROJECTDIR}/entrusted_client/target    && rm -rf ${PROJECTDIR}/entrusted_client/target
    test -d ${PROJECTDIR}/entrusted_webclient/target && rm -rf ${PROJECTDIR}/entrusted_webclient/target
    test -d ${PROJECTDIR}/entrusted_webserver/target && rm -rf ${PROJECTDIR}/entrusted_webserver/target            

    cd ${PROJECTDIR}

    echo "Building entrusted_client (entrusted-gui) for ${CPU_ARCH}"

    # GLIBC
    if [ ${CPU_ARCH} != "amd64" ]
    then                      
        podman run --rm --platform linux/${DEB_ARCH} -v "${PROJECTDIR}":/src  -v "${PROJECTDIR}/../artifacts":/artifacts -v "${PROJECTDIR}/../ci_cd":/ci_cd docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION} /bin/sh -c "CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 RUSTFLAGS='-C link-arg=-lgcc -C link-arg=-lX11' cargo build --target ${RUST_TARGET_NOTSTATIC} --release --features=gui,fltk/use-wayland,fltk/fltk-bundled  --manifest-path /src/entrusted_client/Cargo.toml --bin entrusted-gui && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 CC_AARCH64_UNKNOWN_LINUX_MUSL=musl-gcc CXX_AARCH64_UNKNOWN_LINUX_MUSL=musl-g++ AR_AARCH64_UNKNOWN_LINUX_MUSL=ar CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc RUSTFLAGS='-C link-arg=-lgcc -C target-feature=+crt-static' cargo build --target ${RUST_TARGET_STATIC} --release --manifest-path /src/entrusted_webserver/Cargo.toml && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 CC_AARCH64_UNKNOWN_LINUX_MUSL=musl-gcc CXX_AARCH64_UNKNOWN_LINUX_MUSL=musl-g++ AR_AARCH64_UNKNOWN_LINUX_MUSL=ar CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc RUSTFLAGS='-C link-arg=-lgcc -C target-feature=+crt-static' cargo build --target ${RUST_TARGET_STATIC} --release --manifest-path /src/entrusted_webclient/Cargo.toml && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 CC_AARCH64_UNKNOWN_LINUX_MUSL=musl-gcc CXX_AARCH64_UNKNOWN_LINUX_MUSL=musl-g++ AR_AARCH64_UNKNOWN_LINUX_MUSL=ar CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc RUSTFLAGS='-C link-arg=-lgcc -C target-feature=+crt-static' cargo build --target ${RUST_TARGET_STATIC} --release --manifest-path /src/entrusted_client/Cargo.toml --bin entrusted-cli && cp /src/entrusted_webclient/target/${RUST_TARGET_STATIC}/release/entrusted-webclient /src/entrusted_webserver/target/${RUST_TARGET_STATIC}/release/entrusted-webserver /src/entrusted_client/target/${RUST_TARGET_NOTSTATIC}/release/entrusted-gui /src/entrusted_client/target/${RUST_TARGET_STATIC}/release/entrusted-cli /artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}/ && strip /artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}/entrusted-gui && strip /artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}-glibc/entrusted-cli && strip /artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}/entrusted-webserver && strip /artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}/entrusted-webclient" || (sleep 10 && podman run --rm --platform linux/${DEB_ARCH} -v "${PROJECTDIR}":/src  -v "${PROJECTDIR}/../artifacts":/artifacts -v "${PROJECTDIR}/../ci_cd":/ci_cd docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION} /bin/sh -c "CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 RUSTFLAGS='-C link-arg=-lgcc -C link-arg=-lX11' cargo build --target ${RUST_TARGET_NOTSTATIC} --release --features=gui,fltk/use-wayland,fltk/fltk-bundled  --manifest-path /src/entrusted_client/Cargo.toml --bin entrusted-gui && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 CC_AARCH64_UNKNOWN_LINUX_MUSL=musl-gcc CXX_AARCH64_UNKNOWN_LINUX_MUSL=musl-g++ AR_AARCH64_UNKNOWN_LINUX_MUSL=ar CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc RUSTFLAGS='-C link-arg=-lgcc -C target-feature=+crt-static' cargo build --target ${RUST_TARGET_STATIC} --release --manifest-path /src/entrusted_webserver/Cargo.toml && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 CC_AARCH64_UNKNOWN_LINUX_MUSL=musl-gcc CXX_AARCH64_UNKNOWN_LINUX_MUSL=musl-g++ AR_AARCH64_UNKNOWN_LINUX_MUSL=ar CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc RUSTFLAGS='-C link-arg=-lgcc -C target-feature=+crt-static' cargo build --target ${RUST_TARGET_STATIC} --release --manifest-path /src/entrusted_webclient/Cargo.toml && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 CC_AARCH64_UNKNOWN_LINUX_MUSL=musl-gcc CXX_AARCH64_UNKNOWN_LINUX_MUSL=musl-g++ AR_AARCH64_UNKNOWN_LINUX_MUSL=ar CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc RUSTFLAGS='-C link-arg=-lgcc -C target-feature=+crt-static' cargo build --target ${RUST_TARGET_STATIC} --release --manifest-path /src/entrusted_client/Cargo.toml --bin entrusted-cli && cp /src/entrusted_webclient/target/${RUST_TARGET_STATIC}/release/entrusted-webclient /src/entrusted_webserver/target/${RUST_TARGET_STATIC}/release/entrusted-webserver /src/entrusted_client/target/${RUST_TARGET_NOTSTATIC}/release/entrusted-gui /src/entrusted_client/target/${RUST_TARGET_STATIC}/release/entrusted-cli /artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}/ && strip /artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}/entrusted-gui && strip /artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}/entrusted-cli && strip /artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}/entrusted-webserver && strip /artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}/entrusted-webclient")
        
        retVal=$?
        if [ $retVal -ne 0 ]; then
            echo "Failure to create ${CPU_ARCH} Linux binaries"
            exit 1
        fi        
    else
        podman run --platform linux/${DEB_ARCH} --rm -v "${PROJECTDIR}":/src -v "${PROJECTDIR}/../artifacts":/artifacts -v "${PROJECTDIR}/../ci_cd":/ci_cd docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION} /bin/sh -c "CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 cargo build --target ${RUST_TARGET_NOTSTATIC} --release --features=gui,fltk/use-wayland,fltk/fltk-bundled  --manifest-path /src/entrusted_client/Cargo.toml --bin entrusted-gui && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 cargo build --release --target ${RUST_TARGET_STATIC} --manifest-path /src/entrusted_client/Cargo.toml --bin entrusted-cli && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target ${RUST_TARGET_STATIC} --manifest-path /src/entrusted_webserver/Cargo.toml && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target ${RUST_TARGET_STATIC} --manifest-path /src/entrusted_webclient/Cargo.toml && cp /src/entrusted_webclient/target/${RUST_TARGET_STATIC}/release/entrusted-webclient /src/entrusted_webserver/target/${RUST_TARGET_STATIC}/release/entrusted-webserver /src/entrusted_client/target/${RUST_TARGET_NOTSTATIC}/release/entrusted-gui /src/entrusted_client/target/${RUST_TARGET_STATIC}/release/entrusted-cli /artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}/ && strip /artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}/entrusted-gui && strip /artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}/entrusted-cli && strip /artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}/entrusted-webserver && strip /artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}/entrusted-webclient" || (sleep 10 && podman run --platform linux/${DEB_ARCH} --rm -v "${PROJECTDIR}":/src -v "${PROJECTDIR}/../artifacts":/artifacts -v "${PROJECTDIR}/../ci_cd":/ci_cd docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION} /bin/sh -c "CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 cargo build --target ${RUST_TARGET_NOTSTATIC} --release --features=gui,fltk/use-wayland,fltk/fltk-bundled  --manifest-path /src/entrusted_client/Cargo.toml --bin entrusted-gui && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 cargo build --release --target ${RUST_TARGET_STATIC} --manifest-path /src/entrusted_client/Cargo.toml --bin entrusted-cli && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target ${RUST_TARGET_STATIC} --manifest-path /src/entrusted_webserver/Cargo.toml && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target ${RUST_TARGET_STATIC} --manifest-path /src/entrusted_webclient/Cargo.toml && cp /src/entrusted_webclient/target/${RUST_TARGET_STATIC}/release/entrusted-webclient /src/entrusted_webserver/target/${RUST_TARGET_STATIC}/release/entrusted-webserver /src/entrusted_client/target/${RUST_TARGET_NOTSTATIC}/release/entrusted-gui /src/entrusted_client/target/${RUST_TARGET_STATIC}/release/entrusted-cli /artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}/ && strip /artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}/entrusted-gui && strip /artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}/entrusted-cli && strip /artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}/entrusted-webserver && strip /artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}/entrusted-webclient")

        retVal=$?
        if [ $retVal -ne 0 ]; then
            echo "Failure to create Linux ${CPU_ARCH} Linux binaries"
            exit 1
        fi
    fi
    
    # MUSL    
    cp "${PROJECTDIR}/../artifacts/entrusted-${APPVERSION}-linux-glibc-${CPU_ARCH}"/* "${PROJECTDIR}/../artifacts/entrusted-${APPVERSION}-linux-musl-${CPU_ARCH}/"
    rm "${PROJECTDIR}/../artifacts/entrusted-${APPVERSION}-linux-musl-${CPU_ARCH}/entrusted-gui"
    
    podman run --rm --platform linux/${DEB_ARCH} -v "${PROJECTDIR}":/src  -v "${PROJECTDIR}/../artifacts":/artifacts -v "${PROJECTDIR}/../ci_cd":/ci_cd docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION} /bin/sh -c "apk add rust cargo git cmake make g++ pango-dev fontconfig-dev libxinerama-dev libxfixes-dev libxcursor-dev libpng-dev cairo-dev librsvg-dev wayland-dev wayland-protocols wayland-libs-client wayland-libs-cursor wayland-libs-egl libxkbcommon-dev zlib-dev wayland-libs-egl mesa-egl mesa-dev dbus-dev curl && cargo clean --manifest-path /src/entrusted_client/Cargo.toml && CFLTK_BUNDLE_URL='${CFLTK_BUNDLE_URL}' cargo build --release --features=gui,fltk/use-wayland,fltk/fltk-bundled --manifest-path /src/entrusted_client/Cargo.toml --bin entrusted-gui && cp /src/entrusted_client/target/release/entrusted-gui /artifacts/entrusted-${APPVERSION}-linux-musl-${CPU_ARCH}/ && strip /artifacts/entrusted-${APPVERSION}-linux-musl-${CPU_ARCH}/entrusted-gui && cargo clean --manifest-path /src/entrusted_client/Cargo.toml" || (sleep 10 && podman run --rm --platform linux/${DEB_ARCH} -v "${PROJECTDIR}":/src  -v "${PROJECTDIR}/../artifacts":/artifacts -v "${PROJECTDIR}/../ci_cd":/ci_cd docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION} /bin/sh -c "apk add rust cargo git cmake make g++ pango-dev fontconfig-dev libxinerama-dev libxfixes-dev libxcursor-dev libpng-dev cairo-dev librsvg-dev wayland-dev wayland-protocols wayland-libs-client wayland-libs-cursor wayland-libs-egl libxkbcommon-dev zlib-dev wayland-libs-egl mesa-egl mesa-dev dbus-dev curl && cargo clean --manifest-path /src/entrusted_client/Cargo.toml && CFLTK_BUNDLE_URL='${CFLTK_BUNDLE_URL}' cargo build --release --features=gui,fltk/use-wayland,fltk/fltk-bundled --manifest-path /src/entrusted_client/Cargo.toml --bin entrusted-gui && cp /src/entrusted_client/target/release/entrusted-gui /artifacts/entrusted-${APPVERSION}-linux-musl-${CPU_ARCH}/ && strip /artifacts/entrusted-${APPVERSION}-linux-musl-${CPU_ARCH}/entrusted-gui && cargo clean --manifest-path /src/entrusted_client/Cargo.toml")

    # Practically speaking most RPM or Debian distributions use GLIBC instead of MUSL, so we only care about GLIBC RPM or DEB artifacts
    for LIBC_FLAVOR in ${LIBC_FLAVORS} ; do
        podman run --rm --platform linux/${DEB_ARCH} -v "${PROJECTDIR}":/src  -v "${PROJECTDIR}/../artifacts":/artifacts  -v "${PROJECTDIR}/../ci_cd":/ci_cd docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION} /bin/sh -c "cd /src && apt update && /ci_cd/linux/redhat.sh ${APPVERSION} /artifacts/entrusted-${APPVERSION}-linux-${LIBC_FLAVOR}-${CPU_ARCH}.rpm /src/images /artifacts/entrusted-${APPVERSION}-linux-${LIBC_FLAVOR}-${CPU_ARCH} ${RPM_ARCH} && /ci_cd/linux/debian.sh ${APPVERSION} /artifacts/entrusted-${APPVERSION}-linux-${LIBC_FLAVOR}-${CPU_ARCH}.deb /src/images /artifacts/entrusted-${APPVERSION}-linux-${LIBC_FLAVOR}-${CPU_ARCH} ${DEB_ARCH}" || (sleep 10 && podman run --rm --platform linux/${DEB_ARCH} -v "${PROJECTDIR}":/src  -v "${PROJECTDIR}/../artifacts":/artifacts  -v "${PROJECTDIR}/../ci_cd":/ci_cd docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION} /bin/sh -c "cd /src && apt update && /ci_cd/linux/redhat.sh ${APPVERSION} /artifacts/entrusted-${APPVERSION}-linux-${LIBC_FLAVOR}-${CPU_ARCH}.rpm /src/images /artifacts/entrusted-${APPVERSION}-linux-${LIBC_FLAVOR}-${CPU_ARCH} ${RPM_ARCH} && /ci_cd/linux/debian.sh ${APPVERSION} /artifacts/entrusted-${APPVERSION}-linux-${LIBC_FLAVOR}-${CPU_ARCH}.deb /src/images /artifacts/entrusted-${APPVERSION}-linux-${LIBC_FLAVOR}-${CPU_ARCH} ${DEB_ARCH}")
        
        retVal=$?
        if [ $retVal -ne 0 ]; then
            echo "Failure to create Linux RPM and Debian packages for ${CPU_ARCH} and libc flavor ${LIBC_FLAVOR}"
            exit 1
        fi
        
        cp ${SCRIPTDIR}/release_README.txt ${PROJECTDIR}/../artifacts/entrusted-${APPVERSION}-linux-${LIBC_FLAVOR}-${CPU_ARCH}/README.txt
        cd ${PROJECTDIR}/../artifacts && tar cvf entrusted-${APPVERSION}-linux-${LIBC_FLAVOR}-${CPU_ARCH}.tar entrusted-${APPVERSION}-linux-${LIBC_FLAVOR}-${CPU_ARCH}    
    done
done

cd ${SCRIPTDIR}
