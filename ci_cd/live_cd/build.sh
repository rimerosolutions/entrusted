#!/usr/bin/env sh
set -x

PREVIOUSDIR="$(echo $PWD)"
SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${SCRIPTDIR}/../..)"
APPVERSION=$(awk -F ' = ' '$1 ~ /version/ { gsub(/[\"]/, "", $2); printf("%s",$2) }' ${PROJECTDIR}/entrusted_client/Cargo.toml)
CPU_ARCHS="amd64 aarch64"
ENTRUSTED_VERSION="${APPVERSION}"

if [ -n "$1" ]; then
    ENTRUSTED_VERSION=$1
fi

for CPU_ARCH in $CPU_ARCHS ; do
    ARTIFACTSDIR="${PROJECTDIR}/artifacts/entrusted-livecd-${CPU_ARCH}-${ENTRUSTED_VERSION}"
    LINUX_ARTIFACTSDIR="${PROJECTDIR}/artifacts/entrusted-linux-${CPU_ARCH}-${ENTRUSTED_VERSION}"
    DEBIAN_ARCH="amd64"

    if [ ${CPU_ARCH} != "amd64" ]
    then
        DEBIAN_ARCH="arm64"
    fi
    
    echo "Building entrusted version: ${ENTRUSTED_VERSION}"

    echo "Cleanup software components build folders"
    rm -rf ${PROJECTDIR}/entrusted_l10n/target
    rm -rf ${PROJECTDIR}/entrusted_container/target
    rm -rf ${PROJECTDIR}/entrusted_client/target
    rm -rf ${PROJECTDIR}/entrusted_webclient/target
    rm -rf ${PROJECTDIR}/entrusted_webserver/target

    mkdir -p "${ARTIFACTSDIR}"

    ROOT_SCRIPTS_DIR="$(realpath $(dirname "$0"))"
    chmod +x "${ROOT_SCRIPTS_DIR}"/*.sh

    "${ROOT_SCRIPTS_DIR}"/01-pre-chroot-script.sh "${ENTRUSTED_VERSION}" "${DEBIAN_ARCH}" "${LINUX_ARTIFACTSDIR}"
    retVal=$?
    if [ $retVal -ne 0 ]; then
        echo "Failed to prepare build system for ${CPU_ARCH}"
        exit 1
    fi

    "${ROOT_SCRIPTS_DIR}"/02-just-chroot-script.sh "${DEBIAN_ARCH}"
    retVal=$?
    if [ $retVal -ne 0 ]; then
        echo "Failed to build system for ${CPU_ARCH}"
        exit 1
    fi

    "${ROOT_SCRIPTS_DIR}"/05-post-chroot-script.sh "${DEBIAN_ARCH}"
    retVal=$?
    if [ $retVal -ne 0 ]; then
        echo "Failed to create ISO image for ${CPU_ARCH}"
        exit 1
    fi

    "${ROOT_SCRIPTS_DIR}"/06-upload-iso-image.sh "${DEBIAN_ARCH}" "${CPU_ARCH}"
    retVal=$?
    if [ $retVal -ne 0 ]; then
        echo "Failed to upload iso image for ${CPU_ARCH}"
        exit 1
    fi
done
