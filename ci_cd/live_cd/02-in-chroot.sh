#!/usr/bin/env sh
set -x

ENTRUSTED_VERSION=$(cat /etc/entrusted_release | head -1)
ENTRUSTED_ARCH=$(cat /etc/entrusted_arch | head -1)
ENTRUSTED_USERNAME=$(cat /files/entrusted_username | head -1)
ENTRUSTED_USERID=$(cat /files/entrusted_userid | head -1)

echo ">>> Setting up hostname"
echo "entrusted-livecd" > /etc/hostname

echo ">>> Updating apt retries to 10"
echo 'Acquire::Retries "10";' > /etc/apt/apt.conf.d/80-retries

echo ">>> Installing custom kernel"
DEBIAN_FRONTEND=noninteractive apt update
dpkg -i /files/minikernel/linux-image*.deb
DEBIAN_FRONTEND=noninteractive apt install -y --no-install-recommends initramfs-tools
cd /boot && initrdsuffix=$(ls vmlinuz-* | awk -F"vmlinuz-" '{print $2}') && cd -
cd /boot && mkinitramfs -o initrd.img-${initrdsuffix} ${initrdsuffix} && cd -

echo ">>> Installing default packages"
DEBIAN_FRONTEND=noninteractive apt install -y --no-install-recommends \
    auditd \
    iptables-persistent \
    doas \
    uidmap \
    dbus-user-session \
    fuse-overlayfs \
    ca-certificates \
    locales \
    network-manager \
    net-tools \
    mg \
    dropbear \
    crun \
    live-boot \
    systemd-sysv \
    && apt clean

echo ">>> Installing podman-static"
tar zxvf /files/podman/podman*.tar.gz --strip-components 1 --exclude="README.md" --exclude="fuse-overlayfs" --exclude="fusermount3" -C /
rm /usr/local/bin/runc

echo ">>> Setting up system files"
cp /files/etc/iptables/rules.v4 /etc/iptables/
cp /files/etc/doas.conf /etc/ && chmod 400 /etc/doas.conf
cp /files/etc/security/limits.conf /etc/security/
cp /files/etc/systemd/system/*.service /etc/systemd/system/
cp -r /files/etc/systemd/coredump.conf.d /etc/systemd/

echo ">>> Creating ${ENTRUSTED_USERNAME} user"
useradd -m -s /bin/bash -u ${ENTRUSTED_USERID} ${ENTRUSTED_USERNAME}
adduser ${ENTRUSTED_USERNAME} sudo

echo ">>> Creating entrusted user files and pulling container image"
runuser -l ${ENTRUSTED_USERNAME} -c "mkdir -p /home/${ENTRUSTED_USERNAME}/.config/containers /home/${ENTRUSTED_USERNAME}/.local/share"
runuser -l ${ENTRUSTED_USERNAME} -c "cat /files/home/entrusted/.bash_profile >> /home/${ENTRUSTED_USERNAME}/.bashrc"
runuser -l ${ENTRUSTED_USERNAME} -c "cat /files/home/entrusted/.bash_profile >> /home/${ENTRUSTED_USERNAME}/.bash_profile"
runuser -l ${ENTRUSTED_USERNAME} -c "cat /files/home/entrusted/.config/containers/containers.conf >> /home/${ENTRUSTED_USERNAME}/.config/containers/containers.conf"
mv /files/entrusted-packaging/containers /home/${ENTRUSTED_USERNAME}/.local/share/
chown -R ${ENTRUSTED_USERNAME}:${ENTRUSTED_USERNAME} /home/${ENTRUSTED_USERNAME}/.local/share

find /home/${ENTRUSTED_USERNAME}/.local/share/containers -type d -name "${ENTRUSTED_USERNAME}" -exec chmod -R a+rw {} \;
find /home/${ENTRUSTED_USERNAME}/.local/share/containers -type d -name "safezone"              -exec chmod -R a+rw {} \;
find /home/${ENTRUSTED_USERNAME}/.local/share/containers -type d -name "tmp"                   -exec chmod -R a+rw {} \;

echo ">>> Copying entrusted binaries"
mv /files/entrusted-webserver /files/entrusted-cli /usr/local/bin
cp /files/usr/local/bin/entrusted-* /usr/local/bin/
chmod +x /usr/local/bin/entrusted-*
cp /files/libhardened_malloc.so /usr/lib/
mkdir -p /var/log/entrusted-webserver

echo ">>> Copying gvisor binaries"
cp -r /files/gvisor/* /usr/local/bin/
cp /files/usr/local/bin/runsc-podman /usr/local/bin/
chmod +x /usr/local/bin/*

echo ">>> Updating default screen messages"
cp /files/etc/motd /etc/motd
cp /files/etc/issue /etc/issue

echo ">>> Updating linger to allows users who aren't logged in to run long-running services."
echo "This also allows the automatic creation of /run/user/NUMERIC_USER_ID as tmpdir for podman"
loginctl enable-linger $ENTRUSTED_USERNAME
mkdir -p /run/user/$(id -u $ENTRUSTED_USERNAME)
chown -R ${ENTRUSTED_USERNAME}:${ENTRUSTED_USERNAME} /run/user/$(id -u ${ENTRUSTED_USERNAME})

echo ">>> Updating passwords"
echo "root:root" | /usr/sbin/chpasswd
echo "${ENTRUSTED_USERNAME}:${ENTRUSTED_USERNAME}" | /usr/sbin/chpasswd

echo ">>> Enabling default services"
systemctl enable dropbear
systemctl enable NetworkManager
systemctl enable netfilter-persistent
systemctl enable systemd-networkd
systemctl enable entrusted-init
systemctl enable entrusted-webserver

echo "apm power_off=1" >> /etc/modules

# See https://madaidans-insecurities.github.io/guides/linux-hardening.html
# See https://www.pluralsight.com/blog/it-ops/linux-hardening-secure-server-checklist
echo ">>> Hardening kernel"

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

echo ">>> Updating machine-id"
echo "b08dfa6083e7567a1921a715000001fb" > /var/lib/dbus/machine-id

echo ">>> Disabling SSH root login"
perl -pi -e 's/^DROPBEAR_EXTRA_ARGS.*/DROPBEAR_EXTRA_ARGS="-w -g"/' /etc/default/dropbear

