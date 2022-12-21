#!/usr/bin/env sh
set -x

ENTRUSTED_VERSION=$(cat /etc/entrusted_release | head -1)
ENTRUSTED_ARCH=$(cat /etc/entrusted_arch | head -1)
ENTRUSTED_USERNAME=$(cat /files/entrusted_username | head -1)
ENTRUSTED_USERID=$(cat /files/entrusted_userid | head -1)

echo "Setting up hostname"
echo "entrusted-livecd" > /etc/hostname

echo "Installing default packages"
DEBIAN_FRONTEND=noninteractive apt update && \
    DEBIAN_FRONTEND=noninteractive apt install -y --no-install-recommends \
    linux-image-${ENTRUSTED_ARCH} \
    auditd \
    iptables-persistent \
    doas \
    uidmap \
    dbus-user-session \
    slirp4netns \
    fuse-overlayfs \
    ca-certificates \
    locales \
    network-manager \
    net-tools \
    mg \
    openssh-sftp-server \
    openssh-server \
    podman \
    live-boot \
    syslinux-efi \
    grub-efi-${ENTRUSTED_ARCH}-bin \
    systemd-sysv

apt clean

echo "Setting up system files"
cp /files/etc/iptables/rules.v4 /etc/iptables/
cp /files/etc/doas.conf /etc/ && chmod 400 /etc/doas.conf
cp /files/etc/security/limits.conf /etc/security/
cp /files/etc/systemd/system/entrusted-webserver.service /etc/systemd/system/
cp -r /files/etc/systemd/coredump.conf.d /etc/systemd/

echo "Creating ${ENTRUSTED_USERNAME} user"
useradd -m -s /bin/bash -u ${ENTRUSTED_USERID} ${ENTRUSTED_USERNAME}
adduser ${ENTRUSTED_USERNAME} sudo

echo "Creating entrusted user files and pulling container image"
runuser -l ${ENTRUSTED_USERNAME} -c "mkdir -p /home/${ENTRUSTED_USERNAME}/.local/share"
mv /files/entrusted-packaging/containers /home/${ENTRUSTED_USERNAME}/.local/share/
chown -R ${ENTRUSTED_USERNAME}:${ENTRUSTED_USERNAME} /home/${ENTRUSTED_USERNAME}/.local/share

find /home/${ENTRUSTED_USERNAME}/.local/share/containers -type d -name "${ENTRUSTED_USERNAME}" -exec chmod -R a+rw {} \;
find /home/${ENTRUSTED_USERNAME}/.local/share/containers -type d -name "safezone"              -exec chmod -R a+rw {} \;
find /home/${ENTRUSTED_USERNAME}/.local/share/containers -type d -name "tmp"                   -exec chmod -R a+rw {} \;

echo "Copying entrusted binaries"
mv /files/entrusted-webserver /files/entrusted-cli /usr/local/bin
cp /files/usr/local/bin/entrusted-fw-enable /usr/local/bin/entrusted-fw-enable
cp /files/usr/local/bin/entrusted-fw-disable /usr/local/bin/entrusted-fw-disable
chmod +x /usr/local/bin/entrusted-*

cp /files/libhardened_malloc.so /usr/lib/
mkdir -p /var/log/entrusted-webserver

echo "Updating default screen messages"
cp /files/etc/motd /etc/motd
cp /files/etc/issue /etc/issue
cp /files/usr/share/containers/containers.conf /usr/share/containers/containers.conf

echo "Updating linger to allows users who aren't logged in to run long-running services."
echo "This also allows the automatic creation of /run/user/NUMERIC_USER_ID as tmpdir for podman"
loginctl enable-linger ${ENTRUSTED_USERNAME}

echo "Updating passwords"
echo "root:root" | /usr/sbin/chpasswd
echo "${ENTRUSTED_USERNAME}:${ENTRUSTED_USERNAME}" | /usr/sbin/chpasswd

echo "Enabling default services"
systemctl enable ssh
systemctl enable NetworkManager
systemctl enable netfilter-persistent
systemctl enable systemd-networkd
systemctl enable entrusted-webserver

rm -rf /files

