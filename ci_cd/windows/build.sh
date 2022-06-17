#!/usr/bin/env sh
set -x

PREVIOUSDIR="$(echo $PWD)"
SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${SCRIPTDIR}/../..)"
APPVERSION=$(awk -F ' = ' '$1 ~ /version/ { gsub(/[\"]/, "", $2); printf("%s",$2) }' ${PROJECTDIR}/entrusted_client/Cargo.toml)
ARTIFACTSDIR="${PROJECTDIR}/artifacts/entrusted-windows-amd64-${APPVERSION}"

mkdir -p ${ARTIFACTSDIR}

cp ${PROJECTDIR}/LICENSE ${ARTIFACTSDIR}/LICENSE.txt

rm -rf ${PROJECTDIR}/entrusted_container/target
rm -rf ${PROJECTDIR}/entrusted_client/target
rm -rf ${PROJECTDIR}/entrusted_webclient/target
rm -rf ${PROJECTDIR}/entrusted_webserver/target

cd ${PROJECTDIR}

echo "Building entrusted_client"
podman run --rm --privileged -v "${PROJECTDIR}":/src docker.io/uycyjnzgntrn/rust-windows:1.60.0 sh -c "cd /src/entrusted_client && RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-pc-windows-gnu"
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure"
  exit 1
fi
cp ${PROJECTDIR}/entrusted_client/target/x86_64-pc-windows-gnu/release/entrusted-cli.exe ${ARTIFACTSDIR}/
cp ${PROJECTDIR}/entrusted_client/target/x86_64-pc-windows-gnu/release/entrusted-gui.exe ${ARTIFACTSDIR}/


echo "Building entrusted_webserver"
podman run --rm  --privileged -v "${PROJECTDIR}":/src docker.io/uycyjnzgntrn/rust-windows:1.60.0 sh -c "cd /src/entrusted_webserver && RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-pc-windows-gnu"
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure"
  exit 1
fi
cp ${PROJECTDIR}/entrusted_webserver/target/x86_64-pc-windows-gnu/release/entrusted-webserver.exe ${ARTIFACTSDIR}/

echo "Building entrusted_webclient"
podman run --rm --privileged -v "${PROJECTDIR}":/src docker.io/uycyjnzgntrn/rust-windows:1.60.0 sh -c "cd /src/entrusted_webclient && RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-pc-windows-gnu"
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure"
  exit 1
fi
cp ${PROJECTDIR}/entrusted_webclient/target/x86_64-pc-windows-gnu/release/entrusted-webclient.exe ${ARTIFACTSDIR}/

echo "Generate windows installer"
cp ${SCRIPTDIR}/installer.nsi ${ARTIFACTSDIR}/
perl -pi -e "s/_APPVERSION_/${APPVERSION}/g" ${ARTIFACTSDIR}/installer.nsi
podman run --rm -v "${ARTIFACTSDIR}":/build docker.io/binfalse/nsis installer.nsi
rm ${ARTIFACTSDIR}/installer.nsi
mv ${ARTIFACTSDIR}/entrusted-windows-amd64-${APPVERSION}.exe ${ARTIFACTSDIR}/../

cp ${SCRIPTDIR}/release_README.txt ${ARTIFACTSDIR}/README.txt

cd ${ARTIFACTSDIR}/.. && zip -r entrusted-windows-amd64-${APPVERSION}.zip entrusted-windows-amd64-${APPVERSION}

cd ${SCRIPTDIR}
