#!/usr/bin/env sh
set -x

DEBIAN_ARCH=$1
LIVE_BOOT_DIR=$2
LIVE_ISO_DIR=$3
LIVE_BOOT_TMP_DIR=$4

ROOT_SCRIPTS_DIR="$(realpath $(dirname "$0"))"
PREVIOUSDIR="$(echo $PWD)"

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
sudo mksquashfs "${LIVE_BOOT_DIR}"/chroot "${LIVE_BOOT_DIR}"/staging/live/filesystem.squashfs \
     -e boot \
     -b 1M \
     -Xdict-size 1M \
     -no-recovery \
     -comp zstd \
     -Xcompression-level 22

echo ">>> Copying Live CD kernel, initrd"
mkdir -p "${LIVE_BOOT_DIR}"/staging/isolinux
cp "${LIVE_BOOT_DIR}"/chroot/boot/vmlinuz-* "${LIVE_BOOT_DIR}"/staging/live/vmlinuz
cp "${LIVE_BOOT_DIR}"/chroot/boot/initrd.img-* "${LIVE_BOOT_DIR}"/staging/live/initrd

echo ">>> Creating EFI bootable components"
cp "${ROOT_SCRIPTS_DIR}"/post_chroot_files/home/entrusted/LIVE_BOOT/staging/isolinux/grub.cfg "${LIVE_BOOT_DIR}"/staging/isolinux/

podman run  \
       --platform linux/amd64 \
       -v "${LIVE_BOOT_DIR}/staging/isolinux":/ISOLINUX \
       docker.io/uycyjnzgntrn/grub-${CPU_ARCH}:fedora-37 \
       ${CPU_ARCH}-grub-mkstandalone \
       --format=${EFI_ARCH}-efi \
       --output=/ISOLINUX/BOOT${BOOT_EFI_ARCH_UPPER}.efi \
       --modules="part_gpt part_msdos fat iso9660" \
       --locales="" \
       --fonts="" \
       boot/grub/grub.cfg=/ISOLINUX/grub.cfg

ls "${LIVE_BOOT_DIR}"/staging/isolinux/BOOT${BOOT_EFI_ARCH_UPPER}.efi || (echo "Unable to EFI bootable components!" && exit 1)

echo ">>> Creating FAT16 UEFI boot disk image"
dd if=/dev/zero of=${LIVE_BOOT_DIR}/staging/isolinux/efiboot.img bs=1M count=10 && \
    sudo mkfs.vfat ${LIVE_BOOT_DIR}/staging/isolinux/efiboot.img && \
    LC_CTYPE=C mmd -i ${LIVE_BOOT_DIR}/staging/isolinux/efiboot.img EFI EFI/BOOT && \
    LC_CTYPE=C mcopy -i ${LIVE_BOOT_DIR}/staging/isolinux/efiboot.img "${LIVE_BOOT_DIR}"/staging/isolinux/BOOT${BOOT_EFI_ARCH_UPPER}.efi ::EFI/BOOT/

echo ">>> Creating Grub BIOS image"
podman run  \
       --platform linux/amd64 \
       -v "${LIVE_BOOT_DIR}/staging/isolinux":/ISOLINUX \
       docker.io/uycyjnzgntrn/grub-amd64:fedora-37 \
       ${CPU_ARCH}-grub-mkstandalone \
       --format=i386-pc \
       --output="/ISOLINUX/core.img" \
       --install-modules="linux16 linux normal iso9660 biosdisk memdisk search tar ls" \
       --modules="linux16 linux normal iso9660 biosdisk search" \
       --locales="" \
       --fonts="" \
       "boot/grub/grub.cfg=/ISOLINUX/grub.cfg"

echo ">>> Combine bootable Grub cdboot.img"
podman run  \
       --platform linux/amd64 \
       -v "${LIVE_BOOT_DIR}/staging/isolinux":/ISOLINUX \
       docker.io/uycyjnzgntrn/grub-amd64:fedora-37 \
       sh -c "cat /usr/lib/grub/i386-pc/cdboot.img /ISOLINUX/core.img > /ISOLINUX/bios.img"

echo ">>> Creating Live CD ISO image"
podman run  \
       --platform linux/amd64 \
       -v "${LIVE_BOOT_TMP_DIR}":/MYTMP \
       docker.io/uycyjnzgntrn/grub-amd64:fedora-37 \
       sh -c "cp /usr/lib/grub/i386-pc/boot_hybrid.img /MYTMP"

sudo xorriso \
     -as mkisofs \
     -iso-level 3 \
     -full-iso9660-filenames \
     -volid "ENTRUSTED_LIVE" \
     -output "${LIVE_ISO_DIR}/entrusted-${ENTRUSTED_VERSION}-livecd-${CPU_ARCH}.iso" \
     -eltorito-boot boot/grub/bios.img \
     -no-emul-boot \
     -boot-load-size 4 \
     -boot-info-table \
     --eltorito-catalog boot/grub/boot.cat \
     --grub2-boot-info \
     --grub2-mbr "${LIVE_BOOT_TMP_DIR}/boot_hybrid.img" \
     -eltorito-alt-boot \
     -e EFI/efiboot.img \
     -no-emul-boot \
     -append_partition 2 0xef ${LIVE_BOOT_DIR}/staging/isolinux/efiboot.img \
     -m "${LIVE_BOOT_DIR}/staging/isolinux/efiboot.img" \
     -m "${LIVE_BOOT_DIR}/staging/isolinux/bios.img" \
     -graft-points \
     "/EFI/efiboot.img=${LIVE_BOOT_DIR}/staging/isolinux/efiboot.img" \
     "/boot/grub/bios.img=${LIVE_BOOT_DIR}/staging/isolinux/bios.img" \
     "${LIVE_BOOT_DIR}/staging"

cd $PREVIOUSDIR
