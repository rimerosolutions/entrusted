#!/usr/bin/env sh
set -x

PREVIOUSDIR="$(echo $PWD)"
SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${SCRIPTDIR}/../..)"
APPVERSION=$(awk -F ' = ' '$1 ~ /version/ { gsub(/[\"]/, "", $2); printf("%s",$2) }' ${PROJECTDIR}/dangerzone_client/Cargo.toml)

DANGERZONE_VERSION="${APPVERSION}"

if [ -n "$1" ]; then  
  DANGERZONE_VERSION=$1
fi
ARTIFACTSDIR="${PROJECTDIR}/artifacts/dangerzone-livecd-amd64-${DANGERZONE_VERSION}"
echo "Building dangerzone version: ${DANGERZONE_VERSION}"

echo "Cleanup software components build folders"
rm -rf ${PROJECTDIR}/dangerzone_l10n/target
rm -rf ${PROJECTDIR}/dangerzone_container/target
rm -rf ${PROJECTDIR}/dangerzone_client/target
rm -rf ${PROJECTDIR}/dangerzone_httpclient/target
rm -rf ${PROJECTDIR}/dangerzone_httpserver/target

mkdir -p "${ARTIFACTSDIR}"

ROOT_SCRIPTS_DIR="$(realpath $(dirname "$0"))"
chmod +x "${ROOT_SCRIPTS_DIR}"/*.sh

"${ROOT_SCRIPTS_DIR}"/01-pre-chroot-script.sh "${DANGERZONE_VERSION}"
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

