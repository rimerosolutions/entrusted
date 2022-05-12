#!/usr/bin/env sh
set -x

PREVIOUSDIR="$(echo $PWD)"
SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${SCRIPTDIR}/../..)"
APPVERSION=$(awk -F ' = ' '$1 ~ /version/ { gsub(/[\"]/, "", $2); printf("%s",$2) }' ${PROJECTDIR}/dangerzone_client/Cargo.toml)
ARTIFACTSDIR="${PROJECTDIR}/artifacts/dangerzone-darwin-amd64-${APPVERSION}"

mkdir -p ${ARTIFACTSDIR}
cd ${PROJECTDIR}

echo "Building dangerzone_client"
podman run --rm \
    --volume "${PROJECTDIR}":/root/src \
    --workdir /root/src \
    docker.io/joseluisq/rust-linux-darwin-builder:1.60.0 \
    sh -c "export CC=/usr/local/osxcross/target/bin/o64-clang; export CXX=/usr/local/osxcross/target/bin/o64-clang++; cd /root/src/dangerzone_client && RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-apple-darwin"
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure"
  exit 1
fi
cp ${PROJECTDIR}/dangerzone_client/target/x86_64-apple-darwin/release/dangerzone-cli ${ARTIFACTSDIR}
cp ${PROJECTDIR}/dangerzone_client/target/x86_64-apple-darwin/release/dangerzone-gui ${ARTIFACTSDIR}

echo "Building dangerzone_httpclient"
podman run --rm \
    --volume "${PROJECTDIR}":/root/src \
    --workdir /root/src \
    docker.io/joseluisq/rust-linux-darwin-builder:1.60.0 \
    sh -c "export CC=/usr/local/osxcross/target/bin/o64-clang; export CXX=/usr/local/osxcross/target/bin/o64-clang++; cd /root/src/dangerzone_httpclient && RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-apple-darwin"
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure"
  exit 1
fi
cp ${PROJECTDIR}/dangerzone_httpclient/target/x86_64-apple-darwin/release/dangerzone-httpclient ${ARTIFACTSDIR}

echo "Building dangerzone_httpserver"
podman run --rm \
    --volume "${PROJECTDIR}":/root/src \
    --workdir /root/src \
    docker.io/joseluisq/rust-linux-darwin-builder:1.60.0 \
    sh -c "export CC=/usr/local/osxcross/target/bin/o64-clang; export CXX=/usr/local/osxcross/target/bin/o64-clang++; cd /root/src/dangerzone_httpserver && RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-apple-darwin"
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure"
  exit 1
fi
cp ${PROJECTDIR}/dangerzone_httpserver/target/x86_64-apple-darwin/release/dangerzone-httpserver ${ARTIFACTSDIR}

echo "Creating dangerzone appbundle"
cd ${SCRIPTDIR}
APPNAME=Dangerzone
APPBUNDLE=${ARTIFACTSDIR}/${APPNAME}.app
APPBUNDLECONTENTS=${APPBUNDLE}/Contents
APPBUNDLEEXE=${APPBUNDLECONTENTS}/MacOS
APPBUNDLERESOURCES=${APPBUNDLECONTENTS}/Resources
APPBUNDLEICON=${APPBUNDLECONTENTS}/Resources
APPBUNDLECOMPANY="Rimero Solutions Inc"
APPBUNDLEVERSION=${APPVERSION}

mkdir ${APPBUNDLE}
mkdir ${APPBUNDLE}/Contents
mkdir ${APPBUNDLE}/Contents/MacOS
mkdir ${APPBUNDLE}/Contents/Resources

convert -scale 16x16 macosx/${APPNAME}.png macosx/${APPNAME}_16_16.png
convert -scale 32x32 macosx/${APPNAME}.png macosx/${APPNAME}_32_32.png
convert -scale 128x128 macosx/${APPNAME}.png macosx/${APPNAME}_128_128.png
convert -scale 256x256 macosx/${APPNAME}.png macosx/${APPNAME}_256_256.png
convert -scale 512x512 macosx/${APPNAME}.png macosx/${APPNAME}_512_512.png

cp macosx/Info.plist ${APPBUNDLECONTENTS}/
cp macosx/PkgInfo ${APPBUNDLECONTENTS}/
png2icns ${APPBUNDLEICON}/${APPNAME}.icns macosx/${APPNAME}_16_16.png macosx/${APPNAME}_32_32.png macosx/${APPNAME}_128_128.png macosx/${APPNAME}_256_256.png macosx/${APPNAME}_512_512.png

rm macosx/${APPNAME}_16_16.png macosx/${APPNAME}_32_32.png macosx/${APPNAME}_128_128.png macosx/${APPNAME}_256_256.png macosx/${APPNAME}_512_512.png

cp ${PROJECTDIR}/dangerzone_client/target/x86_64-apple-darwin/release/dangerzone-cli ${APPBUNDLEEXE}/
mv  ${ARTIFACTSDIR}/dangerzone-gui ${APPBUNDLEEXE}/${APPNAME}
perl -pi -e "s/_COMPANY_NAME_/${APPBUNDLECOMPANY}/g" ${APPBUNDLECONTENTS}/Info.plist
perl -pi -e "s/_APPVERSION_/${APPBUNDLEVERSION}/g" ${APPBUNDLECONTENTS}/Info.plist

cd ${PREVIOUSDIR}

