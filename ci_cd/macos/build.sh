#!/usr/bin/env sh
set -x

PREVIOUSDIR="$(echo $PWD)"
SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${SCRIPTDIR}/../../app)"
APPVERSION=$(grep "^version" ${PROJECTDIR}/entrusted_client/Cargo.toml  | cut -d"=" -f2 | xargs)
CPU_ARCHS="amd64 aarch64"
RUST_CI_VERSION="1.72.0"

for CPU_ARCH in $CPU_ARCHS ; do
    ARTIFACTSDIR="${PROJECTDIR}/../artifacts/entrusted-${APPVERSION}-macos-${CPU_ARCH}"
    rm -rf ${ARTIFACTSDIR}
done

for CPU_ARCH in $CPU_ARCHS ; do
    ARTIFACTSDIR="${PROJECTDIR}/../artifacts/entrusted-${APPVERSION}-macos-${CPU_ARCH}"
    RUST_TARGET="x86_64-apple-darwin"
    BUILD_PREAMBLE="true"
    ADDITIONAL_PARAMS=""
    RUSTFLAGS_PARAMS="RUSTFLAGS='-C target-feature=+crt-static'"
    EXPORT_PARAMS="export CARGO_NET_GIT_FETCH_WITH_CLI=true; export CARGO_NET_RETRY=10; export CXX=/usr/local/osxcross/target/bin/o64-clang++; export CC=/usr/local/osxcross/target/bin/o64-clang;"
    STRIP_COMMAND="x86_64-apple-darwin21.4-strip"

    if [ ${CPU_ARCH} != "amd64" ]
    then
        RUST_TARGET="aarch64-apple-darwin"
        EXPORT_PARAMS="export CARGO_NET_GIT_FETCH_WITH_CLI=true; export CARGO_NET_RETRY=10; export CC=oa64-clang; export CXX=oa64-clang++;"    
        ADDITIONAL_PARAMS="CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=arm64-apple-darwin21.4-clang LIBZ_SYS_STATIC=1"
        BUILD_PREAMBLE="rustup target add aarch64-apple-darwin"
        STRIP_COMMAND="arm64-apple-darwin21.4-strip"
    fi

    mkdir -p ${ARTIFACTSDIR}

    rm -rf ${PROJECTDIR}/entrusted_container/target
    rm -rf ${PROJECTDIR}/entrusted_client/target
    rm -rf ${PROJECTDIR}/entrusted_webclient/target
    rm -rf ${PROJECTDIR}/entrusted_webserver/target

    cd ${PROJECTDIR}

    echo "Building all Mac OS binaries for ${CPU_ARCH}"

    podman run --rm \
           --volume "${PROJECTDIR}":/root/src \
           --workdir /root/src \
           docker.io/uycyjnzgntrn/rust-macos:${RUST_CI_VERSION} \
           sh -c "${EXPORT_PARAMS} ${BUILD_PREAMBLE}; ${ADDITIONAL_PARAMS} ${RUSTFLAGS_PARAMS} cargo build --release --target  ${RUST_TARGET}  --manifest-path /root/src/entrusted_webserver/Cargo.toml && ${ADDITIONAL_PARAMS} ${RUSTFLAGS_PARAMS} cargo build --release --features=gui --target ${RUST_TARGET} --manifest-path /root/src/entrusted_client/Cargo.toml && ${ADDITIONAL_PARAMS} ${RUSTFLAGS_PARAMS} cargo build --release --target ${RUST_TARGET} --manifest-path /root/src/entrusted_webclient/Cargo.toml && ${STRIP_COMMAND} /root/src/entrusted_client/target/${RUST_TARGET}/release/entrusted-cli && ${STRIP_COMMAND} /root/src/entrusted_client/target/${RUST_TARGET}/release/entrusted-gui && ${STRIP_COMMAND} /root/src/entrusted_webclient/target/${RUST_TARGET}/release/entrusted-webclient && ${STRIP_COMMAND} /root/src/entrusted_webserver/target/${RUST_TARGET}/release/entrusted-webserver"    
    
    retVal=$?
    if [ $retVal -ne 0 ]; then
        echo "Failure"
        exit 1
    fi        
    
    cp ${PROJECTDIR}/entrusted_client/target/${RUST_TARGET}/release/entrusted-cli ${ARTIFACTSDIR}
    cp ${PROJECTDIR}/entrusted_client/target/${RUST_TARGET}/release/entrusted-gui ${ARTIFACTSDIR}
    cp ${PROJECTDIR}/entrusted_webclient/target/${RUST_TARGET}/release/entrusted-webclient ${ARTIFACTSDIR}
    cp ${PROJECTDIR}/entrusted_webserver/target/${RUST_TARGET}/release/entrusted-webserver ${ARTIFACTSDIR}

    # See https://github.com/zhlynn/zsign
    # See https://forums.ivanti.com/s/article/Obtaining-an-Apple-Developer-ID-Certificate-for-macOS-Provisioning?language=en_US&ui-force-components-controllers-recordGlobalValueProvider.RecordGvp.getRecord=1
    # echo "TODO need to create signed app bundle with proper entitlements, do we need to pay to share software for free too??????????????"

    echo "Creating Entrusted appbundle"
    cd ${SCRIPTDIR}
    APPNAME=Entrusted
    APPBUNDLE=${ARTIFACTSDIR}/${APPNAME}.app
    APPDMGDIR=${ARTIFACTSDIR}/dmg
    APPBUNDLECONTENTS=${APPBUNDLE}/Contents
    APPBUNDLETMP=${APPBUNDLE}/tmp
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
    mkdir -p ${APPBUNDLETMP}    

    convert ${PROJECTDIR}/images/${APPNAME}_icon.png ${APPBUNDLETMP}/${APPNAME}_icon.jpg
    
    convert -scale 16x16    -depth 8 ${APPBUNDLETMP}/${APPNAME}_icon.jpg  ${APPBUNDLETMP}/${APPNAME}_16_16.png
    convert -scale 32x32    -depth 8 ${APPBUNDLETMP}/${APPNAME}_icon.jpg  ${APPBUNDLETMP}/${APPNAME}_32_32.png
    convert -scale 128x128  -depth 8 ${APPBUNDLETMP}/${APPNAME}_icon.jpg  ${APPBUNDLETMP}/${APPNAME}_128_128.png
    convert -scale 256x256  -depth 8 ${APPBUNDLETMP}/${APPNAME}_icon.jpg  ${APPBUNDLETMP}/${APPNAME}_256_256.png
    convert -scale 512x512  -depth 8 ${APPBUNDLETMP}/${APPNAME}_icon.jpg  ${APPBUNDLETMP}/${APPNAME}_512_512.png

    png2icns ${APPBUNDLEICON}/${APPNAME}.icns ${APPBUNDLETMP}/${APPNAME}_16_16.png ${APPBUNDLETMP}/${APPNAME}_32_32.png ${APPBUNDLETMP}/${APPNAME}_128_128.png ${APPBUNDLETMP}/${APPNAME}_256_256.png  ${APPBUNDLETMP}/${APPNAME}_512_512.png && rm -rf ${APPBUNDLETMP}
    
    cp ${SCRIPTDIR}/macos/Info.plist ${APPBUNDLECONTENTS}/
    cp ${SCRIPTDIR}/macos/PkgInfo ${APPBUNDLECONTENTS}/

    cp ${PROJECTDIR}/entrusted_client/target/${RUST_TARGET}/release/entrusted-cli ${APPBUNDLEEXE}/
    mv ${ARTIFACTSDIR}/entrusted-gui ${APPBUNDLEEXE}/${APPNAME}
    perl -pi -e "s/_COMPANY_NAME_/${APPBUNDLECOMPANY}/g" ${APPBUNDLECONTENTS}/Info.plist
    perl -pi -e "s/_APPVERSION_/${APPBUNDLEVERSION}/g" ${APPBUNDLECONTENTS}/Info.plist

    cp -r ${APPBUNDLE} ${APPDMGDIR}/
    ln -s /Applications ${APPDMGDIR}/
    podman run --rm -v "${ARTIFACTSDIR}":/files docker.io/sporsh/create-dmg "Entrusted" /files/dmg/ /files/entrusted-${APPVERSION}-macos-${CPU_ARCH}.dmg
    rm -rf ${APPDMGDIR}
    mv ${ARTIFACTSDIR}/*.dmg ${ARTIFACTSDIR}/../

    cp ${SCRIPTDIR}/release_README.txt ${ARTIFACTSDIR}/README.txt

    cd ${ARTIFACTSDIR}/.. && zip -r entrusted-${APPVERSION}-macos-${CPU_ARCH}.zip entrusted-${APPVERSION}-macos-${CPU_ARCH}
done

cd ${SCRIPTDIR}
