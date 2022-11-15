#!/usr/bin/env sh
set -x

PREVIOUSDIR="$(echo $PWD)"

ROOT_SCRIPTDIR="$(realpath $(dirname "$0"))"
test -d "${ROOT_SCRIPTDIR}/../packages"  && rm -rf ${ROOT_SCRIPTDIR}/../packages
test -d "${ROOT_SCRIPTDIR}/../artifacts" && rm -rf ${ROOT_SCRIPTDIR}/../artifacts

mkdir -p ${ROOT_SCRIPTDIR}/../packages

sh ${ROOT_SCRIPTDIR}/create_container_image.sh
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Could not build container image"
  exit 1
fi

sh ${ROOT_SCRIPTDIR}/windows/build.sh
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Windows build failure"
  exit 1
fi

sh ${ROOT_SCRIPTDIR}/macos/build.sh
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Mac OS build failure"
  exit 1
fi

sh ${ROOT_SCRIPTDIR}/linux/build.sh
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Linux build failure"
  exit 1
fi

sh ${ROOT_SCRIPTDIR}/live_cd/build.sh
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Live CD build failure"
  exit 1
fi

echo "Moving release packages to final folder"
cd ${ROOT_SCRIPTDIR}/../artifacts
cp *.iso *.dmg *.exe *.zip *.tar *.rpm *.deb ${ROOT_SCRIPTDIR}/../packages/

cd ${PREVIOUSDIR}