# See https://github.com/juju4/ansible-harden-systemd
# See https://github.com/krathalan/systemd-sandboxing
echo ">>> Harden few systemd services"
cp /files/lib/systemd/system/*.service /lib/systemd/system/
chown root:root /lib/systemd/system/NetworkManager.service
chown root:root /lib/systemd/system/auditd.service
chown root:root /lib/systemd/system/dbus.service
chown root:root /lib/systemd/system/rc-local.service
chown root:root /lib/systemd/system/wpa_supplicant.service
chmod 0644 /lib/systemd/system/NetworkManager.service
chmod 0644 /lib/systemd/system/auditd.service
chmod 0644 /lib/systemd/system/dbus.service
chmod 0644 /lib/systemd/system/rc-local.service
chmod 0644 /lib/systemd/system/wpa_supplicant.service

# See https://github.com/konstruktoid/hardening
echo ">>> Adjust login.defs"
perl -pi -e 's/^UMASK.*/UMASK 077/' /etc/login.defs
perl -pi -e 's/^.*LOG_OK_LOGINS.*/LOG_OK_LOGINS yes/' /etc/login.defs

echo ">>> Apply default umask in master profile"
echo "umask 077" >> /etc/profile

echo ">>> Apply seccomp rules to package manager"
echo 'APT::Sandbox::Seccomp "1";' | tee /etc/apt/apt.conf.d/99seccomp

echo ">>> Delete few default users"
userdel -r games 
userdel -r gnats 
userdel -r irc 
userdel -r list 
userdel -r news 
userdel -r sync 
userdel -r uucp

echo ">>> Trim filesystem"
mkdir -p /tmp/locales && cp -rf /usr/share/locale/locale.alias /usr/share/locale/en_CA /tmp/locales
rm -rf /usr/share/locale/* && mv /tmp/locales/* /usr/share/locale/
rm -rf /usr/share/common-licences
rm -rf /usr/share/man
rm -rf /usr/share/pixmaps
rm -rf /usr/share/doc*
rm -rf /usr/share/info
rm -rf /var/cache/apt/*
rm -rf /tmp/*
rm -rf /run/user/*
rm -rf /var/log/* && mkdir -p /var/log/entrusted-webserver /var/log/audit

echo ">>> Cleanup chroot files"
rm -rf /files

exit
