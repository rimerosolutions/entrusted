#!/usr/bin/env sh
set -x

ROOT_SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${ROOT_SCRIPTDIR}/..)"
APPVERSION=$(awk -F ' = ' '$1 ~ /version/ { gsub(/[\"]/, "", $2); printf("%s",$2) }' ${PROJECTDIR}/entrusted_client/Cargo.toml)
PLATFORM_ARCHS="linux/amd64 linux/arm64/v8"
CPU_ARCHS="amd64 arm64"

OLDIR=`pwd`

rm -rf  ${PROJECTDIR}/entrusted_client/target    \
        ${PROJECTDIR}/entrusted_webclient/target \
        ${PROJECTDIR}/entrusted_webserver/target \
        ${PROJECTDIR}/entrusted_container/target \
        ${PROJECTDIR}/entrusted_l10n/target

for CPU_ARCH in $CPU_ARCHS ; do
    podman rmi --force docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}-${CPU_ARCH}
done

podman rmi --force docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}

buildah manifest create docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}

cd "${PROJECTDIR}"

for PLATFORM_ARCH in $PLATFORM_ARCHS ; do
    CPU_ARCH="amd64"

    if [ "${PLATFORM_ARCH}" != "linux/amd64" ]
    then
        CPU_ARCH="arm64"
    fi

    buildah bud --squash --platform=${PLATFORM_ARCH} --format docker -t docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}-${CPU_ARCH} -f entrusted_container/Dockerfile .
    buildah manifest add docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION} docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}-${CPU_ARCH}
done

cd "${OLD_DIR}"
