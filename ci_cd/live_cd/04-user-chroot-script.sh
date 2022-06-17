#!/usr/bin/env sh
set -x

ENTRUSTED_VERSION=$(cat /etc/entrusted_release | head -1)

mkdir -p $HOME/.config/containers

cat /files/home/entrusted/.bashrc_append >> ~/.bashrc

podman run --rm docker-archive:/files/entrusted-container.tar "docker.io/uycyjnzgntrn/entrusted_container:${ENTRUSTED_VERSION}" cat /etc/os-release
