#!/usr/bin/env sh
set -x
ENTRUSTED_VERSION=$1
test -f /tmp/live-entrusted-container.tar && rm /tmp/live-entrusted-container.tar
podman image prune -f --filter label=stage=entrusted_container_builder
podman save -m  -o /tmp/live-entrusted-container.tar "docker.io/uycyjnzgntrn/entrusted_container:${ENTRUSTED_VERSION}"
