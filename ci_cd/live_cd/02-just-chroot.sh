#!/usr/bin/env sh
set -x

LIVE_BOOT_DIR=$1
LIVE_BOOT_TMP_DIR=$2
ENTRUSTED_VERSION=$3
DEBIAN_ARCH=$4
ENTRUSTED_USER_NAME=$5
ENTRUSTED_USER_ID=$6

ROOT_SCRIPTS_DIR="$(realpath $(dirname "$0"))"

sudo cp -rf "${ROOT_SCRIPTS_DIR}"/chroot_files "${LIVE_BOOT_DIR}"/chroot/files
sudo cp -rf "${ROOT_SCRIPTS_DIR}"/03-in-chroot.sh "${LIVE_BOOT_DIR}"/chroot/files/
sudo cp "${LIVE_BOOT_TMP_DIR}"/live-libhardened_malloc.so "${LIVE_BOOT_DIR}"/chroot/files/libhardened_malloc.so
# sudo cp -r "${LIVE_BOOT_TMP_DIR}"/minikernel "${LIVE_BOOT_DIR}"/chroot/files/minikernel
sudo cp -r "${LIVE_BOOT_TMP_DIR}"/podman "${LIVE_BOOT_DIR}"/chroot/files/podman
sudo cp -r "${LIVE_BOOT_TMP_DIR}"/gvisor "${LIVE_BOOT_DIR}"/chroot/files/gvisor
sudo mv "${LIVE_BOOT_TMP_DIR}"/entrusted-packaging "${LIVE_BOOT_DIR}"/chroot/files/entrusted-packaging
sudo mv "${LIVE_BOOT_TMP_DIR}"/live-entrusted-cli "${LIVE_BOOT_DIR}"/chroot/files/entrusted-cli
sudo mv "${LIVE_BOOT_TMP_DIR}"/live-entrusted-webserver "${LIVE_BOOT_DIR}"/chroot/files/entrusted-webserver

sudo chmod +x "${LIVE_BOOT_DIR}"/chroot/files/*.sh

sudo echo "${ENTRUSTED_USER_NAME}" > "${LIVE_BOOT_TMP_DIR}"/entrusted_username
sudo echo "${ENTRUSTED_USER_ID}" > "${LIVE_BOOT_TMP_DIR}"/entrusted_userid
sudo mv "${LIVE_BOOT_TMP_DIR}"/entrusted_userid "${LIVE_BOOT_DIR}"/chroot/files/entrusted_userid
sudo mv "${LIVE_BOOT_TMP_DIR}"/entrusted_username "${LIVE_BOOT_DIR}"/chroot/files/entrusted_username

sudo echo "${DEBIAN_ARCH}" > "${LIVE_BOOT_TMP_DIR}"/entrusted_arch
sudo echo "${ENTRUSTED_VERSION}" > "${LIVE_BOOT_TMP_DIR}"/entrusted_release
sudo cp "${LIVE_BOOT_TMP_DIR}"/entrusted_release "${LIVE_BOOT_DIR}"/chroot/etc/entrusted_release
sudo cp "${LIVE_BOOT_TMP_DIR}"/entrusted_arch    "${LIVE_BOOT_DIR}"/chroot/etc/entrusted_arch

sudo systemd-nspawn -D "${LIVE_BOOT_DIR}"/chroot /files/03-in-chroot.sh
