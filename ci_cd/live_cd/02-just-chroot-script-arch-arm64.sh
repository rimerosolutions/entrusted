#!/usr/bin/env sh
set -x
DEBIAN_ARCH=$1
sudo mv /tmp/entrusted-armpackaging $HOME/LIVE_BOOT-${DEBIAN_ARCH}/chroot/files/entrusted-armpackaging
