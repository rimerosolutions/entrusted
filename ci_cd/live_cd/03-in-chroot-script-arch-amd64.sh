#!/usr/bin/env sh
set -x
ENTRUSTED_VERSION=$1
runuser -l entrusted -c "/files/04-user-chroot-script.sh ${ENTRUSTED_VERSION}"
