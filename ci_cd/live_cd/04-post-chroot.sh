#!/usr/bin/env sh
set -x

DEBIAN_ARCH=$1
LIVE_BOOT_DIR=$2
LIVE_ISO_DIR=$3

ROOT_SCRIPTS_DIR="$(realpath $(dirname "$0"))"
EFI_ARCH="x86_64"
BOOT_EFI_ARCH="x64"
CPU_ARCH="amd64"

if [ ${DEBIAN_ARCH} != "amd64" ]
then
    EFI_ARCH="arm64"
    BOOT_EFI_ARCH="aa64"
    CPU_ARCH="aarch64"
fi

echo "Deleting previous artifacts ISO and squashfs files"
ENTRUSTED_VERSION=$(cat "${LIVE_BOOT_DIR}"/chroot/etc/entrusted_release | head -1)
test -d "${LIVE_ISO_DIR}" && mkdir -p "${LIVE_ISO_DIR}"
test -f "${LIVE_BOOT_DIR}"/staging/live/filesystem.squashfs && sudo rm "${LIVE_BOOT_DIR}"/staging/live/filesystem.squashfs

echo "Creating filesystem"
mkdir -p "${LIVE_BOOT_DIR}"/staging/EFI/boot/
mkdir -p "${LIVE_BOOT_DIR}"/staging/boot/grub/${EFI_ARCH}-efi                          
mkdir -p "${LIVE_BOOT_DIR}"/staging/isolinux
mkdir -p "${LIVE_BOOT_DIR}"/staging/live
mkdir -p "${LIVE_BOOT_DIR}"/tmp

sudo mksquashfs "${LIVE_BOOT_DIR}"/chroot "${LIVE_BOOT_DIR}"/staging/live/filesystem.squashfs -e boot

echo "Preparing boot files"
cp "${LIVE_BOOT_DIR}"/chroot/boot/vmlinuz-* "${LIVE_BOOT_DIR}"/staging/live/vmlinuz
cp "${LIVE_BOOT_DIR}"/chroot/boot/initrd.img-* "${LIVE_BOOT_DIR}"/staging/live/initrd

cp "${ROOT_SCRIPTS_DIR}"/post_chroot_files/home/entrusted/LIVE_BOOT/staging/isolinux/isolinux.cfg "${LIVE_BOOT_DIR}"/staging/isolinux/isolinux.cfg
cp "${ROOT_SCRIPTS_DIR}"/post_chroot_files/home/entrusted/LIVE_BOOT/staging/boot/grub/grub.cfg    "${LIVE_BOOT_DIR}"/staging/boot/grub/grub.cfg
cp "${ROOT_SCRIPTS_DIR}"/post_chroot_files/home/entrusted/LIVE_BOOT/tmp/grub-standalone.cfg       "${LIVE_BOOT_DIR}"/tmp/grub-standalone.cfg

touch "${LIVE_BOOT_DIR}"/staging/DEBIAN_CUSTOM

cp /usr/lib/ISOLINUX/isolinux.bin  "${LIVE_BOOT_DIR}"/staging/isolinux/ \
    && cp /usr/lib/syslinux/modules/bios/* "${LIVE_BOOT_DIR}"/staging/isolinux/
cp -r "${LIVE_BOOT_DIR}"/chroot/usr/lib/grub/${EFI_ARCH}-efi/* "${LIVE_BOOT_DIR}"/staging/boot/grub/${EFI_ARCH}-efi/

grub-mkstandalone --format=${EFI_ARCH}-efi  --directory="${LIVE_BOOT_DIR}"/chroot/usr/lib/grub/${EFI_ARCH}-efi --output="${LIVE_BOOT_DIR}"/tmp/boot${BOOT_EFI_ARCH}.efi --locales= --fonts= boot/grub/grub.cfg="${LIVE_BOOT_DIR}"/tmp/grub-standalone.cfg
(cd "${LIVE_BOOT_DIR}"/staging/EFI/boot && dd if=/dev/zero of=efiboot.img bs=1M count=20 && /sbin/mkfs.vfat efiboot.img && mmd -i efiboot.img efi efi/boot && mcopy -vi efiboot.img "${LIVE_BOOT_DIR}"/tmp/boot${BOOT_EFI_ARCH}.efi ::efi/boot/)

echo "Creating Live CD ISO image"
xorriso -as mkisofs -iso-level 3 -o "${LIVE_ISO_DIR}"/entrusted-livecd-${CPU_ARCH}-${ENTRUSTED_VERSION}.iso -full-iso9660-filenames -volid DEBIAN_CUSTOM -isohybrid-mbr /usr/lib/ISOLINUX/isohdpfx.bin  -eltorito-boot isolinux/isolinux.bin -no-emul-boot -boot-load-size 4 -boot-info-table --eltorito-catalog isolinux/isolinux.cat -eltorito-alt-boot -e /EFI/boot/efiboot.img -no-emul-boot -isohybrid-gpt-basdat --append_partition 2 0xef "${LIVE_BOOT_DIR}"/staging/EFI/boot/efiboot.img "${LIVE_BOOT_DIR}"/staging
