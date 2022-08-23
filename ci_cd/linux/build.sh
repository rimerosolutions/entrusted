#!/usr/bin/env sh
set -x

PREVIOUSDIR="$(echo $PWD)"
SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${SCRIPTDIR}/../..)"
APPVERSION=$(awk -F ' = ' '$1 ~ /version/ { gsub(/[\"]/, "", $2); printf("%s",$2) }' ${PROJECTDIR}/entrusted_client/Cargo.toml)
ARTIFACTSDIR="${PROJECTDIR}/artifacts/entrusted-linux-amd64-${APPVERSION}"
PKG_FILE_DEB="${PROJECTDIR}/artifacts/entrusted-linux-amd64-${APPVERSION}.deb"
PKG_FILE_RPM="${PROJECTDIR}/artifacts/entrusted-linux-amd64-${APPVERSION}.rpm"

mkdir -p ${ARTIFACTSDIR}

test -d ${PROJECTDIR}/entrusted_container/target && rm -rf ${PROJECTDIR}/entrusted_container/target
test -d ${PROJECTDIR}/entrusted_client/target && rm -rf ${PROJECTDIR}/entrusted_client/target
test -d ${PROJECTDIR}/entrusted_webclient/target && rm -rf ${PROJECTDIR}/entrusted_webclient/target
test -d ${PROJECTDIR}/entrusted_webserver/target && rm -rf ${PROJECTDIR}/entrusted_webserver/target

cd ${PROJECTDIR}

echo "Building entrusted_client (entrusted-gui)"
podman run --rm --privileged -v "${PROJECTDIR}":/src -v "${PROJECTDIR}/artifacts":/artifacts docker.io/uycyjnzgntrn/rust-centos8:1.63.0 /bin/bash -c "ln -sf /usr/lib64/libfuse.so.2.9.2 /usr/lib/libfuse.so.2 && mkdir -p /tmp/appdir/usr/bin /tmp/appdir/usr/share/icons && cp /src/ci_cd/linux/xdg/* /tmp/appdir/ && cd /src/entrusted_client && /root/.cargo/bin/cargo build --release --bin entrusted-gui && cp target/release/entrusted-gui /tmp/appdir/ && cp /src/images/Entrusted.png /tmp/appdir/usr/share/icons/entrusted-gui.png && ARCH=x86_64 linuxdeploy --appdir /tmp/appdir --desktop-file /tmp/appdir/entrusted-gui.desktop --icon-filename /tmp/appdir/usr/share/icons/entrusted-gui.png --output appimage && mv *.AppImage /artifacts/entrusted-linux-amd64-${APPVERSION}/Entrusted_GUI-x86_64.AppImage"

retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure to create Linux GUI AppImage binary"
  exit 1
fi

# echo "Building other Linux binaries"
cd ${PROJECTDIR}
podman run --rm --volume "${PWD}":/root/src --workdir /root/src docker.io/joseluisq/rust-linux-darwin-builder:1.63.0 sh -c "RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-unknown-linux-musl --manifest-path /root/src/entrusted_client/Cargo.toml --bin entrusted-cli && RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-unknown-linux-musl --manifest-path /root/src/entrusted_webserver/Cargo.toml && RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-unknown-linux-musl --manifest-path /root/src/entrusted_webclient/Cargo.toml"

retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Fail to build other Linux CLI binaries"
  exit 1
fi

cp ${PROJECTDIR}/entrusted_client/target/x86_64-unknown-linux-musl/release/entrusted-cli ${ARTIFACTSDIR}
cp ${PROJECTDIR}/entrusted_webserver/target/x86_64-unknown-linux-musl/release/entrusted-webserver ${ARTIFACTSDIR}
cp ${PROJECTDIR}/entrusted_webclient/target/x86_64-unknown-linux-musl/release/entrusted-webclient ${ARTIFACTSDIR}

cp ${SCRIPTDIR}/release_README.txt ${ARTIFACTSDIR}/README.txt

cd ${ARTIFACTSDIR}/.. && tar cvf entrusted-linux-amd64-${APPVERSION}.tar entrusted-linux-amd64-${APPVERSION}

${SCRIPTDIR}/debian.sh ${APPVERSION} ${PKG_FILE_DEB} ${PROJECTDIR}/images ${ARTIFACTSDIR}
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure to create Linux DEB package"
  exit 1
fi

${SCRIPTDIR}/redhat.sh ${APPVERSION} ${PKG_FILE_RPM} ${PROJECTDIR}/images ${ARTIFACTSDIR}
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure to create Linux RPM package"
  exit 1
fi

cd ${SCRIPTDIR}
