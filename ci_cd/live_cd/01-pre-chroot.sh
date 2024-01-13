#!/usr/bin/env sh
set -x

ENTRUSTED_VERSION=$1
UNAME_ARCH=$2
DEBIAN_ARCH=$3
CONTAINER_ARCH=$4
LINUX_ARTIFACTSDIR=$5
LIVE_BOOT_DIR=$6
LIVE_BOOT_TMP_DIR=$7
CONTAINER_USER_NAME=$8
CONTAINER_USER_ID=$9

VERSION_PODMAN_STATIC="4.8.2"
VERSION_KERNEL_DEBLIVE_SMALLSERVER="6.6.11"
THIS_SCRIPTS_DIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${THIS_SCRIPTS_DIR}/../../app)"
VERSION_HARDENED_MALLOC="2024010400"
VERSION_GVISOR="20231218.0"
RUST_CI_VERSION="1.72.0"

echo ">>> Creating LIVE_BOOT folder"
test -d "${LIVE_BOOT_DIR}"  && sudo rm -rf "${LIVE_BOOT_DIR}"
mkdir -p "${LIVE_BOOT_DIR}/chroot" && sudo chmod -R a+rw "${LIVE_BOOT_DIR}"

echo ">>> Boostraping Debian installation"
sudo debootstrap --arch=${DEBIAN_ARCH} --variant=minbase bookworm "${LIVE_BOOT_DIR}"/chroot https://mirror.csclub.uwaterloo.ca/debian/ || (sleep 10 && sudo debootstrap --arch=${DEBIAN_ARCH} --variant=minbase bookworm "${LIVE_BOOT_DIR}"/chroot https://mirror.csclub.uwaterloo.ca/debian/)
retVal=$?
if [ "$retVal" != "0" ]; then
	echo "Could not bootstrap Debian installation!" && exit 1
fi

echo ">>> Copying entrusted-cli and entrusted-webserver to temporary storage"
cp "${LINUX_ARTIFACTSDIR}"/entrusted-cli "${LIVE_BOOT_TMP_DIR}"/live-entrusted-cli
cp "${LINUX_ARTIFACTSDIR}"/entrusted-webserver "${LIVE_BOOT_TMP_DIR}"/live-entrusted-webserver

echo ">>> Downloading custom kernel"
RELNUM_KERNEL_DEBLIVE_SMALLSERVER=$(echo $VERSION_KERNEL_DEBLIVE_SMALLSERVER | awk -F"." '{print $1"."$2}')
podman run --platform linux/${DEBIAN_ARCH} \
       --rm \
       --log-driver=none  \
       -v "${LIVE_BOOT_TMP_DIR}":/live_boot_tmp_dir \
       docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION} \
       /bin/sh -c "test -d /live_boot_tmp_dir/minikernel && rm -rf /live_boot_tmp_dir/minikernel; mkdir -p /live_boot_tmp_dir/minikernel; wget -P /live_boot_tmp_dir/minikernel https://github.com/yveszoundi/kernel-deblive-smallserver/releases/download/${RELNUM_KERNEL_DEBLIVE_SMALLSERVER}/kernel-deblive-smallserver-${VERSION_KERNEL_DEBLIVE_SMALLSERVER}-${DEBIAN_ARCH}.zip && unzip -d /live_boot_tmp_dir/minikernel /live_boot_tmp_dir/minikernel/kernel-deblive-smallserver-${VERSION_KERNEL_DEBLIVE_SMALLSERVER}-${DEBIAN_ARCH}.zip"
