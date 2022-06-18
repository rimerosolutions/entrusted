#!/usr/bin/env sh
set -x

PREVIOUSDIR="$(echo $PWD)"
SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${SCRIPTDIR}/../..)"
APPVERSION=$(awk -F ' = ' '$1 ~ /version/ { gsub(/[\"]/, "", $2); printf("%s",$2) }' ${PROJECTDIR}/entrusted_client/Cargo.toml)
ARTIFACTSDIR="${PROJECTDIR}/artifacts/entrusted-linux-amd64-${APPVERSION}"

mkdir -p ${ARTIFACTSDIR}

rm -rf ${PROJECTDIR}/entrusted_container/target
rm -rf ${PROJECTDIR}/entrusted_client/target
rm -rf ${PROJECTDIR}/entrusted_webclient/target
rm -rf ${PROJECTDIR}/entrusted_webserver/target

cd ${PROJECTDIR}

echo "Building entrusted_client (entrusted-gui)"
cp -f ${PROJECTDIR}/images/Entrusted.png ${SCRIPTDIR}/appdir/entrusted-gui.png

podman run --rm --privileged -v "${PROJECTDIR}":/src -v "${SCRIPTDIR}/appdir":/appdir -v "${PROJECTDIR}/artifacts":/artifacts docker.io/uycyjnzgntrn/rust-centos7:1.60.0 /bin/bash -c "ln -sf /usr/lib64/libfuse.so.2.9.2 /usr/lib/libfuse.so.2 && mkdir -p /appdir/usr/bin /appdir/usr/share/icons && cd /src/entrusted_client && /root/.cargo/bin/cargo build --release --bin entrusted-gui && cp target/release/entrusted-gui /appdir/ && mv /appdir/entrusted-gui.png /appdir/usr/share/icons/entrusted-gui.png && ARCH=x86_64 linuxdeploy --appdir /appdir --desktop-file /appdir/entrusted-gui.desktop --icon-filename /appdir/usr/share/icons/entrusted-gui.png --output appimage && mv *.AppImage /artifacts/entrusted-linux-amd64-${APPVERSION}/Entrusted_GUI-x86_64.AppImage && rm -rf /appdir/usr && rm -rf /appdir/entrusted-gui /appdir/entrusted-gui.png /appdir/.DirIcon"
echo "Restoring old GUI Desktop file to discard appimagetool changes"
cd ${PROJECTDIR} && git checkout ci_cd/linux/appdir/entrusted-gui.desktop && cd -

retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure"
  exit 1
fi

echo "Building entrusted_client (entrusted-cli)"
cd ${PROJECTDIR}
podman run --rm --volume "${PWD}":/root/src --workdir /root/src docker.io/joseluisq/rust-linux-darwin-builder:1.60.0 sh -c "RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-unknown-linux-musl --manifest-path /root/src/entrusted_client/Cargo.toml --bin entrusted-cli"
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure"
  exit 1
fi
cp ${PROJECTDIR}/entrusted_client/target/x86_64-unknown-linux-musl/release/entrusted-cli ${ARTIFACTSDIR}

echo "Building entrusted_webserver"
cd ${PROJECTDIR}
podman run --rm --volume "${PWD}":/root/src --workdir /root/src docker.io/joseluisq/rust-linux-darwin-builder:1.60.0 sh -c "RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-unknown-linux-musl --manifest-path /root//src/entrusted_webserver/Cargo.toml"
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure"
  exit 1
fi
cp ${PROJECTDIR}/entrusted_webserver/target/x86_64-unknown-linux-musl/release/entrusted-webserver ${ARTIFACTSDIR}

echo "Building entrusted_webclient"
cd ${PROJECTDIR}
podman run --rm --volume "${PWD}":/root/src --workdir /root/src docker.io/joseluisq/rust-linux-darwin-builder:1.60.0 sh -c "RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-unknown-linux-musl --manifest-path /root/src/entrusted_webclient/Cargo.toml"
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure"
  exit 1
fi
cp ${PROJECTDIR}/entrusted_webclient/target/x86_64-unknown-linux-musl/release/entrusted-webclient ${ARTIFACTSDIR}

cp ${SCRIPTDIR}/release_README.txt ${ARTIFACTSDIR}/README.txt

cd ${ARTIFACTSDIR}/.. && tar cvf entrusted-linux-amd64-${APPVERSION}.tar entrusted-linux-amd64-${APPVERSION}

cd ${SCRIPTDIR}
