#!/usr/bin/env sh
set -x

ENTRUSTED_VERSION=$1
DEBIAN_ARCH=$2
CONTAINER_ARCH=$3
LINUX_ARTIFACTSDIR=$4
LIVE_BOOT_DIR=$5
LIVE_BOOT_TMP_DIR=$6
CONTAINER_USER=$7
CONTAINER_USER_ID=$8

THIS_SCRIPTS_DIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${THIS_SCRIPTS_DIR}/../../app)"

echo "Cleanup previous build"
test -d "${LIVE_BOOT_DIR}" && sudo rm -rf "${LIVE_BOOT_DIR}"

echo "Creating LIVE_BOOT folder"

mkdir -p "${LIVE_BOOT_DIR}"
sudo chmod -R a+rw "${LIVE_BOOT_DIR}"

echo "Creating bootstrap environment for minimal Debian installation"
sudo debootstrap \
    --arch=${DEBIAN_ARCH} \
    --variant=minbase \
    bullseye \
    "${LIVE_BOOT_DIR}"/chroot \
    https://mirror.csclub.uwaterloo.ca/debian/

cp "${LINUX_ARTIFACTSDIR}"/entrusted-cli "${LIVE_BOOT_TMP_DIR}"/live-entrusted-cli
cp "${LINUX_ARTIFACTSDIR}"/entrusted-webserver "${LIVE_BOOT_TMP_DIR}"/live-entrusted-webserver

echo "It is assumed that you published the entrusted container image to Docker Hub already..."
test -d "${LIVE_BOOT_TMP_DIR}"/entrusted-packaging &&  sudo rm -rf "${LIVE_BOOT_TMP_DIR}"/entrusted-packaging

sudo mkdir -p "${LIVE_BOOT_TMP_DIR}"/entrusted-packaging && sudo chmod -R a+rw "${LIVE_BOOT_TMP_DIR}"/entrusted-packaging

CONTAINER_USER_HOMEDIR="/home/${CONTAINER_USER}"
sudo killall -u "${CONTAINER_USER}" || true
sudo userdel -r "${CONTAINER_USER}" || true
sudo mkdir -p "${LIVE_BOOT_TMP_DIR}/home" && sudo chmod -R a+rw "${LIVE_BOOT_TMP_DIR}/home"
sudo useradd -m -s /bin/bash -d "${CONTAINER_USER_HOMEDIR}" -u ${CONTAINER_USER_ID} "${CONTAINER_USER}"
sudo adduser "${CONTAINER_USER}" sudo || true
sudo adduser "${CONTAINER_USER}" systemd-journal || true
sudo adduser "${CONTAINER_USER}" adm || true
sudo adduser "${CONTAINER_USER}" docker || true


cd /

sudo runuser -l "${CONTAINER_USER}" -c "mkdir -p ${CONTAINER_USER_HOMEDIR}/.config/containers"

cd -

mkdir -p ~/.config/containers

echo "Workspace: ${PROJECTDIR}"
echo "container arch: linux/${DEBIAN_ARCH}"
echo "image tag: linux/docker.io/uycyjnzgntrn/entrusted_container:${ENTRUSTED_VERSION}"

cd "${PROJECTDIR}" && podman build --jobs 2 --squash-all --force-rm --platform "linux/${DEBIAN_ARCH}" -t "docker.io/uycyjnzgntrn/entrusted_container:${ENTRUSTED_VERSION}" -f entrusted_container/Dockerfile . && cd -

podman save -m -o ${LIVE_BOOT_TMP_DIR}/image.tar docker.io/uycyjnzgntrn/entrusted_container:${ENTRUSTED_VERSION}


retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Unable to export container image to tar archive!"
  exit 1
fi

sudo loginctl enable-linger ${CONTAINER_USER_ID}
sudo mkdir -p /run/user/${CONTAINER_USER_ID} && sudo chown -R ${CONTAINER_USER} /run/user/${CONTAINER_USER_ID}
cd / && sudo runuser -l "${CONTAINER_USER}" -c "podman run docker-archive:${LIVE_BOOT_TMP_DIR}/image.tar ls / && podman images" && cd -

sudo rm ${LIVE_BOOT_TMP_DIR}/image.tar

#sudo rm -rf ${PROJECTDIR}

cd /

sudo cp -r ${CONTAINER_USER_HOMEDIR}/.local/share/containers ${LIVE_BOOT_TMP_DIR}/entrusted-packaging

cd -

test -d "${LIVE_BOOT_TMP_DIR}"/hardened_malloc-${DEBIAN_ARCH} && rm -rf "${LIVE_BOOT_TMP_DIR}"/hardened_malloc-${DEBIAN_ARCH}
mkdir -p "${LIVE_BOOT_TMP_DIR}"/hardened_malloc-${DEBIAN_ARCH}
podman run --platform linux/${DEBIAN_ARCH} --rm -v "${LIVE_BOOT_TMP_DIR}/hardened_malloc-${DEBIAN_ARCH}":/artifacts docker.io/uycyjnzgntrn/rust-linux:1.64.0 /bin/sh -c "mkdir -p /src && cd /src && git clone https://github.com/GrapheneOS/hardened_malloc.git && cd hardened_malloc && make N_ARENA=1 CONFIG_EXTENDED_SIZE_CLASSES=false && cp /src/hardened_malloc/out/libhardened_malloc.so /artifacts/"
sudo cp "${LIVE_BOOT_TMP_DIR}/hardened_malloc-${DEBIAN_ARCH}"/libhardened_malloc.so "${LIVE_BOOT_TMP_DIR}"/live-libhardened_malloc.so
sudo rm -rf "${LIVE_BOOT_TMP_DIR}/hardened_malloc-${DEBIAN_ARCH}"

retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Could not build hardened_malloc!"
  exit 1
fi

