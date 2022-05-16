#!/usr/bin/env bash

DANGERZONE_VERSION=$(cat /etc/dangerzone_release | head -1)

mkdir -p $HOME/.config/containers
cp /files/home/dangerzone/.config/containers/containers.conf $HOME/.config/containers/containers.conf
cp /files/usr/share/containers/containers.conf /usr/share/containers/containers.conf

cd /tmp
wget https://github.com/rimerosolutions/dangerzone-rust/releases/download/${DANGERZONE_VERSION}/dangerzone-linux-amd64-${DANGERZONE_VERSION}.tar
tar xf dangerzone-linux-amd64-${DANGERZONE_VERSION}.tar && cd dangerzone-linux-amd64-${DANGERZONE_VERSION}

cat /files/home/dangerzone/.bashrc_append >> ~/.bashrc

podman pull docker.io/uycyjnzgntrn/dangerzone-converter:${DANGERZONE_VERSION}
