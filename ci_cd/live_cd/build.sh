#!/usr/bin/env sh
set -x

PREVIOUSDIR="$(echo $PWD)"
SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${SCRIPTDIR}/../..)"
APPVERSION=$(awk -F ' = ' '$1 ~ /version/ { gsub(/[\"]/, "", $2); printf("%s",$2) }' ${PROJECTDIR}/entrusted_client/Cargo.toml)

ENTRUSTED_VERSION="${APPVERSION}"

if [ -n "$1" ]; then  
  ENTRUSTED_VERSION=$1
fi
ARTIFACTSDIR="${PROJECTDIR}/artifacts/entrusted-livecd-amd64-${ENTRUSTED_VERSION}"
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

"${ROOT_SCRIPTS_DIR}"/01-pre-chroot-script.sh "${ENTRUSTED_VERSION}"
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failed to prepare build system"
  exit 1
fi

"${ROOT_SCRIPTS_DIR}"/02-just-chroot-script.sh
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failed to build system"
  exit 1
fi

"${ROOT_SCRIPTS_DIR}"/05-post-chroot-script.sh
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failed to create ISO image"
  exit 1
fi

"${ROOT_SCRIPTS_DIR}"/06-upload-iso-image.sh
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failed to upload iso image"
  exit 1
fi

