#!/usr/bin/env bash

DANGERZONE_VERSION=$(cat $HOME/LIVE_BOOT/chroot/etc/dangerzone_release | head -1)

cd $HOME

# See https://github.com/mayth/go-simple-upload-server
# curl -Ffile=@LIVE_BOOT/dangerzone-livecd-${DANGERZONE_VERSION}.iso http://localhost:25478/upload?token=dangerzone_token

echo "I refuse to upload the live CD image at ${HOME}/LIVE_BOOT/dangerzone-livecd-${DANGERZONE_VERSION}.iso"
