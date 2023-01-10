#!/usr/bin/env sh
set -x

DEBIAN_ARCH=$1
LIVE_BOOT_DIR=$2
LIVE_BOOT_TMP_DIR=$3
LIVE_ISO_DIR=$4

ROOT_SCRIPTS_DIR="$(realpath $(dirname "$0"))"

EFI_ARCH="x86_64"
BOOT_EFI_ARCH="x64"
BOOT_EFI_ARCH_UPPER="X64"
CPU_ARCH="amd64"

if [ "${DEBIAN_ARCH}" != "amd64" ]
then
    EFI_ARCH="arm64"
    BOOT_EFI_ARCH="aa64"
    BOOT_EFI_ARCH_UPPER="AA64"
    CPU_ARCH="aarch64"
fi

echo ">>> Deleting previous artifacts ISO and squashfs files"
ENTRUSTED_VERSION=$(cat "${LIVE_BOOT_DIR}"/chroot/etc/entrusted_release | head -1)
test -d "${LIVE_ISO_DIR}" || mkdir -p "${LIVE_ISO_DIR}"

echo ">>> Creating Live CD squashfs filesystem"
test -f "${LIVE_BOOT_DIR}"/staging/live/filesystem.squashfs && sudo rm "${LIVE_BOOT_DIR}"/staging/live/filesystem.squashfs
mkdir -p "${LIVE_BOOT_DIR}"/staging/live
sudo mksquashfs "${LIVE_BOOT_DIR}"/chroot "${LIVE_BOOT_DIR}"/staging/live/filesystem.squashfs -e boot

echo ">>> Copying Live CD kernel, initrd and microcodes"
touch "${LIVE_BOOT_DIR}"/staging/ENTRUSTED_LIVE
mkdir -p "${LIVE_BOOT_DIR}"/staging/isolinux
cp "${LIVE_BOOT_TMP_DIR}"/microcode/*.img "${LIVE_BOOT_DIR}"/staging/live/
cp "${LIVE_BOOT_DIR}"/chroot/boot/vmlinuz-* "${LIVE_BOOT_DIR}"/staging/live/vmlinuz
cp "${LIVE_BOOT_DIR}"/chroot/boot/initrd.img-* "${LIVE_BOOT_DIR}"/staging/live/initrd

echo ">>> Creating BIOS/Legacy bootable components"
cp "${ROOT_SCRIPTS_DIR}"/post_chroot_files/home/entrusted/LIVE_BOOT/staging/isolinux/isolinux.cfg "${LIVE_BOOT_DIR}"/staging/isolinux/
cp /usr/lib/ISOLINUX/isolinux.bin   "${LIVE_BOOT_DIR}"/staging/isolinux/
cp /usr/lib/syslinux/modules/bios/* "${LIVE_BOOT_DIR}"/staging/isolinux/

echo ">>> Creating EFI bootable components"
mkdir -p "${LIVE_BOOT_DIR}"/tmp/efiboot/EFI/BOOT/systemd
mkdir -p "${LIVE_BOOT_DIR}"/tmp/efiboot/live
cp "${LIVE_BOOT_DIR}"/chroot/usr/lib/systemd/boot/efi/systemd-boot${BOOT_EFI_ARCH}.efi "${LIVE_BOOT_DIR}"/tmp/efiboot/EFI/BOOT/BOOT${BOOT_EFI_ARCH_UPPER}.EFI
cp "${LIVE_BOOT_DIR}"/chroot/usr/lib/systemd/boot/efi/systemd-boot${BOOT_EFI_ARCH}.efi "${LIVE_BOOT_DIR}"/tmp/efiboot/EFI/BOOT/systemd/
cp "${LIVE_BOOT_DIR}"/staging/live/vmlinuz "${LIVE_BOOT_DIR}"/tmp/efiboot/live/vmlinuz.gz
cp "${LIVE_BOOT_DIR}"/staging/live/initrd "${LIVE_BOOT_DIR}"/tmp/efiboot/live/initrd.gz
gzip -d "${LIVE_BOOT_DIR}"/tmp/efiboot/live/vmlinuz.gz
gzip -d "${LIVE_BOOT_DIR}"/tmp/efiboot/live/initrd.gz
cp "${LIVE_BOOT_DIR}"/staging/live/*.img "${LIVE_BOOT_DIR}"/tmp/efiboot/live/
cp -r "${ROOT_SCRIPTS_DIR}"/post_chroot_files/home/entrusted/LIVE_BOOT/tmp/efiboot/loader "${LIVE_BOOT_DIR}"/tmp/efiboot/
efiboot_size="$(du -hsm $LIVE_BOOT_DIR/tmp/efiboot | tail -1 | cut -f 1)"
efisize=$((efisize + 2))
truncate -s "${efiboot_size}M" "${LIVE_BOOT_DIR}"/tmp/efiboot.img
sudo mkfs.vfat "${LIVE_BOOT_DIR}"/tmp/efiboot.img
mkdir -p "${LIVE_BOOT_DIR}"/tmp/newefiboot
sudo mount "${LIVE_BOOT_DIR}"/tmp/efiboot.img "${LIVE_BOOT_DIR}"/tmp/newefiboot
sudo cp -r "${LIVE_BOOT_DIR}"/tmp/efiboot/* "${LIVE_BOOT_DIR}"/tmp/newefiboot/
sudo umount -d "${LIVE_BOOT_DIR}"/tmp/newefiboot && sudo rmdir "${LIVE_BOOT_DIR}"/tmp/newefiboot
cp "${LIVE_BOOT_DIR}"/tmp/efiboot.img "${LIVE_BOOT_DIR}"/staging/
sudo rm -rf "${LIVE_BOOT_DIR}"/tmp

echo ">>> Creating Live CD ISO image"
xorriso \
    -as mkisofs \
    -iso-level 3 \
    -o "${LIVE_ISO_DIR}/entrusted-livecd-${CPU_ARCH}-${ENTRUSTED_VERSION}.iso" \
    -full-iso9660-filenames \
    -volid "ENTRUSTED_LIVE" \
    -isohybrid-mbr /usr/lib/ISOLINUX/isohdpfx.bin  \
    -eltorito-boot isolinux/isolinux.bin \
    -no-emul-boot \
    -boot-load-size 4 \
    -boot-info-table \
    --eltorito-catalog isolinux/isolinux.cat \
    -eltorito-alt-boot \
    -e efiboot.img \
    -no-emul-boot \
    -isohybrid-gpt-basdat \
    --append_partition 2 0xef "${LIVE_BOOT_DIR}"/staging/efiboot.img \
    "${LIVE_BOOT_DIR}/staging"
