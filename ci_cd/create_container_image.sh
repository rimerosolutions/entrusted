#!/usr/bin/env sh
set -x

ROOT_SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${ROOT_SCRIPTDIR}/../app)"
APPVERSION=$(awk -F ' = ' '$1 ~ /version/ { gsub(/[\"]/, "", $2); printf("%s",$2) }' ${PROJECTDIR}/entrusted_client/Cargo.toml)

OLDIR=`pwd`

rm -rf  ${PROJECTDIR}/entrusted_client/target    \
        ${PROJECTDIR}/entrusted_webclient/target \
        ${PROJECTDIR}/entrusted_webserver/target \
        ${PROJECTDIR}/entrusted_container/target \
        ${PROJECTDIR}/entrusted_l10n/target

podman rmi --force docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}-amd64
podman rmi --force docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}-arm64
podman rmi --force docker.io/uycyjnzgntrn/entrusted_container:latest
podman rmi --force docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}

podman rm --force docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}-amd64
podman rm --force docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}-arm64
podman rm --force docker.io/uycyjnzgntrn/entrusted_container:latest
podman rm --force docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}

podman manifest rm docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}-amd64
podman manifest rm docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}-arm64
podman manifest rm docker.io/uycyjnzgntrn/entrusted_container:latest
podman manifest rm docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}

buildah manifest create docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}

cd "${PROJECTDIR}"

buildah bud --squash-all --platform=linux/amd64 --format docker -t docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}-amd64 -f entrusted_container/Dockerfile .
retVal=$?
if [ $retVal -ne 0 ]; then
    echo "Failure to create entrusted_container container image for amd64"
    exit 1
fi
buildah manifest add docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION} docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}-amd64


buildah bud --squash-all --platform=linux/arm64 --format docker -t docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}-arm64 -f entrusted_container/Dockerfile .
retVal=$?
if [ $retVal -ne 0 ]; then
    echo "Failure to create entrusted_container container image for arm64"
    exit 1
fi
buildah manifest add docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION} docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}-arm64

podman tag docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION} docker.io/uycyjnzgntrn/entrusted_container:latest

podman image prune -f --filter label=stage=entrusted_container_builder


cd "${OLD_DIR}"
