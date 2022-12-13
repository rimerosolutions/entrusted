#!/usr/bin/env sh
set -x

THIS_SCRIPTS_DIR="$(realpath $(dirname "$0"))"

ENTRUSTED_VERSION=$1
CONTAINER_USER=$2

echo "It is assumed that you published the entrusted container image to Docker Hub already..."
test -d /tmp/entrusted-armpackaging &&  sudo rm -rf /tmp/entrusted-armpackaging
sudo killall -u entrusted
sudo userdel -r "${CONTAINER_USER}"
sudo useradd -ms /bin/bash "${CONTAINER_USER}"
sudo adduser "${CONTAINER_USER}" sudo

cd /

sudo -u "${CONTAINER_USER}" bash -c "mkdir -p /home/${CONTAINER_USER}/.config/containers/"
sudo cp "${THIS_SCRIPTS_DIR}"/arm_files/podman_storage.conf /home/"${CONTAINER_USER}"/.config/containers/storage.conf
sudo chown entrusted:entrusted /home/"${CONTAINER_USER}"/.config/containers/storage.conf
sudo -u "${CONTAINER_USER}" bash -c "podman pull --platform linux/${DEBIAN_ARCH}  docker.io/uycyjnzgntrn/entrusted_container:${ENTRUSTED_VERSION}"

sudo -u "${CONTAINER_USER}" bash -c "mkdir -p /tmp/entrusted-armpackaging"
sudo -u "${CONTAINER_USER}" bash -c "cp -r /home/${CONTAINER_USER}/.local/share/containers /tmp/entrusted-armpackaging/"

sudo find /tmp/entrusted-armpackaging/containers -type d -name "entrusted" -exec chmod -R a+rw {} \;
sudo find /tmp/entrusted-armpackaging/containers -type d -name "safezone"  -exec chmod -R a+rw {} \;
sudo find /tmp/entrusted-armpackaging/containers -type d -name "tmp"       -exec chmod -R a+rw {} \;

cd -
