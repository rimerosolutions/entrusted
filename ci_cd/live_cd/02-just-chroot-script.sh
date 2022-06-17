#!/usr/bin/env sh
set -x

ROOT_SCRIPTS_DIR="$(realpath $(dirname "$0"))"

sudo cp -rf "${ROOT_SCRIPTS_DIR}"/chroot_files $HOME/LIVE_BOOT/chroot/files
sudo cp -rf "${ROOT_SCRIPTS_DIR}"/03-in-chroot-script.sh $HOME/LIVE_BOOT/chroot/files/
sudo cp -rf "${ROOT_SCRIPTS_DIR}"/04-user-chroot-script.sh $HOME/LIVE_BOOT/chroot/files/
sudo mv /tmp/live-entrusted-container.tar $HOME/LIVE_BOOT/chroot/files/entrusted-container.tar
sudo mv /tmp/live-entrusted-cli $HOME/LIVE_BOOT/chroot/files/entrusted-cli
sudo mv /tmp/live-entrusted-webserver $HOME/LIVE_BOOT/chroot/files/entrusted-webserver

chmod +x $HOME/LIVE_BOOT/chroot/files/03-in-chroot-script.sh
chmod +x $HOME/LIVE_BOOT/chroot/files/04-user-chroot-script.sh

sudo systemd-nspawn -D $HOME/LIVE_BOOT/chroot /files/03-in-chroot-script.sh

