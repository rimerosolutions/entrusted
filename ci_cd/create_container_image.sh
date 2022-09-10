#!/usr/bin/env sh
set -x

ROOT_SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${ROOT_SCRIPTDIR}/..)"
APPVERSION=$(awk -F ' = ' '$1 ~ /version/ { gsub(/[\"]/, "", $2); printf("%s",$2) }' ${PROJECTDIR}/entrusted_client/Cargo.toml)

OLDIR=`pwd`

rm -rf  ${PROJECTDIR}/entrusted_client/target    \
        ${PROJECTDIR}/entrusted_webclient/target \
        ${PROJECTDIR}/entrusted_webserver/target \
        ${PROJECTDIR}/entrusted_container/target \
        ${PROJECTDIR}/entrusted_l10n/target

podman rmi --force docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}-amd64
podman rmi --force docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}-arm64
podman rmi --force docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}

buildah manifest create docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}

cd "${PROJECTDIR}"

buildah bud --squash --platform=linux/amd64 --format docker -t docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}-amd64 -f entrusted_container/Dockerfile .
buildah manifest add docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION} docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}-amd64

buildah bud --squash --platform=linux/arm64/v8 --format docker -t docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}-arm64 -f entrusted_container/Dockerfile .
buildah manifest add docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION} docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}-arm64

podman tag docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION} docker.io/uycyjnzgntrn/entrusted_container:latest

cd "${OLD_DIR}"
