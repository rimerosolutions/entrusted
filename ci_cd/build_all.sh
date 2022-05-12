#!/usr/bin/env sh
set -x

PREVIOUSDIR="$(echo $PWD)"

ROOT_SCRIPTDIR="$(realpath $(dirname "$0"))"

rm -rf ${ROOT_SCRIPTDIR}/../artifacts

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

cd ${PREVIOUSDIR}
