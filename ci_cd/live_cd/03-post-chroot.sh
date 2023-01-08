#!/usr/bin/env sh
set -x

DEBIAN_ARCH=$1
LIVE_BOOT_DIR=$2
LIVE_BOOT_TMP_DIR=$3
LIVE_ISO_DIR=$4

ROOT_SCRIPTS_DIR="$(realpath $(dirname "$0"))"
CPU_ARCH="amd64"

if [ "${DEBIAN_ARCH}" != "amd64" ]
then
    CPU_ARCH="aarch64"
fi

echo ">>> Deleting previous artifacts ISO and squashfs files"
ENTRUSTED_VERSION=$(cat "${LIVE_BOOT_DIR}"/chroot/etc/entrusted_release | head -1)
test -d "${LIVE_ISO_DIR}" || mkdir -p "${LIVE_ISO_DIR}"

echo ">>> Creating Live CD squashfs filesystem"
test -f "${LIVE_BOOT_DIR}"/staging/live/filesystem.squashfs && sudo rm "${LIVE_BOOT_DIR}"/staging/live/filesystem.squashfs
mkdir -p "${LIVE_BOOT_DIR}"/staging/live
sudo mksquashfs "${LIVE_BOOT_DIR}"/chroot "${LIVE_BOOT_DIR}"/staging/live/filesystem.squashfs -e boot

echo ">>> Preparing Live CD boot files"
touch "${LIVE_BOOT_DIR}"/staging/ENTRUSTED_LIVE
mkdir -p "${LIVE_BOOT_DIR}"/staging/limine
cp "${LIVE_BOOT_DIR}"/chroot/boot/vmlinuz-* "${LIVE_BOOT_DIR}"/staging/live/vmlinuz
cp "${LIVE_BOOT_DIR}"/chroot/boot/initrd.img-* "${LIVE_BOOT_DIR}"/staging/live/initrd
cp "${LIVE_BOOT_TMP_DIR}"/limine/limine.sys "${LIVE_BOOT_DIR}"/staging/
cp "${ROOT_SCRIPTS_DIR}"/post_chroot_files/home/entrusted/LIVE_BOOT/staging/limine."${CPU_ARCH}".cfg "${LIVE_BOOT_DIR}"/staging/limine.cfg
cp "${LIVE_BOOT_TMP_DIR}"/limine/*.EFI "${LIVE_BOOT_DIR}"/staging/limine/
cp "${LIVE_BOOT_TMP_DIR}"/limine/*.bin "${LIVE_BOOT_DIR}"/staging/limine/
if [ "${DEBIAN_ARCH}" != "amd64" ]
then
    vmlinux-to-elf "${LIVE_BOOT_DIR}"/staging/live/vmlinuz "${LIVE_BOOT_DIR}"/staging/live/vmlinux || die "Could not build arm64 kernel elf file"
    rm "${LIVE_BOOT_DIR}"/staging/live/vmlinuz
fi

echo ">>> Creating Live CD ISO image"
xorriso -as mkisofs -iso-level 3 -o "${LIVE_ISO_DIR}"/entrusted-livecd-${CPU_ARCH}-${ENTRUSTED_VERSION}.iso -volid ENTRUSTED_LIVE -b limine/limine-cd.bin -no-emul-boot -boot-load-size 4 -boot-info-table --efi-boot limine/limine-cd-efi.bin -efi-boot-part --efi-boot-image --protective-msdos-label "${LIVE_BOOT_DIR}"/staging
"${LIVE_BOOT_TMP_DIR}"/limine/limine-deploy "${LIVE_ISO_DIR}"/entrusted-livecd-${CPU_ARCH}-${ENTRUSTED_VERSION}.iso

