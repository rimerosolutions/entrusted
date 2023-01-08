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

PODMAN_VERSION="4.3.1"
THIS_SCRIPTS_DIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${THIS_SCRIPTS_DIR}/../../app)"

echo "Cleanup previous build"
test -d "${LIVE_BOOT_DIR}" && sudo rm -rf "${LIVE_BOOT_DIR}"

echo "Creating LIVE_BOOT folder"

mkdir -p "${LIVE_BOOT_DIR}"
sudo chmod -R a+rw "${LIVE_BOOT_DIR}"

echo ">>> Boostraping Debian installation"
sudo debootstrap \
    --arch=${DEBIAN_ARCH} \
    --variant=minbase \
    bullseye \
    "${LIVE_BOOT_DIR}"/chroot \
    https://mirror.csclub.uwaterloo.ca/debian/

cp "${LINUX_ARTIFACTSDIR}"/entrusted-cli "${LIVE_BOOT_TMP_DIR}"/live-entrusted-cli
cp "${LINUX_ARTIFACTSDIR}"/entrusted-webserver "${LIVE_BOOT_TMP_DIR}"/live-entrusted-webserver

# Podman hangs often with emulated arm64 builds, thus the --log-driver=none flag as attempt to mitigate
# See https://github.com/containers/podman/issues/13779

echo ">>> Building custom kernel"
test -d "${LIVE_BOOT_TMP_DIR}"/minikernel && rm -rf "${LIVE_BOOT_TMP_DIR}"/minikernel
mkdir -p "${LIVE_BOOT_TMP_DIR}"/minikernel
cp ${THIS_SCRIPTS_DIR}/chroot_files/usr/src/linux/config "${LIVE_BOOT_TMP_DIR}"/minikernel/
cp ${THIS_SCRIPTS_DIR}/chroot_files/usr/src/linux/sources.list "${LIVE_BOOT_TMP_DIR}"/minikernel/
podman run --platform linux/${DEBIAN_ARCH} --log-driver=none  -v "${LIVE_BOOT_TMP_DIR}/minikernel":/artifacts docker.io/uycyjnzgntrn/rust-linux:1.64.0 /bin/sh -c 'cat /artifacts/sources.list >> /etc/apt/sources.list && apt update && apt install -y linux-source-6.0 && apt install -y build-essential bc kmod cpio flex libncurses5-dev libelf-dev libssl-dev dwarves bison rsync && cd /usr/src && tar axf linux-source-*.tar.xz && rm linux-source*.tar.xz && cd linux-source-* && cp /artifacts/config .config && nice make -j`nproc` bindeb-pkg &&  cp ../*.deb /artifacts/' || (sleep 10 && podman run --platform linux/${DEBIAN_ARCH} --log-driver=none  -v "${LIVE_BOOT_TMP_DIR}/minikernel":/artifacts docker.io/uycyjnzgntrn/rust-linux:1.64.0 /bin/sh -c 'cat /artifacts/sources.list >> /etc/apt/sources.list && apt update && apt install -y linux-source-6.0 && apt install -y build-essential bc kmod cpio flex libncurses5-dev libelf-dev libssl-dev dwarves bison rsync && cd /usr/src && tar axf linux-source-*.tar.xz && rm linux-source*.tar.xz && cd linux-source-* && cp /artifacts/config .config && nice make -j`nproc` bindeb-pkg &&  cp ../*.deb /artifacts/')
retVal=$?
if [ "$retVal" != "0" ]; then
	echo "Could not build kernel!" && exit 1