echo "apm power_off=1" >> /etc/modules

# See https://madaidans-insecurities.github.io/guides/linux-hardening.html
# See https://www.pluralsight.com/blog/it-ops/linux-hardening-secure-server-checklist
echo "Hardening kernel systems"

echo "kernel.core_pattern=|/bin/false" >> /etc/sysctl.conf
echo "vm.swappiness=1" >> /etc/sysctl.conf
echo "fs.suid_dumpable=0" >> /etc/sysctl.conf

echo "kernel.randomize_va_space=1" >> /etc/sysctl.conf
echo "kernel.kptr_restrict=2" >> /etc/sysctl.conf
echo "kernel.dmesg_restrict=1" >> /etc/sysctl.conf
echo "kernel.printk=3 3 3 3" >> /etc/sysctl.conf
echo "kernel.unprivileged_bpf_disabled=1" >> /etc/sysctl.conf
echo "net.core.bpf_jit_harden=2" >> /etc/sysctl.conf
echo "kernel.kexec_load_disabled=1" >> /etc/sysctl.conf
echo "vm.unprivileged_userfaultfd=0" >> /etc/sysctl.conf
echo "kernel.sysrq=4" >> /etc/sysctl.conf
echo "dev.tty.ldisc_autoload=0" >> /etc/sysctl.conf
echo "kernel.perf_event_paranoid=2" >> /etc/sysctl.conf

echo "net.ipv4.tcp_syncookies=1" >> /etc/sysctl.conf
echo "net.ipv4.tcp_rfc1337=1" >> /etc/sysctl.conf
echo "net.ipv4.conf.all.rp_filter=1" >> /etc/sysctl.conf
echo "net.ipv4.conf.default.rp_filter=1" >> /etc/sysctl.conf
echo "net.ipv4.icmp_echo_ignore_all=1" >> /etc/sysctl.conf
echo "net.ipv4.conf.all.accept_source_route=0" >> /etc/sysctl.conf
echo "net.ipv4.conf.default.accept_source_route=0" >> /etc/sysctl.conf
echo "net.ipv6.conf.all.accept_source_route=0" >> /etc/sysctl.conf
echo "net.ipv6.conf.default.accept_source_route=0" >> /etc/sysctl.conf
echo "net.ipv6.conf.all.accept_ra=0" >> /etc/sysctl.conf
echo "net.ipv6.conf.default.accept_ra=0" >> /etc/sysctl.conf
echo "net.ipv4.tcp_sack=0" >> /etc/sysctl.conf
echo "net.ipv4.tcp_dsack=0" >> /etc/sysctl.conf
echo "net.ipv4.tcp_fack=0" >> /etc/sysctl.conf

echo "kernel.yama.ptrace_scope=2" >> /etc/sysctl.conf
echo "vm.mmap_rnd_bits=32" >> /etc/sysctl.conf
echo "vm.mmap_rnd_compat_bits=16" >> /etc/sysctl.conf
echo "fs.protected_symlinks=1" >> /etc/sysctl.conf
echo "fs.protected_hardlinks=1" >> /etc/sysctl.conf
echo "fs.protected_fifos=2" >> /etc/sysctl.conf
echo "fs.protected_regular=2" >> /etc/sysctl.conf

echo "Hardening SSH configuration"

echo "PermitEmptyPasswords no" >> /etc/ssh/sshd_config
echo "PermitRootLogin no" >> /etc/ssh/sshd_config
echo "Protocol 2" >> /etc/ssh/sshd_config
echo "X11Forwarding no" >> /etc/ssh/sshd_config
echo "ClientAliveInterval 300" >> /etc/ssh/sshd_config
echo "ClientAliveCountMax 0" >> /etc/ssh/sshd_config

echo "b08dfa6083e7567a1921a715000001fb" > /var/lib/dbus/machine-id

echo "Trim filesystem"
rm -rf /usr/share/man/* /usr/share/doc/* /usr/share/info/* /var/cache/apt/* /var/log/*

echo "Ensure that we don't have weird permission issues with tmp"
rm -rf /tmp/* && chmod -R a+rw /tmp

exit
