#!/usr/bin/env sh
set -x

PREVIOUSDIR="$(echo $PWD)"

ROOT_SCRIPTDIR="$(realpath $(dirname "$0"))"
rm -rf ${ROOT_SCRIPTDIR}/../packages
rm -rf ${ROOT_SCRIPTDIR}/../artifacts

mkdir -p ${ROOT_SCRIPTDIR}/../packages
sh ${ROOT_SCRIPTDIR}/darwin/build.sh
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "MacOS build failure"
  exit 1
fi

sh ${ROOT_SCRIPTDIR}/linux/build.sh
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Linux build failure"
  exit 1
fi

sh ${ROOT_SCRIPTDIR}/windows/build.sh
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Windows build failure"
  exit 1
fi

sh ${ROOT_SCRIPTDIR}/live_cd/build.sh
retVal=$?
if [ $retVal -ne 0 ]; then
	echo "Live CD build failure"
  exit 1
fi

echo "Moving release packages to final folder"
cp ${ROOT_SCRIPTDIR}/../artifacts/*.iso \
   ${ROOT_SCRIPTDIR}/../artifacts/*.exe \
   ${ROOT_SCRIPTDIR}/../artifacts/*.zip \
   ${ROOT_SCRIPTDIR}/../artifacts/*.dmg \
   ${ROOT_SCRIPTDIR}/../artifacts/*.tar \
   ${ROOT_SCRIPTDIR}/../packages/

cd ${PREVIOUSDIR}
