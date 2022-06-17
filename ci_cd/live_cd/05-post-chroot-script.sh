#!/usr/bin/env sh
set -x

ROOT_SCRIPTS_DIR="$(realpath $(dirname "$0"))"

echo "Deleting previous artifacts ISO and squashfs files"
ENTRUSTED_VERSION=$(cat $HOME/LIVE_BOOT/chroot/etc/entrusted_release | head -1)
test -f $HOME/LIVE_BOOT/entrusted-livecd-amd64-${ENTRUSTED_VERSION}.iso && rm $HOME/LIVE_BOOT/entrusted-livecd-amd64-${ENTRUSTED_VERSION}.iso
test -f $HOME/LIVE_BOOT/staging/live/filesystem.squashfs && sudo rm $HOME/LIVE_BOOT/staging/live/filesystem.squashfs

echo "Creating filesystem"
mkdir -p $HOME/LIVE_BOOT/staging/EFI/boot/
mkdir -p $HOME/LIVE_BOOT/staging/boot/grub/x86_64-efi                          
mkdir -p $HOME/LIVE_BOOT/staging/isolinux
mkdir -p $HOME/LIVE_BOOT/staging/live
mkdir -p $HOME/LIVE_BOOT/tmp
sudo mksquashfs $HOME/LIVE_BOOT/chroot $HOME/LIVE_BOOT/staging/live/filesystem.squashfs -e boot

echo "Preparing boot files"
cp $HOME/LIVE_BOOT/chroot/boot/vmlinuz-* $HOME/LIVE_BOOT/staging/live/vmlinuz
cp $HOME/LIVE_BOOT/chroot/boot/initrd.img-* $HOME/LIVE_BOOT/staging/live/initrd

cp "${ROOT_SCRIPTS_DIR}"/post_chroot_files/home/entrusted/LIVE_BOOT/staging/isolinux/isolinux.cfg $HOME/LIVE_BOOT/staging/isolinux/isolinux.cfg
cp "${ROOT_SCRIPTS_DIR}"/post_chroot_files/home/entrusted/LIVE_BOOT/staging/boot/grub/grub.cfg    $HOME/LIVE_BOOT/staging/boot/grub/grub.cfg
cp "${ROOT_SCRIPTS_DIR}"/post_chroot_files/home/entrusted/LIVE_BOOT/tmp/grub-standalone.cfg       $HOME/LIVE_BOOT/tmp/grub-standalone.cfg

touch $HOME/LIVE_BOOT/staging/DEBIAN_CUSTOM

cp /usr/lib/ISOLINUX/isolinux.bin  $HOME/LIVE_BOOT/staging/isolinux/ \
    && cp /usr/lib/syslinux/modules/bios/* $HOME/LIVE_BOOT/staging/isolinux/
cp -r /usr/lib/grub/x86_64-efi/* $HOME/LIVE_BOOT/staging/boot/grub/x86_64-efi/

grub-mkstandalone --format=x86_64-efi --output=$HOME/LIVE_BOOT/tmp/bootx64.efi --locales= --fonts= boot/grub/grub.cfg=$HOME/LIVE_BOOT/tmp/grub-standalone.cfg
(cd $HOME/LIVE_BOOT/staging/EFI/boot && dd if=/dev/zero of=efiboot.img bs=1M count=20 && /sbin/mkfs.vfat efiboot.img && mmd -i efiboot.img efi efi/boot && mcopy -vi efiboot.img $HOME/LIVE_BOOT/tmp/bootx64.efi ::efi/boot/)

echo "Creating Live CD ISO image"
xorriso -as mkisofs -iso-level 3 -o $HOME/LIVE_BOOT/entrusted-livecd-amd64-${ENTRUSTED_VERSION}.iso -full-iso9660-filenames -volid DEBIAN_CUSTOM -isohybrid-mbr /usr/lib/ISOLINUX/isohdpfx.bin  -eltorito-boot isolinux/isolinux.bin -no-emul-boot -boot-load-size 4 -boot-info-table --eltorito-catalog isolinux/isolinux.cat -eltorito-alt-boot -e /EFI/boot/efiboot.img -no-emul-boot -isohybrid-gpt-basdat --append_partition 2 0xef $HOME/LIVE_BOOT/staging/EFI/boot/efiboot.img $HOME/LIVE_BOOT/staging
