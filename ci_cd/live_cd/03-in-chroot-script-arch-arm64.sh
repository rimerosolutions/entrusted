#!/usr/bin/env sh
set -x

ENTRUSTED_VERSION=$1
runuser -l entrusted -c "mkdir -p /home/entrusted/.local/share"
mv /files/entrusted-armpackaging/containers /home/entrusted/.local/share/
chown -R entrusted:entrusted /home/entrusted/.local/share

