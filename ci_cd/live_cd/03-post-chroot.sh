#!/usr/bin/env sh
set -x

DEBIAN_ARCH=$1
LIVE_BOOT_DIR=$2
LIVE_ISO_DIR=$3

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
sudo mksquashfs "${LIVE_BOOT_DIR}"/chroot "${LIVE_BOOT_DIR}"/staging/live/filesystem.squashfs -e boot -b 1M -Xdict-size 1M -no-recovery -comp zstd -Xcompression-level 19

echo ">>> Copying Live CD kernel, initrd"
mkdir -p "${LIVE_BOOT_DIR}"/staging/isolinux
cp "${LIVE_BOOT_DIR}"/chroot/boot/vmlinuz-* "${LIVE_BOOT_DIR}"/staging/live/vmlinuz
cp "${LIVE_BOOT_DIR}"/chroot/boot/initrd.img-* "${LIVE_BOOT_DIR}"/staging/live/initrd

echo ">>> Creating BIOS/Legacy bootable components"
cp "${ROOT_SCRIPTS_DIR}"/post_chroot_files/home/entrusted/LIVE_BOOT/staging/isolinux/grub.cfg "${LIVE_BOOT_DIR}"/staging/isolinux/
if [ "${DEBIAN_ARCH}" != "amd64" ]
then
    podman run -it \
           --platform linux/amd64 \
           -v "${LIVE_BOOT_DIR}/staging/isolinux":/ISOLINUX \
           ghcr.io/linux-surface/grub-aarch64:fedora-37-2 \
           aarch64-grub-mkstandalone --format=${EFI_ARCH}-efi \
           --output=/ISOLINUX/BOOT${BOOT_EFI_ARCH_UPPER}.efi \
           --modules="part_gpt part_msdos" \
           --locales="" \
           --fonts="" \
           boot/grub/grub.cfg=/ISOLINUX/grub.cfg                      
else
    grub-mkstandalone --format=${EFI_ARCH}-efi \
                      --output="${LIVE_BOOT_DIR}"/staging/isolinux/BOOT${BOOT_EFI_ARCH_UPPER}.efi \
                      --modules="part_gpt part_msdos" \
                      --locales="" \
                      --fonts="" \
                      boot/grub/grub.cfg="${LIVE_BOOT_DIR}"/staging/isolinux/grub.cfg
fi

echo ">>> Creating FAT16 UEFI boot disk image"
dd if=/dev/zero of=${LIVE_BOOT_DIR}/staging/efiboot.img bs=1M count=10 && \
    sudo mkfs.vfat ${LIVE_BOOT_DIR}/staging/efiboot.img && \
    LC_CTYPE=C mmd -i ${LIVE_BOOT_DIR}/staging/efiboot.img EFI EFI/BOOT && \
    LC_CTYPE=C mcopy -i ${LIVE_BOOT_DIR}/staging/efiboot.img "${LIVE_BOOT_DIR}"/staging/isolinux/BOOT${BOOT_EFI_ARCH_UPPER}.efi ::EFI/BOOT/

echo ">>> Creating Grub BIOS image"
grub-mkstandalone \
    --format=i386-pc \
    --output="${LIVE_BOOT_DIR}/staging/isolinux/core.img" \
    --install-modules="linux16 linux normal iso9660 biosdisk memdisk search tar ls" \
    --modules="linux16 linux normal iso9660 biosdisk search" \
    --locales="" \
    --fonts="" \
    "boot/grub/grub.cfg=${LIVE_BOOT_DIR}/staging/isolinux/grub.cfg"

echo ">>> Combine bootable Grub cdboot.img"
cat /usr/lib/grub/i386-pc/cdboot.img ${LIVE_BOOT_DIR}/staging/isolinux/core.img > ${LIVE_BOOT_DIR}/staging/isolinux/bios.img

echo ">>> Creating Live CD ISO image"
xorriso -as mkisofs \
        -iso-level 3 \
        -volid "ENTRUSTED_LIVE" \
        -full-iso9660-filenames \
        -J -J -joliet-long \
        -output "${LIVE_ISO_DIR}/entrusted-livecd-${CPU_ARCH}-${ENTRUSTED_VERSION}.iso" \
        --grub2-mbr /usr/lib/grub/i386-pc/boot_hybrid.img \
        -partition_offset 16 \
        --mbr-force-bootable \
        -append_partition 2 28732ac11ff8d211ba4b00a0c93ec93b ${LIVE_BOOT_DIR}/staging/efiboot.img \
        -appended_part_as_gpt \-iso_mbr_part_type a2a0d0ebe5b9334487c068b6b72699c7 \
        -eltorito-boot isolinux/bios.img \
        -no-emul-boot \
        -boot-load-size 4 \
        -boot-info-table \
        --eltorito-catalog isolinux/boot.cat \
        --grub2-boot-info \
        -eltorito-alt-boot \
        -e '--interval:appended_partition_2:::' \
        -no-emul-boot "${LIVE_BOOT_DIR}/staging"

cd $PREVIOUSDIR
