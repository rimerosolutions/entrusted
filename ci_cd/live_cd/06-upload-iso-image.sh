#!/usr/bin/env sh
SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${SCRIPTDIR}/../..)"
DANGERZONE_VERSION=$(cat $HOME/LIVE_BOOT/chroot/etc/dangerzone_release | head -1)

mkdir -p "${PROJECTDIR}/artifacts"
cd $HOME

# See https://github.com/mayth/go-simple-upload-server
# curl -Ffile=@LIVE_BOOT/dangerzone-livecd-${DANGERZONE_VERSION}.iso http://localhost:25478/upload?token=dangerzone_token

cp "${HOME}/LIVE_BOOT/dangerzone-livecd-amd64-${DANGERZONE_VERSION}.iso" "${PROJECTDIR}/artifacts/"
