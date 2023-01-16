#!/usr/bin/env sh
set -x

PREVIOUSDIR="$(echo $PWD)"
SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${SCRIPTDIR}/../../app)"
APPVERSION=$(grep "^version" ${PROJECTDIR}/entrusted_client/Cargo.toml  | cut -d"=" -f2 | xargs)
CPU_ARCHS="amd64 aarch64"
ENTRUSTED_VERSION="${APPVERSION}"
CONTAINER_USER_NAME="entrusted"
CONTAINER_USER_ID="1024"

echo "Install required packages"
sudo apt update && sudo apt install -y \
    debootstrap \
    squashfs-tools \
    dosfstools \
    xorriso \
    grub-efi-amd64-bin \
    grub-pc-bin \
    fakeroot \
    sudo \
    bash \
    wget \
    systemd-container \
    bzip2 \
    gzip \
    mtools

for CPU_ARCH in $CPU_ARCHS ; do
    sudo killall -u "${CONTAINER_USER_NAME}" || true
    sudo userdel -r "${CONTAINER_USER_NAME}" || true
    sudo test -d "/home/${CONTAINER_USER_NAME}" && sudo rm -rf "/home/${CONTAINER_USER_NAME}"
    sudo test -d "/run/user/${CONTAINER_USER_ID}" && sudo rm -rf "/run/user/${CONTAINER_USER_ID}"
    
    MY_TMPDIR="${TMPDIR}"
    test -d "${MY_TMPDIR}" || MY_TMPDIR="/tmp"
    ENTRUSTED_ROOT_TMPDIR="${MY_TMPDIR}/entrusted-tmpbuild"
    LIVE_BOOT_DIR="${ENTRUSTED_ROOT_TMPDIR}/entrusted-livecd/live_boot_dir-${CPU_ARCH}-${ENTRUSTED_VERSION}"
    LIVE_BOOT_TMP_DIR="${ENTRUSTED_ROOT_TMPDIR}/entrusted-livecd/live_boot_tmpdir-${CPU_ARCH}-${ENTRUSTED_VERSION}"
    LINUX_ARTIFACTSDIR="${ENTRUSTED_ROOT_TMPDIR}/entrusted-livecd/linux_artifacts-${CPU_ARCH}-${ENTRUSTED_VERSION}"
    LIVE_ISO_DIR="${PROJECTDIR}/../artifacts"
    DEBIAN_ARCH="amd64"
    UNAME_ARCH="x86_64"
    RUST_MUSL_TARGET="x86_64-unknown-linux-musl"
    RUST_PREAMBLE="RUSTFLAGS='-C target-feature=+crt-static'"
    test -d "${ENTRUSTED_ROOT_TMPDIR}" && sudo rm -rf "${ENTRUSTED_ROOT_TMPDIR}"
    mkdir -p "${ENTRUSTED_ROOT_TMPDIR}" "${LIVE_BOOT_TMP_DIR}" "${LIVE_ISO_DIR}" "${LIVE_BOOT_DIR}" "${LINUX_ARTIFACTSDIR}"
    
    test -f "${LIVE_ISO_DIR}"/entrusted-livecd-${CPU_ARCH}-${ENTRUSTED_VERSION}.iso && rm "${LIVE_ISO_DIR}"/entrusted-livecd-${CPU_ARCH}-${ENTRUSTED_VERSION}.iso

    if [ ${CPU_ARCH} != "amd64" ]
    then
        DEBIAN_ARCH="arm64"
        UNAME_ARCH="aarch64"
        RUST_MUSL_TARGET="aarch64-unknown-linux-musl"
        RUST_PREAMBLE="CC_AARCH64_UNKNOWN_LINUX_MUSL=musl-gcc CXX_AARCH64_UNKNOWN_LINUX_MUSL=musl-g++ AR_AARCH64_UNKNOWN_LINUX_MUSL=ar CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc RUSTFLAGS='-C link-arg=-lgcc -C target-feature=+crt-static'"
    fi        
    
    echo "Building entrusted version: ${ENTRUSTED_VERSION}"

    echo "Cleanup software components build folders"
    test -d ${PROJECTDIR}/entrusted_l10n/target      && rm -rf ${PROJECTDIR}/entrusted_l10n/target
    test -d ${PROJECTDIR}/entrusted_container/target && rm -rf ${PROJECTDIR}/entrusted_container/target
    test -d ${PROJECTDIR}/entrusted_client/target    && rm -rf ${PROJECTDIR}/entrusted_client/target
    test -d ${PROJECTDIR}/entrusted_webclient/target && rm -rf ${PROJECTDIR}/entrusted_webclient/target
    test -d ${PROJECTDIR}/entrusted_webserver/target && rm -rf ${PROJECTDIR}/entrusted_webserver/target    
    
    podman run --log-driver=none --platform linux/${DEBIAN_ARCH} -v "${PROJECTDIR}/..":/src -v "${LINUX_ARTIFACTSDIR}":/artifacts docker.io/uycyjnzgntrn/rust-linux:1.64.0 /bin/sh -c "CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 ${RUST_PREAMBLE} cargo build --release --target ${RUST_MUSL_TARGET} --manifest-path /src/app/entrusted_webserver/Cargo.toml && cp /src/app/entrusted_webserver/target/${RUST_MUSL_TARGET}/release/entrusted-webserver /artifacts/ && rm -rf /src/app/entrusted_webserver/target && CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_NET_RETRY=10 ${RUST_PREAMBLE} cargo build --release --target ${RUST_MUSL_TARGET} --manifest-path /src/app/entrusted_client/Cargo.toml && cp /src/app/entrusted_client/target/${RUST_MUSL_TARGET}/release/entrusted-cli /artifacts/ && rm -rf /src/app/entrusted_client/target && strip /artifacts/entrusted-cli && strip /artifacts/entrusted-webserver"
    retVal=$?
    if [ "$retVal" != "0" ]; then
        echo "Could not build entrusted-cli and entrusted-webserver" && exit 1
    fi

    ROOT_SCRIPTS_DIR="$(realpath $(dirname "$0"))"
    chmod +x "${ROOT_SCRIPTS_DIR}"/*.sh

    "${ROOT_SCRIPTS_DIR}"/01-pre-chroot.sh "${ENTRUSTED_VERSION}" \
                         "${UNAME_ARCH}" \
                         "${DEBIAN_ARCH}" \
                         "${DEBIAN_ARCH}" \
                         "${LINUX_ARTIFACTSDIR}" \
                         "${LIVE_BOOT_DIR}" \
                         "${LIVE_BOOT_TMP_DIR}" \
                         "${CONTAINER_USER_NAME}" \
                         "${CONTAINER_USER_ID}"
    retVal=$?
    if [ "$retVal" != "0" ]; then
        echo "Failed to prepare build for ${CPU_ARCH}" && exit 1
    fi

    "${ROOT_SCRIPTS_DIR}"/03-post-chroot.sh "${DEBIAN_ARCH}" "${LIVE_BOOT_DIR}" "${LIVE_ISO_DIR}"
    retVal=$?
    if [ "$retVal" != "0" ]; then
        echo "Failed to create ISO image for ${CPU_ARCH}" && exit 1
    fi
    
    sudo rm -rf "${ENTRUSTED_ROOT_TMPDIR}"   || true
    sudo killall -u "${CONTAINER_USER_NAME}" || true
    sudo userdel -r "${CONTAINER_USER_NAME}" || true    
    sudo test -d "/home/${CONTAINER_USER_NAME}"   && sudo rm -rf "/home/${CONTAINER_USER_NAME}"
    sudo test -d "/run/user/${CONTAINER_USER_ID}" && sudo rm -rf "/run/user/${CONTAINER_USER_ID}"
done
