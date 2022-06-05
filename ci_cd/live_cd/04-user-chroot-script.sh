#!/usr/bin/env sh
set -x

DANGERZONE_VERSION=$(cat /etc/dangerzone_release | head -1)

mkdir -p $HOME/.config/containers

cat /files/home/dangerzone/.bashrc_append >> ~/.bashrc

podman run --rm docker-archive:/files/dangerzone-container.tar "docker.io/uycyjnzgntrn/dangerzone-converter:${DANGERZONE_VERSION}" cat /etc/os-release
