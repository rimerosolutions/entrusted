#!/usr/bin/env sh
set -x

PREVIOUSDIR="$(echo $PWD)"
SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${SCRIPTDIR}/../..)"
APPVERSION=$(awk -F ' = ' '$1 ~ /version/ { gsub(/[\"]/, "", $2); printf("%s",$2) }' ${PROJECTDIR}/dangerzone_client/Cargo.toml)
ARTIFACTSDIR="${PROJECTDIR}/artifacts/dangerzone-linux-amd64-${APPVERSION}"

mkdir -p ${ARTIFACTSDIR}

rm -rf ${PROJECTDIR}/dangerzone_container/target
rm -rf ${PROJECTDIR}/dangerzone_client/target
rm -rf ${PROJECTDIR}/dangerzone_httpclient/target
rm -rf ${PROJECTDIR}/dangerzone_httpserver/target

cd ${PROJECTDIR}

echo "Building dangerzone_client (dangerzone-gui)"
cp -f ${PROJECTDIR}/images/Dangerzone.png ${SCRIPTDIR}/appdir/dangerzone-gui.png
podman run --privileged -v "${PROJECTDIR}":/src -v "${SCRIPTDIR}/appdir":/appdir -v "${PROJECTDIR}/artifacts":/artifacts docker.io/uycyjnzgntrn/rust-centos7:1.60.0 /bin/bash -c "ln -sf /usr/lib64/libfuse.so.2.9.2 /usr/lib/libfuse.so.2 && mkdir -p /appdir/usr/bin /appdir/usr/share/icons && cd /src/dangerzone_client && /root/.cargo/bin/cargo build --release --bin dangerzone-gui && cp target/release/dangerzone-gui /appdir/ && mv /appdir/dangerzone-gui.png /appdir/usr/share/icons/dangerzone-gui.png && linuxdeploy --appdir /appdir --desktop-file /appdir/dangerzone-gui.desktop --icon-filename /appdir/usr/share/icons/dangerzone-gui.png --output appimage && mv *zone*.AppImage /artifacts/dangerzone-linux-amd64-${APPVERSION}/ && rm -rf /appdir/usr && rm -rf /appdir/dangerzone-gui /appdir/dangerzone-gui.png /appdir/.DirIcon"

retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure"
  exit 1
fi

echo "Building dangerzone_client (dangerzone-cli)"
cd ${PROJECTDIR}
podman run --rm --volume "${PWD}":/root/src --workdir /root/src docker.io/joseluisq/rust-linux-darwin-builder:1.60.0 sh -c "RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-unknown-linux-musl --manifest-path /root/src/dangerzone_client/Cargo.toml --bin dangerzone-cli"
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure"
  exit 1
fi
cp ${PROJECTDIR}/dangerzone_client/target/x86_64-unknown-linux-musl/release/dangerzone-cli ${ARTIFACTSDIR}

echo "Building dangerzone_httpserver"
cd ${PROJECTDIR}
podman run --rm --volume "${PWD}":/root/src --workdir /root/src docker.io/joseluisq/rust-linux-darwin-builder:1.60.0 sh -c "RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-unknown-linux-musl --manifest-path /root//src/dangerzone_httpserver/Cargo.toml"
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure"
  exit 1
fi
cp ${PROJECTDIR}/dangerzone_httpserver/target/x86_64-unknown-linux-musl/release/dangerzone-httpserver ${ARTIFACTSDIR}

echo "Building dangerzone_httpclient"
cd ${PROJECTDIR}
podman run --rm --volume "${PWD}":/root/src --workdir /root/src docker.io/joseluisq/rust-linux-darwin-builder:1.60.0 sh -c "RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-unknown-linux-musl --manifest-path /root/src/dangerzone_httpclient/Cargo.toml"
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure"
  exit 1
fi
cp ${PROJECTDIR}/dangerzone_httpclient/target/x86_64-unknown-linux-musl/release/dangerzone-httpclient ${ARTIFACTSDIR}

