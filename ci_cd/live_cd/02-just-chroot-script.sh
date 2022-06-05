#!/usr/bin/env sh
set -x

ROOT_SCRIPTS_DIR="$(realpath $(dirname "$0"))"

sudo cp -rf "${ROOT_SCRIPTS_DIR}"/chroot_files $HOME/LIVE_BOOT/chroot/files
sudo cp -rf "${ROOT_SCRIPTS_DIR}"/03-in-chroot-script.sh $HOME/LIVE_BOOT/chroot/files/
sudo cp -rf "${ROOT_SCRIPTS_DIR}"/04-user-chroot-script.sh $HOME/LIVE_BOOT/chroot/files/
sudo mv /tmp/live-dangerzone-container.tar $HOME/LIVE_BOOT/chroot/files/dangerzone-container.tar
sudo mv /tmp/live-dangerzone-cli $HOME/LIVE_BOOT/chroot/files/dangerzone-cli
sudo mv /tmp/live-dangerzone-httpserver $HOME/LIVE_BOOT/chroot/files/dangerzone-httpserver

chmod +x $HOME/LIVE_BOOT/chroot/files/03-in-chroot-script.sh
chmod +x $HOME/LIVE_BOOT/chroot/files/04-user-chroot-script.sh

sudo systemd-nspawn -D $HOME/LIVE_BOOT/chroot /files/03-in-chroot-script.sh

