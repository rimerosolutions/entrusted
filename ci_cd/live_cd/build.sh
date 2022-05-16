#!/usr/bin/env bash

DANGERZONE_VERSION=0.0.2

if [ -n "$1" ]; then  
  DANGERZONE_VERSION=$1
fi

echo "Using custom dangerzone version ${DANGERZONE_VERSION}"

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
