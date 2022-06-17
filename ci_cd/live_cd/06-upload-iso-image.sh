#!/usr/bin/env sh
SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${SCRIPTDIR}/../..)"
ENTRUSTED_VERSION=$(cat $HOME/LIVE_BOOT/chroot/etc/entrusted_release | head -1)

mkdir -p "${PROJECTDIR}/artifacts"
cd $HOME

# See https://github.com/mayth/go-simple-upload-server
# curl -Ffile=@LIVE_BOOT/entrusted-livecd-${ENTRUSTED_VERSION}.iso http://localhost:25478/upload?token=entrusted_token

cp "${HOME}/LIVE_BOOT/entrusted-livecd-amd64-${ENTRUSTED_VERSION}.iso" "${PROJECTDIR}/artifacts/"
