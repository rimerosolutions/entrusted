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

podman rmi --force docker.io/uycyjnzgntrn/entrusted_container:latest
podman rmi --force docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}
podman manifest rm docker.io/uycyjnzgntrn/entrusted_container:latest
podman manifest rm docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION}

cd "${PROJECTDIR}"

podman build --log-level info --jobs 2 --squash-all --format docker  --platform linux/arm64/v8 --platform linux/amd64 --manifest docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION} -f entrusted_container/Dockerfile .

retVal=$?
if [ $retVal -ne 0 ]; then
    echo "Failure to create entrusted_container container image"
    exit 1
fi

podman tag docker.io/uycyjnzgntrn/entrusted_container:${APPVERSION} docker.io/uycyjnzgntrn/entrusted_container:latest

podman image prune -f --filter label=stage=entrusted_container_builder

cd "${OLD_DIR}"
