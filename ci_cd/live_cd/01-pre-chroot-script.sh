#!/usr/bin/env sh
set -x

ENTRUSTED_VERSION=$1
DEBIAN_ARCH=$2
LINUX_ARTIFACTSDIR=$3
THIS_SCRIPTS_DIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${THIS_SCRIPTS_DIR}/../../app)"
CONTAINER_USER="entrusted"

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

${THIS_SCRIPTS_DIR}/01-pre-chroot-script-arch-${DEBIAN_ARCH}.sh ${ENTRUSTED_VERSION} ${CONTAINER_USER}

retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Unable to export container image to tar archive!"
  exit 1
fi

test -d /tmp/hardened_malloc-${DEBIAN_ARCH} && rm -rf /tmp/hardened_malloc-${DEBIAN_ARCH}
mkdir -p /tmp/hardened_malloc-${DEBIAN_ARCH}

podman run --platform linux/${DEBIAN_ARCH} --rm -v "/tmp/hardened_malloc-${DEBIAN_ARCH}":/artifacts docker.io/uycyjnzgntrn/rust-linux:1.64.0 /bin/sh -c "mkdir -p /src && cd /src && git clone https://github.com/GrapheneOS/hardened_malloc.git && cd hardened_malloc && make N_ARENA=1 CONFIG_EXTENDED_SIZE_CLASSES=false && cp out/libhardened_malloc.so /artifacts/"

retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Could not build hardened_malloc!"
  exit 1
fi

cp /tmp/hardened_malloc-${DEBIAN_ARCH}/libhardened_malloc.so /tmp/live-libhardened_malloc.so && rm -rf /tmp/hardened_malloc-${DEBIAN_ARCH}