fi
ls "${LIVE_BOOT_TMP_DIR}"/minikernel/*.deb || exit 1

echo ">>> Building limine bootloader for amd64"
podman run --platform linux/amd64  --log-driver=none  -v "${LIVE_BOOT_TMP_DIR}":/artifacts docker.io/uycyjnzgntrn/rust-linux:1.64.0 /bin/sh -c "mkdir -p /src && cd /src && git clone https://github.com/limine-bootloader/limine.git --branch=v4.x-branch-binary --depth=1 && cd limine && make && cp -r ../limine /artifacts/limine" || (sleep 10 && podman run --platform linux/amd64 --log-driver=none  -v "${LIVE_BOOT_TMP_DIR}":/artifacts docker.io/uycyjnzgntrn/rust-linux:1.64.0 /bin/sh -c "mkdir -p /src && cd /src && git clone https://github.com/limine-bootloader/limine.git --branch=v4.x-branch-binary --depth=1 && cd limine && make && cp -r ../limine /artifacts/limine")
retVal=$?
if [ "$retVal" != "0" ]; then
	echo "Could not build limine!" && exit 1
fi

echo ">>> Building hardened_malloc"
podman run --platform linux/${DEBIAN_ARCH} --log-driver=none  -v "${LIVE_BOOT_TMP_DIR}":/artifacts docker.io/uycyjnzgntrn/rust-linux:1.64.0 /bin/sh -c "mkdir -p /src && cd /src && git clone https://github.com/GrapheneOS/hardened_malloc.git && cd hardened_malloc && make N_ARENA=1 CONFIG_NATIVE=false CONFIG_EXTENDED_SIZE_CLASSES=false && cp /src/hardened_malloc/out/libhardened_malloc.so /artifacts/live-libhardened_malloc.so" || (sleep 10 && podman run --platform linux/${DEBIAN_ARCH} --log-driver=none -v "${LIVE_BOOT_TMP_DIR}":/artifacts docker.io/uycyjnzgntrn/rust-linux:1.64.0 /bin/sh -c "mkdir -p /src && cd /src && git clone https://github.com/GrapheneOS/hardened_malloc.git && cd hardened_malloc && make N_ARENA=1 CONFIG_NATIVE=false CONFIG_EXTENDED_SIZE_CLASSES=false && cp /src/hardened_malloc/out/libhardened_malloc.so /artifacts/live-libhardened_malloc.so")
retVal=$?
if [ "$retVal" != "0" ]; then
	echo "Could not build hardened_malloc!" && exit 1
fi

echo ">>> Downloading gvisor"
test -d "${LIVE_BOOT_TMP_DIR}"/gvisor && rm -rf "${LIVE_BOOT_TMP_DIR}"/gvisor
mkdir -p "${LIVE_BOOT_TMP_DIR}"/gvisor
cd "${LIVE_BOOT_TMP_DIR}"/gvisor && wget https://storage.googleapis.com/gvisor/releases/release/latest/${UNAME_ARCH}/runsc 
cd "${LIVE_BOOT_TMP_DIR}"/gvisor && wget https://storage.googleapis.com/gvisor/releases/release/latest/${UNAME_ARCH}/containerd-shim-runsc-v1
cd "${LIVE_BOOT_TMP_DIR}"/gvisor && chmod a+rx runsc containerd-shim-runsc-v1
retVal=$?
if [ "$retVal" != "0" ]; then
	echo "Could not download gvisor!" && exit 1
fi

echo ">>> Downloading podman-static"
test -d "${LIVE_BOOT_TMP_DIR}"/podman && rm -rf "${LIVE_BOOT_TMP_DIR}"/podman
mkdir -p "${LIVE_BOOT_TMP_DIR}"/podman
cd "${LIVE_BOOT_TMP_DIR}"/podman && wget https://github.com/mgoltzsche/podman-static/releases/download/v${PODMAN_VERSION}/podman-linux-${DEBIAN_ARCH}.tar.gz && cd -

CONTAINER_USER_HOMEDIR="/home/${CONTAINER_USER_NAME}"
sudo mkdir -p "${LIVE_BOOT_TMP_DIR}/home" && sudo chmod -R a+rw "${LIVE_BOOT_TMP_DIR}/home"
sudo useradd -m -s /bin/bash -d "${CONTAINER_USER_HOMEDIR}" -u ${CONTAINER_USER_ID} "${CONTAINER_USER_NAME}"
sudo test -d "${CONTAINER_USER_HOMEDIR}" || (sudo mkdir -p "${CONTAINER_USER_HOMEDIR}" && sudo chown -R "${CONTAINER_USER_NAME}" "${CONTAINER_USER_HOMEDIR}")
sudo adduser "${CONTAINER_USER_NAME}" sudo || true
sudo adduser "${CONTAINER_USER_NAME}" systemd-journal || true
sudo adduser "${CONTAINER_USER_NAME}" adm || true
sudo adduser "${CONTAINER_USER_NAME}" docker || true

cd /

cd -

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
cd / && sudo runuser -l "${CONTAINER_USER_NAME}" -c "XDG_RUNTIME_DIR=/run/user/${CONTAINER_USER_ID} podman run --log-driver=none docker-archive:${LIVE_BOOT_TMP_DIR}/image.tar ls / && podman images" && cd -
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

sudo cp -rf "${THIS_SCRIPTS_DIR}/chroot_files" "${LIVE_BOOT_DIR}/chroot/files"
sudo cp -rf "${THIS_SCRIPTS_DIR}/02-in-chroot.sh" "${LIVE_BOOT_DIR}/chroot/files/"
sudo cp "${LIVE_BOOT_TMP_DIR}/live-libhardened_malloc.so" "${LIVE_BOOT_DIR}/chroot/files/libhardened_malloc.so"
sudo cp -r "${LIVE_BOOT_TMP_DIR}/minikernel" "${LIVE_BOOT_DIR}/chroot/files/minikernel"
sudo cp -r "${LIVE_BOOT_TMP_DIR}/podman" "${LIVE_BOOT_DIR}/chroot/files/podman"
sudo cp -r "${LIVE_BOOT_TMP_DIR}/gvisor" "${LIVE_BOOT_DIR}/chroot/files/gvisor"
sudo mv "${LIVE_BOOT_TMP_DIR}/entrusted-packaging" "${LIVE_BOOT_DIR}/chroot/files/entrusted-packaging"
sudo mv "${LIVE_BOOT_TMP_DIR}/live-entrusted-cli" "${LIVE_BOOT_DIR}/chroot/files/entrusted-cli"
sudo mv "${LIVE_BOOT_TMP_DIR}/live-entrusted-webserver" "${LIVE_BOOT_DIR}/chroot/files/entrusted-webserver"

sudo chmod +x "${LIVE_BOOT_DIR}"/chroot/files/*.sh

echo "${ENTRUSTED_USER_NAME}" > "${LIVE_BOOT_TMP_DIR}"/entrusted_username
echo "${ENTRUSTED_USER_ID}" > "${LIVE_BOOT_TMP_DIR}"/entrusted_userid
sudo mv "${LIVE_BOOT_TMP_DIR}"/entrusted_userid "${LIVE_BOOT_DIR}"/chroot/files/entrusted_userid
sudo mv "${LIVE_BOOT_TMP_DIR}"/entrusted_username "${LIVE_BOOT_DIR}"/chroot/files/entrusted_username

echo "${DEBIAN_ARCH}" > "${LIVE_BOOT_TMP_DIR}"/entrusted_arch
echo "${ENTRUSTED_VERSION}" > "${LIVE_BOOT_TMP_DIR}"/entrusted_release
sudo cp "${LIVE_BOOT_TMP_DIR}"/entrusted_release "${LIVE_BOOT_DIR}"/chroot/etc/entrusted_release
sudo cp "${LIVE_BOOT_TMP_DIR}"/entrusted_arch    "${LIVE_BOOT_DIR}"/chroot/etc/entrusted_arch

sudo systemd-nspawn -D "${LIVE_BOOT_DIR}"/chroot /files/02-in-chroot.sh
