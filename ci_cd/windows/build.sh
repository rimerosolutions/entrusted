#!/usr/bin/env sh
set -x

PREVIOUSDIR="$(echo $PWD)"
SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${SCRIPTDIR}/../..)"
APPVERSION=$(awk -F ' = ' '$1 ~ /version/ { gsub(/[\"]/, "", $2); printf("%s",$2) }' ${PROJECTDIR}/dangerzone-client/Cargo.toml)
ARTIFACTSDIR="${PROJECTDIR}/artifacts/dangerzone-windows-amd64-${APPVERSION}"

mkdir -p ${ARTIFACTSDIR}
cd ${PROJECTDIR}

echo "Building dangerzone_client"
podman run --privileged -v "${PROJECTDIR}":/src docker.io/uycyjnzgntrn/rust-windows:1.60.0 sh -c "cd /src/dangerzone_client && RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-pc-windows-gnu"
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure"
  exit 1
fi
cp ${PROJECTDIR}/dangerzone_client/target/x86_64-pc-windows-gnu/release/dangerzone-cli.exe ${ARTIFACTSDIR}/
cp ${PROJECTDIR}/dangerzone_client/target/x86_64-pc-windows-gnu/release/dangerzone-gui.exe ${ARTIFACTSDIR}/


echo "Building dangerzone_httpserver"
podman run --privileged -v "${PROJECTDIR}":/src docker.io/uycyjnzgntrn/rust-windows:1.60.0 sh -c "cd /src/dangerzone_httpserver && RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-pc-windows-gnu"
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure"
  exit 1
fi
cp ${PROJECTDIR}/dangerzone_httpserver/target/x86_64-pc-windows-gnu/release/dangerzone-httpserver.exe ${ARTIFACTSDIR}/

echo "Building dangerzone_httpclient"
podman run --privileged -v "${PROJECTDIR}":/src docker.io/uycyjnzgntrn/rust-windows:1.60.0 sh -c "cd /src/dangerzone_httpclient && RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-pc-windows-gnu"
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure"
  exit 1
fi
cp ${PROJECTDIR}/dangerzone_httpclient/target/x86_64-pc-windows-gnu/release/dangerzone-httpclient.exe ${ARTIFACTSDIR}/

cd ${PREVIOUSDIR}

