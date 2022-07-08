#!/usr/bin/env sh
set -x

ENTRUSTED_VERSION=$1
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

echo "${ENTRUSTED_VERSION}" > /tmp/entrusted_release
sudo cp /tmp/entrusted_release $HOME/LIVE_BOOT/chroot/etc/entrusted_release

cp "${THIS_SCRIPTS_DIR}"/../../artifacts/entrusted-linux*/entrusted-cli /tmp/live-entrusted-cli
cp "${THIS_SCRIPTS_DIR}"/../../artifacts/entrusted-linux*/entrusted-webserver /tmp/live-entrusted-webserver

test -f /tmp/live-entrusted-container.tar && rm /tmp/live-entrusted-container.tar

podman build --squash -t "docker.io/uycyjnzgntrn/entrusted_container:${ENTRUSTED_VERSION}" "${THIS_SCRIPTS_DIR}/../../" -f "${THIS_SCRIPTS_DIR}/../../entrusted_container/Dockerfile"

retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Unable to build container image, please check for compilation errors!"
  exit 1
fi

podman image prune -f --filter label=stage=entrusted_container_builder

podman save -o /tmp/live-entrusted-container.tar "docker.io/uycyjnzgntrn/entrusted_container:${ENTRUSTED_VERSION}"

retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Unable to export container image to tar archive!"
  exit 1
fi
