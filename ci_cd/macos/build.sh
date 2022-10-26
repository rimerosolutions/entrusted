#!/usr/bin/env sh
set -x

PREVIOUSDIR="$(echo $PWD)"
SCRIPTDIR="$(realpath $(dirname "$0"))"
PROJECTDIR="$(realpath ${SCRIPTDIR}/../../app)"
APPVERSION=$(awk -F ' = ' '$1 ~ /version/ { gsub(/[\"]/, "", $2); printf("%s",$2) }' ${PROJECTDIR}/entrusted_client/Cargo.toml)
CPU_ARCHS="amd64 aarch64"

for CPU_ARCH in $CPU_ARCHS ; do
    ARTIFACTSDIR="${PROJECTDIR}/../artifacts/entrusted-macos-${CPU_ARCH}-${APPVERSION}"
    rm -rf ${ARTIFACTSDIR}
done

for CPU_ARCH in $CPU_ARCHS ; do
    ARTIFACTSDIR="${PROJECTDIR}/../artifacts/entrusted-macos-${CPU_ARCH}-${APPVERSION}"
    RUST_TARGET="x86_64-apple-darwin"
    BUILD_PREAMBLE="true"
    RUSTFLAGS_PARAMS="RUSTFLAGS='-C target-feature=+crt-static'"
    EXPORT_PARAMS="export CXX=/usr/local/osxcross/target/bin/o64-clang++; export CC=/usr/local/osxcross/target/bin/o64-clang;"
    ADDITIONAL_PARAMS="cargo clean;"
    STRIP_COMMAND="x86_64-apple-darwin21.4-strip"

    if [ ${CPU_ARCH} != "amd64" ]
    then
        RUST_TARGET="aarch64-apple-darwin"
        EXPORT_PARAMS="export CC=oa64-clang; export CXX=oa64-clang++;"    
        ADDITIONAL_PARAMS="cargo clean;CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=arm64-apple-darwin21.4-clang LIBZ_SYS_STATIC=1"
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
    echo "TODO check stripping binaries later after more testing"

    podman run --rm \
           --volume "${PROJECTDIR}":/root/src \
           --workdir /root/src \
           docker.io/uycyjnzgntrn/rust-macos:1.64.0 \
           sh -c "${EXPORT_PARAMS} ${BUILD_PREAMBLE};cd /root/src/entrusted_webserver && ${ADDITIONAL_PARAMS} ${RUSTFLAGS_PARAMS} cargo build --release --target  ${RUST_TARGET} && cd /root/src/entrusted_client && ${ADDITIONAL_PARAMS} ${RUSTFLAGS_PARAMS} cargo build --release --features=gui --target ${RUST_TARGET} && cd /root/src/entrusted_webclient && ${ADDITIONAL_PARAMS} ${RUSTFLAGS_PARAMS} cargo build --release --target ${RUST_TARGET}"
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

    convert -scale 16x16   ${SCRIPTDIR}/macos/${APPNAME}.png ${SCRIPTDIR}/macos/${APPNAME}_16_16.png
    convert -scale 32x32   ${SCRIPTDIR}/macos/${APPNAME}.png ${SCRIPTDIR}/macos/${APPNAME}_32_32.png
    convert -scale 128x128 ${SCRIPTDIR}/macos/${APPNAME}.png ${SCRIPTDIR}/macos/${APPNAME}_128_128.png
    convert -scale 256x256 ${SCRIPTDIR}/macos/${APPNAME}.png ${SCRIPTDIR}/macos/${APPNAME}_256_256.png
    convert -scale 512x512 ${SCRIPTDIR}/macos/${APPNAME}.png ${SCRIPTDIR}/macos/${APPNAME}_512_512.png

    cp ${SCRIPTDIR}/macos/Info.plist ${APPBUNDLECONTENTS}/
    cp ${SCRIPTDIR}/macos/PkgInfo ${APPBUNDLECONTENTS}/
    png2icns ${APPBUNDLEICON}/${APPNAME}.icns ${SCRIPTDIR}/macos/${APPNAME}_16_16.png ${SCRIPTDIR}/macos/${APPNAME}_32_32.png ${SCRIPTDIR}/macos/${APPNAME}_128_128.png ${SCRIPTDIR}/macos/${APPNAME}_256_256.png ${SCRIPTDIR}/macos/${APPNAME}_512_512.png

    rm ${SCRIPTDIR}/macos/${APPNAME}_16_16.png
    rm ${SCRIPTDIR}/macos/${APPNAME}_32_32.png 
    rm ${SCRIPTDIR}/macos/${APPNAME}_128_128.png 
    rm ${SCRIPTDIR}/macos/${APPNAME}_256_256.png 
    rm ${SCRIPTDIR}/macos/${APPNAME}_512_512.png

    cp ${PROJECTDIR}/entrusted_client/target/${RUST_TARGET}/release/entrusted-cli ${APPBUNDLEEXE}/
    mv ${ARTIFACTSDIR}/entrusted-gui ${APPBUNDLEEXE}/
    cp ${SCRIPTDIR}/macos/${APPNAME}  ${APPBUNDLEEXE}/
    perl -pi -e "s/_COMPANY_NAME_/${APPBUNDLECOMPANY}/g" ${APPBUNDLECONTENTS}/Info.plist
    perl -pi -e "s/_APPVERSION_/${APPBUNDLEVERSION}/g" ${APPBUNDLECONTENTS}/Info.plist

    cp -r ${APPBUNDLE} ${APPDMGDIR}/
    ln -s /Applications ${APPDMGDIR}/
    podman run --rm -v "${ARTIFACTSDIR}":/files docker.io/sporsh/create-dmg "Entrusted" /files/dmg/ /files/entrusted-macos-${CPU_ARCH}-${APPVERSION}.dmg
    rm -rf ${APPDMGDIR}
    mv ${ARTIFACTSDIR}/*.dmg ${ARTIFACTSDIR}/../

    cp ${SCRIPTDIR}/release_README.txt ${ARTIFACTSDIR}/README.txt

    cd ${ARTIFACTSDIR}/.. && zip -r entrusted-macos-${CPU_ARCH}-${APPVERSION}.zip entrusted-macos-${CPU_ARCH}-${APPVERSION}
done

cd ${SCRIPTDIR}
