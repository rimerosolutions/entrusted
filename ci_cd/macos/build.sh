#!/usr/bin/env sh
set -x

PREVIOUSDIR="$(echo $PWD)"
SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${SCRIPTDIR}/../..)"
APPVERSION=$(awk -F ' = ' '$1 ~ /version/ { gsub(/[\"]/, "", $2); printf("%s",$2) }' ${PROJECTDIR}/entrusted_client/Cargo.toml)
ARTIFACTSDIR="${PROJECTDIR}/artifacts/entrusted-macos-amd64-${APPVERSION}"

mkdir -p ${ARTIFACTSDIR}

rm -rf ${PROJECTDIR}/entrusted_container/target
rm -rf ${PROJECTDIR}/entrusted_client/target
rm -rf ${PROJECTDIR}/entrusted_webclient/target
rm -rf ${PROJECTDIR}/entrusted_webserver/target

cd ${PROJECTDIR}

echo "Building entrusted_client"
podman run --rm \
    --volume "${PROJECTDIR}":/root/src \
    --workdir /root/src \
    docker.io/joseluisq/rust-linux-darwin-builder:1.60.0 \
    sh -c "export CC=/usr/local/osxcross/target/bin/o64-clang; export CXX=/usr/local/osxcross/target/bin/o64-clang++; cd /root/src/entrusted_client && RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-apple-darwin"
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure"
  exit 1
fi
cp ${PROJECTDIR}/entrusted_client/target/x86_64-apple-darwin/release/entrusted-cli ${ARTIFACTSDIR}
cp ${PROJECTDIR}/entrusted_client/target/x86_64-apple-darwin/release/entrusted-gui ${ARTIFACTSDIR}

echo "Building entrusted_webclient"
podman run --rm \
    --volume "${PROJECTDIR}":/root/src \
    --workdir /root/src \
    docker.io/joseluisq/rust-linux-darwin-builder:1.60.0 \
    sh -c "export CC=/usr/local/osxcross/target/bin/o64-clang; export CXX=/usr/local/osxcross/target/bin/o64-clang++; cd /root/src/entrusted_webclient && RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-apple-darwin"
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure"
  exit 1
fi
cp ${PROJECTDIR}/entrusted_webclient/target/x86_64-apple-darwin/release/entrusted-webclient ${ARTIFACTSDIR}

echo "Building entrusted_webserver"
podman run --rm \
    --volume "${PROJECTDIR}":/root/src \
    --workdir /root/src \
    docker.io/joseluisq/rust-linux-darwin-builder:1.60.0 \
    sh -c "export CC=/usr/local/osxcross/target/bin/o64-clang; export CXX=/usr/local/osxcross/target/bin/o64-clang++; cd /root/src/entrusted_webserver && RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-apple-darwin"
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Failure"
  exit 1
fi
cp ${PROJECTDIR}/entrusted_webserver/target/x86_64-apple-darwin/release/entrusted-webserver ${ARTIFACTSDIR}


# See https://github.com/zhlynn/zsign
# See https://forums.ivanti.com/s/article/Obtaining-an-Apple-Developer-ID-Certificate-for-macOS-Provisioning?language=en_US&ui-force-components-controllers-recordGlobalValueProvider.RecordGvp.getRecord=1
echo "TODO need to create signed app bundle with proper entitlements"

echo "Creating Entrusted appbundle"
cd ${SCRIPTDIR}
APPNAME=Entrusted
APPBUNDLE=${ARTIFACTSDIR}/${APPNAME}.app
APPDMGDIR=${ARTIFACTSDIR}/dmg
APPBUNDLECONTENTS=${APPBUNDLE}/Contents
APPBUNDLEEXE=${APPBUNDLECONTENTS}/MacOS
APPBUNDLERESOURCES=${APPBUNDLECONTENTS}/Resources
APPBUNDLEICON=${APPBUNDLECONTENTS}/Resources
APPBUNDLECOMPANY="Rimero Solutions Inc"
APPBUNDLEVERSION=${APPVERSION}

mkdir -p ${APPDMGDIR}
mkdir -p ${APPBUNDLE}
mkdir -p ${APPBUNDLE}/Contents
mkdir -p ${APPBUNDLE}/Contents/MacOS
mkdir -p ${APPBUNDLE}/Contents/Resources

convert -scale 16x16 macos/${APPNAME}.png macos/${APPNAME}_16_16.png
convert -scale 32x32 macos/${APPNAME}.png macos/${APPNAME}_32_32.png
convert -scale 128x128 macos/${APPNAME}.png macos/${APPNAME}_128_128.png
convert -scale 256x256 macos/${APPNAME}.png macos/${APPNAME}_256_256.png
convert -scale 512x512 macos/${APPNAME}.png macos/${APPNAME}_512_512.png

cp macos/Info.plist ${APPBUNDLECONTENTS}/
cp macos/PkgInfo ${APPBUNDLECONTENTS}/
png2icns ${APPBUNDLEICON}/${APPNAME}.icns macos/${APPNAME}_16_16.png macos/${APPNAME}_32_32.png macos/${APPNAME}_128_128.png macos/${APPNAME}_256_256.png macos/${APPNAME}_512_512.png

rm macos/${APPNAME}_16_16.png macos/${APPNAME}_32_32.png macos/${APPNAME}_128_128.png macos/${APPNAME}_256_256.png macos/${APPNAME}_512_512.png

cp ${PROJECTDIR}/entrusted_client/target/x86_64-apple-darwin/release/entrusted-cli ${APPBUNDLEEXE}/
mv ${ARTIFACTSDIR}/entrusted-gui ${APPBUNDLEEXE}/
cp macos/${APPNAME}  ${APPBUNDLEEXE}/
perl -pi -e "s/_COMPANY_NAME_/${APPBUNDLECOMPANY}/g" ${APPBUNDLECONTENTS}/Info.plist
perl -pi -e "s/_APPVERSION_/${APPBUNDLEVERSION}/g" ${APPBUNDLECONTENTS}/Info.plist

cp -r ${APPBUNDLE} ${APPDMGDIR}/
ln -s /Applications ${APPDMGDIR}/
podman run --rm -v "${ARTIFACTSDIR}":/files docker.io/sporsh/create-dmg "Entrusted" /files/dmg/ /files/entrusted-macos-amd64-${APPVERSION}.dmg
rm -rf ${APPDMGDIR}
mv ${ARTIFACTSDIR}/*.dmg ${ARTIFACTSDIR}/../

cp ${SCRIPTDIR}/release_README.txt ${ARTIFACTSDIR}/README.txt

cd ${ARTIFACTSDIR}/.. && zip -r entrusted-macos-amd64-${APPVERSION}.zip entrusted-macos-amd64-${APPVERSION}

cd ${SCRIPTDIR}
