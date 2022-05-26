#!/usr/bin/env sh
set -x

DANGERZONE_VERSION=$1
ROOT_SCRIPTS_DIR="$(realpath $(dirname "$0"))"
echo "Cleanup previous build"
sudo rm -rf $HOME/LIVE_BOOT

echo "Install required packages"
sudo apt update && sudo apt install -y \
    fakeroot \
    debootstrap \
    squashfs-tools \
    dosfstools \
    xorriso \
    mg xz-utils \
    isolinux \
    fakeroot \
    sudo \
    bash \
    wget \
    syslinux-efi \
    grub-pc-bin \
    grub-efi-amd64-bin \
    systemd-container \
    bzip2 gzip \
    mtools

echo "Creating LIVE_BOOT folder"

mkdir -p $HOME/LIVE_BOOT

echo "Creating bootstrap environment for minimal Debian installation"
sudo debootstrap \
    --arch=amd64 \
    --variant=minbase \
    bullseye \
    $HOME/LIVE_BOOT/chroot \
    http://ftp.us.debian.org/debian/

echo "${DANGERZONE_VERSION}" > /tmp/dangerzone_release
sudo cp /tmp/dangerzone_release $HOME/LIVE_BOOT/chroot/etc/dangerzone_release

