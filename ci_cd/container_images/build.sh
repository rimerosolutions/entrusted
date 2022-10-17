#!/usr/bin/env sh

PREVIOUSDIR="$(echo $PWD)"
ROOT_SCRIPTDIR="$(realpath $(dirname "$0"))"
ALPINE_VERSION="3.16.2"
RUST_CI_VERSION="1.64.0"

cd ${ROOT_SCRIPTDIR}

# Windows for amd64
podman build -t docker.io/uycyjnzgntrn/rust-windows:${RUST_CI_VERSION} -f Dockerfile.windows.amd64 .

retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failed to build rust-windows container image"
  exit 1
fi

# Mac OS
podman build -t docker.io/uycyjnzgntrn/rust-macos:${RUST_CI_VERSION} -f Dockerfile.macos.amd64 .

retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failed to build rust-macos container image"
  exit 1
fi
# Linux for amd64 and arm64
podman rmi --force docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION}-amd64
podman rmi --force docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION}-arm64
podman rmi --force docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION}

buildah manifest create docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION}

buildah bud --squash --platform=linux/amd64 --format docker -t docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION}-amd64 -f Dockerfile.linux.amd64 .
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failed to build rust-linux container image for amd64"
  exit 1
fi
buildah manifest add docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION} docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION}-amd64

buildah bud --squash --platform=linux/arm64/v8 --format docker -t docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION}-arm64 -f Dockerfile.linux.arm64 .
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failed to build rust-linux container image for arm64"
  exit 1
fi
buildah manifest add docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION} docker.io/uycyjnzgntrn/rust-linux:${RUST_CI_VERSION}-arm64

# Processing Alpine-based container for amd64 and arm64
podman rmi --force docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION}-amd64
podman rmi --force docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION}-arm64
podman rmi --force docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION}

buildah manifest create docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION}

buildah bud --squash --platform=linux/amd64 --format docker -t docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION}-amd64 -f Dockerfile.container.builder .
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failed to build alpine base container image for amd64"
  exit 1
fi
buildah manifest add docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION} docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION}-amd64

buildah bud --squash --platform=linux/arm64/v8 --format docker -t docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION}-arm64 -f Dockerfile.container.builder .
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failed to build alpine base container image for arm64"
  exit 1
fi
buildah manifest add docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION} docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION}-arm64

cd ${PREVIOUSDIR}
