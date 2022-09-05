#!/usr/bin/env sh
DEBIAN_ARCH=$1
CPU_ARCH=$2

SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${SCRIPTDIR}/../..)"
ENTRUSTED_VERSION=$(cat $HOME/LIVE_BOOT-${DEBIAN_ARCH}/chroot/etc/entrusted_release | head -1)

mkdir -p "${PROJECTDIR}/artifacts"
cd $HOME

# See https://github.com/mayth/go-simple-upload-server
# curl -Ffile=@LIVE_BOOT/entrusted-livecd-${ENTRUSTED_VERSION}.iso http://localhost:25478/upload?token=entrusted_token

cp "${HOME}/LIVE_BOOT-${DEBIAN_ARCH}/entrusted-livecd-${CPU_ARCH}-${ENTRUSTED_VERSION}.iso" "${PROJECTDIR}/artifacts/entrusted-livecd-${CPU_ARCH}-${ENTRUSTED_VERSION}.iso"