ls "${LIVE_BOOT_TMP_DIR}"/minikernel/*.deb || (echo "Could not fetch custom kernel!" && exit 1)
retVal=$?
if [ "$retVal" != "0" ]; then
	echo "Could not fetch custom kernel!" && exit 1
fi

echo ">>> Building hardened_malloc"
podman run --platform linux/${DEBIAN_ARCH} \
       --rm \
       --log-driver=none  \
       -v "${LIVE_BOOT_TMP_DIR}":/artifacts \
       docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION} \
       /bin/sh -c "mkdir -p /tmp/src && cd /tmp/src && git clone https://github.com/GrapheneOS/hardened_malloc.git && cd hardened_malloc  && git checkout ${VERSION_HARDENED_MALLOC} && make N_ARENA=1 CONFIG_NATIVE=false CONFIG_EXTENDED_SIZE_CLASSES=false && cp /tmp/src/hardened_malloc/out/libhardened_malloc.so /artifacts/live-libhardened_malloc.so" || (sleep 10 && podman run --rm --platform linux/${DEBIAN_ARCH} --log-driver=none -v "${LIVE_BOOT_TMP_DIR}":/artifacts docker.io/uycyjnzgntran/rust-linux:${RUST_CI_VERSION} /bin/sh -c "mkdir -p /tmp/src && cd /tmp/src && git clone https://github.com/GrapheneOS/hardened_malloc.git && cd hardened_malloc && git checkout ${VERSION_HARDENED_MALLOC} && make N_ARENA=1 CONFIG_NATIVE=false CONFIG_EXTENDED_SIZE_CLASSES=false && cp /tmp/src/hardened_malloc/out/libhardened_malloc.so /artifacts/live-libhardened_malloc.so")
retVal=$?
if [ "$retVal" != "0" ]; then
	echo "Could not build hardened_malloc!" && exit 1
fi

echo ">>> Downloading gvisor"
podman run --platform linux/${DEBIAN_ARCH} \
       --rm \
       --log-driver=none  \
       -v "${LIVE_BOOT_TMP_DIR}":/live_boot_tmp_dir \
       docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION} \
       /bin/sh -c "test -d /live_boot_tmp_dir/gvisor && rm -rf /live_boot_tmp_dir/gvisor; mkdir -p /live_boot_tmp_dir/gvisor && wget -P /live_boot_tmp_dir/gvisor https://storage.googleapis.com/gvisor/releases/release/${VERSION_GVISOR}/${UNAME_ARCH}/runsc https://storage.googleapis.com/gvisor/releases/release/${VERSION_GVISOR}/${UNAME_ARCH}/containerd-shim-runsc-v1 && chmod +x /live_boot_tmp_dir/gvisor/*"
retVal=$?
if [ "$retVal" != "0" ]; then
	echo "Could not download gvisor!" && exit 1
fi

echo ">>> Downloading podman-static"
podman run --platform linux/${DEBIAN_ARCH} \
       --rm \
       --log-driver=none  \
       -v "${LIVE_BOOT_TMP_DIR}":/live_boot_tmp_dir \
       docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION} \
       /bin/sh -c "test -d /live_boot_tmp_dir/podman && rm -rf /live_boot_tmp_dir/podman; mkdir -p /live_boot_tmp_dir/podman; wget -P /live_boot_tmp_dir/podman https://github.com/mgoltzsche/podman-static/releases/download/v${VERSION_PODMAN_STATIC}/podman-linux-${DEBIAN_ARCH}.tar.gz"
retVal=$?
if [ "$retVal" != "0" ]; then
	echo "Could not download podman-static!" && exit 1
fi

CONTAINER_USER_HOMEDIR="/home/${CONTAINER_USER_NAME}"
sudo mkdir -p "${LIVE_BOOT_TMP_DIR}/home" && sudo chmod -R a+rw "${LIVE_BOOT_TMP_DIR}/home"
sudo useradd -m -s /bin/bash -d "${CONTAINER_USER_HOMEDIR}" -u ${CONTAINER_USER_ID} "${CONTAINER_USER_NAME}"
sudo test -d "${CONTAINER_USER_HOMEDIR}" || (sudo mkdir -p "${CONTAINER_USER_HOMEDIR}" && sudo chown -R "${CONTAINER_USER_NAME}" "${CONTAINER_USER_HOMEDIR}")
sudo adduser "${CONTAINER_USER_NAME}" sudo || true
sudo adduser "${CONTAINER_USER_NAME}" systemd-journal || true
sudo adduser "${CONTAINER_USER_NAME}" adm || true
sudo adduser "${CONTAINER_USER_NAME}" docker || true

echo "Workspace: ${PROJECTDIR}"
echo "Container arch: linux/${DEBIAN_ARCH}"
echo "Image tag: docker.io/uycyjnzgntrn/entrusted_container:${ENTRUSTED_VERSION}"

echo ">>> Building container image"
cd "${PROJECTDIR}" && ( (podman build --squash-all --jobs 2  --platform "linux/${DEBIAN_ARCH}" -t "docker.io/uycyjnzgntrn/entrusted_container:${ENTRUSTED_VERSION}" -f entrusted_container/Dockerfile .) || (sleep 10 && podman build --squash-all  --platform "linux/${DEBIAN_ARCH}" -t "docker.io/uycyjnzgntrn/entrusted_container:${ENTRUSTED_VERSION}" -f entrusted_container/Dockerfile .) ) && cd -
podman image prune -f --filter label=stage=entrusted_container_builder || true
podman save -m -o ${LIVE_BOOT_TMP_DIR}/image.tar docker.io/uycyjnzgntrn/entrusted_container:${ENTRUSTED_VERSION}
retVal=$?
if [ "$retVal" != "0" ]; then
	echo "Unable to export container image to tar archive!" && exit 1
fi

sudo loginctl enable-linger ${CONTAINER_USER_NAME}
sudo mkdir -p /run/user/${CONTAINER_USER_ID}
sudo mkdir -p /run/user/${CONTAINER_USER_ID} && sudo chown -R ${CONTAINER_USER_NAME} /run/user/${CONTAINER_USER_ID}
cd / && sudo runuser -l "${CONTAINER_USER_NAME}" -c "XDG_RUNTIME_DIR=/run/user/${CONTAINER_USER_ID} podman run --rm --log-driver=none docker-archive:${LIVE_BOOT_TMP_DIR}/image.tar ls / && podman images" && cd -
sudo rm ${LIVE_BOOT_TMP_DIR}/image.tar
cd /
test -d "${LIVE_BOOT_TMP_DIR}"/entrusted-packaging &&  sudo rm -rf "${LIVE_BOOT_TMP_DIR}"/entrusted-packaging
sudo mkdir -p "${LIVE_BOOT_TMP_DIR}"/entrusted-packaging && sudo chmod -R a+rw "${LIVE_BOOT_TMP_DIR}"/entrusted-packaging
sudo cp -r ${CONTAINER_USER_HOMEDIR}/.local/share/containers ${LIVE_BOOT_TMP_DIR}/entrusted-packaging
retVal=$?
if [ "$retVal" != "0" ]; then
	echo "Could not copy container image data!" && exit 1
fi
cd -

sudo cp -rf "${THIS_SCRIPTS_DIR}/in_chroot_files" "${LIVE_BOOT_DIR}/chroot/files"
sudo cp -f "${THIS_SCRIPTS_DIR}/02-in-chroot.sh"  "${LIVE_BOOT_DIR}/chroot/files/"
sudo cp -rf "${LIVE_BOOT_TMP_DIR}/gvisor"         "${LIVE_BOOT_DIR}/chroot/files/gvisor"
sudo cp -rf "${LIVE_BOOT_TMP_DIR}/minikernel"     "${LIVE_BOOT_DIR}/chroot/files/minikernel"
sudo cp -rf "${LIVE_BOOT_TMP_DIR}/podman"         "${LIVE_BOOT_DIR}/chroot/files/podman"

sudo cp "${LIVE_BOOT_TMP_DIR}/live-libhardened_malloc.so" "${LIVE_BOOT_DIR}/chroot/files/libhardened_malloc.so"
sudo mv "${LIVE_BOOT_TMP_DIR}/entrusted-packaging"        "${LIVE_BOOT_DIR}/chroot/files/entrusted-packaging"
sudo mv "${LIVE_BOOT_TMP_DIR}/live-entrusted-cli"         "${LIVE_BOOT_DIR}/chroot/files/entrusted-cli"
sudo mv "${LIVE_BOOT_TMP_DIR}/live-entrusted-webserver"   "${LIVE_BOOT_DIR}/chroot/files/entrusted-webserver"

sudo chmod +x "${LIVE_BOOT_DIR}"/chroot/files/*.sh

echo "${CONTAINER_USER_NAME}" > "${LIVE_BOOT_TMP_DIR}"/entrusted_username
echo "${CONTAINER_USER_ID}"   > "${LIVE_BOOT_TMP_DIR}"/entrusted_userid
sudo mv "${LIVE_BOOT_TMP_DIR}"/entrusted_userid "${LIVE_BOOT_DIR}"/chroot/files/entrusted_userid
sudo mv "${LIVE_BOOT_TMP_DIR}"/entrusted_username "${LIVE_BOOT_DIR}"/chroot/files/entrusted_username

echo "${DEBIAN_ARCH}" > "${LIVE_BOOT_TMP_DIR}"/entrusted_arch
echo "${ENTRUSTED_VERSION}" > "${LIVE_BOOT_TMP_DIR}"/entrusted_release
sudo cp "${LIVE_BOOT_TMP_DIR}"/entrusted_release "${LIVE_BOOT_DIR}"/chroot/etc/entrusted_release
sudo cp "${LIVE_BOOT_TMP_DIR}"/entrusted_arch    "${LIVE_BOOT_DIR}"/chroot/etc/entrusted_arch

sudo systemd-nspawn -D "${LIVE_BOOT_DIR}"/chroot /files/02-in-chroot.sh
