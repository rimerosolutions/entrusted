#!/usr/bin/env sh

PREVIOUSDIR="$(echo $PWD)"
ROOT_SCRIPTDIR="$(realpath $(dirname "$0"))"
DEBIAN_VERSION="bookworm"
RUST_CI_VERSION="1.84.1"
ALPINE_VERSION="3.21.3"
GRUB_VERSION="2.06"

cd ${ROOT_SCRIPTDIR}

# # Grub
podman rmi --force docker.io/uycyjnzgntrn/grub:${GRUB_VERSION}-amd64
podman rmi --force docker.io/uycyjnzgntrn/grub:${GRUB_VERSION}-arm64
podman rmi --force docker.io/uycyjnzgntrn/grub:${GRUB_VERSION}

buildah manifest create docker.io/uycyjnzgntrn/grub:${GRUB_VERSION}

buildah bud --squash --platform=linux/amd64 --format docker -t docker.io/uycyjnzgntrn/grub:${GRUB_VERSION}-amd64 -f Dockerfile.grub.amd64 .
retVal=$?
if [ $retVal -ne 0 ]; then
    echo "Failure to create grub container image for amd64"
    exit 1
fi
buildah manifest add docker.io/uycyjnzgntrn/grub:${GRUB_VERSION} docker.io/uycyjnzgntrn/grub:${GRUB_VERSION}-amd64


buildah bud --squash --platform=linux/arm64 --format docker -t docker.io/uycyjnzgntrn/grub:${GRUB_VERSION}-arm64 -f Dockerfile.grub.arm64 .
retVal=$?
if [ $retVal -ne 0 ]; then
    echo "Failure to create grub container image for arm64"
    exit 1
fi
buildah manifest add docker.io/uycyjnzgntrn/grub:${GRUB_VERSION} docker.io/uycyjnzgntrn/grub:${GRUB_VERSION}-arm64

# # Windows for amd64
buildah bud --squash --platform=linux/amd64 --format docker -t docker.io/uycyjnzgntrn/rust-windows:${RUST_CI_VERSION} -f Dockerfile.windows.amd64 .

retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failed to build rust-windows container image"
  exit 1
fi

# # Mac OS
podman rmi --force docker.io/uycyjnzgntrn/rust-macos:${RUST_CI_VERSION}-amd64
podman rmi --force docker.io/uycyjnzgntrn/rust-macos:${RUST_CI_VERSION}-arm64
podman rmi --force docker.io/uycyjnzgntrn/rust-macos:${RUST_CI_VERSION}

buildah manifest create docker.io/uycyjnzgntrn/rust-macos:${RUST_CI_VERSION}

buildah bud --squash --platform=linux/amd64 --format docker -t docker.io/uycyjnzgntrn/rust-macos:${RUST_CI_VERSION}-amd64 -f Dockerfile.macos
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failed to build macos base container image for amd64"
  exit 1
fi
buildah manifest add docker.io/uycyjnzgntrn/rust-macos:${RUST_CI_VERSION} docker.io/uycyjnzgntrn/rust-macos:${RUST_CI_VERSION}-amd64

buildah bud --squash --platform=linux/arm64/v8 --format docker -t docker.io/uycyjnzgntrn/rust-macos:${RUST_CI_VERSION}-arm64 -f Dockerfile.macos
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failed to build macos base container image for arm64"
  exit 1
fi
buildah manifest add docker.io/uycyjnzgntrn/rust-macos:${RUST_CI_VERSION} docker.io/uycyjnzgntrn/rust-macos:${RUST_CI_VERSION}-arm64

# # Linux for amd64 and arm64
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

# # Processing Debian-based container for amd64 and arm64
podman rmi --force docker.io/uycyjnzgntrn/debian:${DEBIAN_VERSION}-rust-${RUST_CI_VERSION}-amd64-tesseract5
podman rmi --force docker.io/uycyjnzgntrn/debian:${DEBIAN_VERSION}-rust-${RUST_CI_VERSION}-arm64-tesseract5
podman rmi --force docker.io/uycyjnzgntrn/debian:${DEBIAN_VERSION}-rust-${RUST_CI_VERSION}-tesseract5

buildah manifest create docker.io/uycyjnzgntrn/debian:${DEBIAN_VERSION}-rust-${RUST_CI_VERSION}-tesseract5

buildah bud --squash --platform=linux/amd64 --format docker -t docker.io/uycyjnzgntrn/debian:${DEBIAN_VERSION}-rust-${RUST_CI_VERSION}-amd64-tesseract5 -f Dockerfile.container.builder .
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failed to build debian base container image for amd64"
  exit 1
fi
buildah manifest add docker.io/uycyjnzgntrn/debian:${DEBIAN_VERSION}-rust-${RUST_CI_VERSION}-tesseract5 docker.io/uycyjnzgntrn/debian:${DEBIAN_VERSION}-rust-${RUST_CI_VERSION}-amd64-tesseract5

buildah bud --squash --platform=linux/arm64/v8 --format docker -t docker.io/uycyjnzgntrn/debian:${DEBIAN_VERSION}-rust-${RUST_CI_VERSION}-arm64-tesseract5 -f Dockerfile.container.builder .
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failed to build debian base container image for arm64"
  exit 1
fi
buildah manifest add docker.io/uycyjnzgntrn/debian:${DEBIAN_VERSION}-rust-${RUST_CI_VERSION}-tesseract5 docker.io/uycyjnzgntrn/debian:${DEBIAN_VERSION}-rust-${RUST_CI_VERSION}-arm64-tesseract5

# # Processing alpine-based container for amd64 and arm64
podman rmi --force docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION}-amd64
podman rmi --force docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION}-arm64
podman rmi --force docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION}

buildah manifest create docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION}

buildah bud --squash --platform=linux/amd64 --format docker -t docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION}-amd64 -f Dockerfile.alpine .
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failed to build alpine base container image for amd64"
  exit 1
fi
buildah manifest add docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION} docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION}-amd64

buildah bud --squash --platform=linux/arm64/v8 --format docker -t docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION}-arm64 -f Dockerfile.alpine .
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failed to build alpine base container image for arm64"
  exit 1
fi
buildah manifest add docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION} docker.io/uycyjnzgntrn/alpine:${ALPINE_VERSION}-arm64

cd ${PREVIOUSDIR}
