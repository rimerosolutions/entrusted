#!/usr/bin/env sh
set -x

DEBIAN_ARCH=$1
ROOT_SCRIPTS_DIR="$(realpath $(dirname "$0"))"

EFI_ARCH="x86_64"
BOOT_EFI_ARCH="x64"
if [ ${DEBIAN_ARCH} != "amd64" ]
then
    EFI_ARCH="arm64"
    BOOT_EFI_ARCH="aa64"
fi

echo "Deleting previous artifacts ISO and squashfs files"
ENTRUSTED_VERSION=$(cat $HOME/LIVE_BOOT-${DEBIAN_ARCH}/chroot/etc/entrusted_release | head -1)
test -f $HOME/LIVE_BOOT-${DEBIAN_ARCH}/entrusted-livecd-${DEBIAN_ARCH}-${ENTRUSTED_VERSION}.iso && rm $HOME/LIVE_BOOT-${DEBIAN_ARCH}/entrusted-livecd-${DEBIAN_ARCH}-${ENTRUSTED_VERSION}.iso
test -f $HOME/LIVE_BOOT-${DEBIAN_ARCH}/staging/live/filesystem.squashfs && sudo rm $HOME/LIVE_BOOT-${DEBIAN_ARCH}/staging/live/filesystem.squashfs

echo "Creating filesystem"
mkdir -p $HOME/LIVE_BOOT-${DEBIAN_ARCH}/staging/EFI/boot/
mkdir -p $HOME/LIVE_BOOT-${DEBIAN_ARCH}/staging/boot/grub/${EFI_ARCH}-efi                          
mkdir -p $HOME/LIVE_BOOT-${DEBIAN_ARCH}/staging/isolinux
mkdir -p $HOME/LIVE_BOOT-${DEBIAN_ARCH}/staging/live
mkdir -p $HOME/LIVE_BOOT-${DEBIAN_ARCH}/tmp

sudo mksquashfs $HOME/LIVE_BOOT-${DEBIAN_ARCH}/chroot $HOME/LIVE_BOOT-${DEBIAN_ARCH}/staging/live/filesystem.squashfs -e boot

echo "Preparing boot files"
cp $HOME/LIVE_BOOT-${DEBIAN_ARCH}/chroot/boot/vmlinuz-* $HOME/LIVE_BOOT-${DEBIAN_ARCH}/staging/live/vmlinuz
cp $HOME/LIVE_BOOT-${DEBIAN_ARCH}/chroot/boot/initrd.img-* $HOME/LIVE_BOOT-${DEBIAN_ARCH}/staging/live/initrd

cp "${ROOT_SCRIPTS_DIR}"/post_chroot_files/home/entrusted/LIVE_BOOT/staging/isolinux/isolinux.cfg $HOME/LIVE_BOOT-${DEBIAN_ARCH}/staging/isolinux/isolinux.cfg
cp "${ROOT_SCRIPTS_DIR}"/post_chroot_files/home/entrusted/LIVE_BOOT/staging/boot/grub/grub.cfg    $HOME/LIVE_BOOT-${DEBIAN_ARCH}/staging/boot/grub/grub.cfg
cp "${ROOT_SCRIPTS_DIR}"/post_chroot_files/home/entrusted/LIVE_BOOT/tmp/grub-standalone.cfg       $HOME/LIVE_BOOT-${DEBIAN_ARCH}/tmp/grub-standalone.cfg

touch $HOME/LIVE_BOOT-${DEBIAN_ARCH}/staging/DEBIAN_CUSTOM

cp /usr/lib/ISOLINUX/isolinux.bin  $HOME/LIVE_BOOT-${DEBIAN_ARCH}/staging/isolinux/ \
    && cp /usr/lib/syslinux/modules/bios/* $HOME/LIVE_BOOT-${DEBIAN_ARCH}/staging/isolinux/
cp -r $HOME/LIVE_BOOT-${DEBIAN_ARCH}/chroot/usr/lib/grub/${EFI_ARCH}-efi/* $HOME/LIVE_BOOT-${DEBIAN_ARCH}/staging/boot/grub/${EFI_ARCH}-efi/

grub-mkstandalone --format=${EFI_ARCH}-efi --output=$HOME/LIVE_BOOT-${DEBIAN_ARCH}/tmp/boot${BOOT_EFI_ARCH}.efi --locales= --fonts= boot/grub/grub.cfg=$HOME/LIVE_BOOT-${DEBIAN_ARCH}/tmp/grub-standalone.cfg
(cd $HOME/LIVE_BOOT-${DEBIAN_ARCH}/staging/EFI/boot && dd if=/dev/zero of=efiboot.img bs=1M count=20 && /sbin/mkfs.vfat efiboot.img && mmd -i efiboot.img efi efi/boot && mcopy -vi efiboot.img $HOME/LIVE_BOOT-${DEBIAN_ARCH}/tmp/boot${BOOT_EFI_ARCH}.efi ::efi/boot/)

echo "Creating Live CD ISO image"
xorriso -as mkisofs -iso-level 3 -o $HOME/LIVE_BOOT-${DEBIAN_ARCH}/entrusted-livecd-${DEBIAN_ARCH}-${ENTRUSTED_VERSION}.iso -full-iso9660-filenames -volid DEBIAN_CUSTOM -isohybrid-mbr /usr/lib/ISOLINUX/isohdpfx.bin  -eltorito-boot isolinux/isolinux.bin -no-emul-boot -boot-load-size 4 -boot-info-table --eltorito-catalog isolinux/isolinux.cat -eltorito-alt-boot -e /EFI/boot/efiboot.img -no-emul-boot -isohybrid-gpt-basdat --append_partition 2 0xef $HOME/LIVE_BOOT-${DEBIAN_ARCH}/staging/EFI/boot/efiboot.img $HOME/LIVE_BOOT-${DEBIAN_ARCH}/staging
