#!/usr/bin/env sh
set -x

DANGERZONE_VERSION=$1
THIS_SCRIPTS_DIR="$(realpath $(dirname "$0"))"
echo "Cleanup previous build"
test -d $HOME/LIVE_BOOT && sudo rm -rf $HOME/LIVE_BOOT

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
    https://mirror.csclub.uwaterloo.ca/debian/

echo "${DANGERZONE_VERSION}" > /tmp/dangerzone_release
sudo cp /tmp/dangerzone_release $HOME/LIVE_BOOT/chroot/etc/dangerzone_release

cp "${THIS_SCRIPTS_DIR}"/../../artifacts/dangerzone-linux*/dangerzone-cli /tmp/live-dangerzone-cli
cp "${THIS_SCRIPTS_DIR}"/../../artifacts/dangerzone-linux*/dangerzone-httpserver /tmp/live-dangerzone-httpserver

test -f /tmp/live-dangerzone-container.tar && rm /tmp/live-dangerzone-container.tar

podman build -t "docker.io/uycyjnzgntrn/dangerzone-converter:${DANGERZONE_VERSION}" "${THIS_SCRIPTS_DIR}/../../" -f "${THIS_SCRIPTS_DIR}/../../dangerzone_container/Dockerfile"
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Unable to build container image, please check for compilation errors!"
  exit 1
fi

podman save -o /tmp/live-dangerzone-container.tar "docker.io/uycyjnzgntrn/dangerzone-converter:${DANGERZONE_VERSION}"

retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Unable to export container image to tar archive!"
  exit 1
fi
