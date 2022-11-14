#!/usr/bin/env sh
set -x

ENTRUSTED_VERSION=$1
DEBIAN_ARCH=$2
LINUX_ARTIFACTSDIR=$3
THIS_SCRIPTS_DIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${THIS_SCRIPTS_DIR}/../../app)"

echo "Cleanup previous build"
test -d $HOME/LIVE_BOOT-${DEBIAN_ARCH} && sudo rm -rf $HOME/LIVE_BOOT-${DEBIAN_ARCH}

echo "Install required packages"
sudo apt update && sudo apt install -y \
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

mkdir -p $HOME/LIVE_BOOT-${DEBIAN_ARCH}

echo "Creating bootstrap environment for minimal Debian installation"
sudo debootstrap \
    --arch=${DEBIAN_ARCH} \
    --variant=minbase \
    bullseye \
    $HOME/LIVE_BOOT-${DEBIAN_ARCH}/chroot \
    https://mirror.csclub.uwaterloo.ca/debian/

echo "${ENTRUSTED_VERSION}" > /tmp/entrusted_release
sudo cp /tmp/entrusted_release $HOME/LIVE_BOOT-${DEBIAN_ARCH}/chroot/etc/entrusted_release
rm /tmp/entrusted_release 

echo "${DEBIAN_ARCH}" > /tmp/entrusted_arch
sudo cp /tmp/entrusted_arch $HOME/LIVE_BOOT-${DEBIAN_ARCH}/chroot/etc/entrusted_arch
rm /tmp/entrusted_arch

cp "${LINUX_ARTIFACTSDIR}"/entrusted-cli /tmp/live-entrusted-cli && cp "${LINUX_ARTIFACTSDIR}"/entrusted-webserver /tmp/live-entrusted-webserver
test -f /tmp/live-entrusted-container.tar && rm /tmp/live-entrusted-container.tar

podman image prune -f --filter label=stage=entrusted_container_builder

podman save -m  -o /tmp/live-entrusted-container.tar "docker.io/uycyjnzgntrn/entrusted_container:${ENTRUSTED_VERSION}"

retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Unable to export container image to tar archive!"
  exit 1
fi
