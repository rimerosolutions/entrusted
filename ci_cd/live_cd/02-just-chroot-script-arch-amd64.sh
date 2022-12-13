#!/usr/bin/env sh
set -x
DEBIAN_ARCH=$1
sudo mv /tmp/live-entrusted-container.tar $HOME/LIVE_BOOT-${DEBIAN_ARCH}/chroot/files/entrusted-container.tar
