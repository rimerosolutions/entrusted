#!/usr/bin/env sh
set -x

ENTRUSTED_VERSION=$(cat /etc/entrusted_release | head -1)

echo "Setting up hostname"
echo "entrusted-livecd" > /etc/hostname

echo "Installing default packages"
export DEBIAN_FRONTEND=noninteractive

apt update && \
    apt install -y \
    linux-image-amd64 \
    auditd \
    iptables-persistent \
    doas \
    uidmap \
    dbus-user-session \
    slirp4netns \
    fuse-overlayfs \
    ca-certificates \
    curl \
    wget \
    locales \
    network-manager \
    net-tools \
    mg \
    openssh-sftp-server \
    openssh-server \
    podman \
    live-boot \
    systemd-sysv

apt clean

echo "Setting up system files"
cp /files/etc/iptables/rules.v4 /etc/iptables/
cp /files/etc/doas.conf /etc/ && chmod 400 /etc/doas.conf
cp /files/etc/systemd/system/entrusted-webserver.service /etc/systemd/system/

echo "Creating entrusted user"
useradd -ms /bin/bash entrusted
usermod -G sudo entrusted

echo "Creating entrusted user files and pulling container image"
/usr/sbin/runuser -l entrusted -c "/files/04-user-chroot-script.sh ${ENTRUSTED_VERSION}"

echo "Copying entrusted binaries"
mv /files/entrusted-webserver /files/entrusted-cli /usr/local/bin
cp /files/usr/local/bin/entrusted-fw-enable /usr/local/bin/entrusted-fw-enable
cp /files/usr/local/bin/entrusted-fw-disable /usr/local/bin/entrusted-fw-disable
chmod +x /usr/local/bin/entrusted-*

echo "Updating default screen messages"
cp /files/etc/motd /etc/motd
cp /files/etc/issue /etc/issue
cp /files/usr/share/containers/containers.conf /usr/share/containers/containers.conf

echo "Updating passwords"
echo 'root:root' | /usr/sbin/chpasswd
echo 'entrusted:entrusted' | /usr/sbin/chpasswd

echo "Enabling default services"
systemctl enable ssh
systemctl enable NetworkManager
systemctl enable netfilter-persistent
systemctl enable systemd-networkd
systemctl enable entrusted-webserver

rm -rf /files

echo "apm power_off=1" >> /etc/modules

exit
