#!/usr/bin/env sh
set -x

PREVIOUSDIR="$(echo $PWD)"

ROOT_SCRIPTDIR="$(realpath $(dirname "$0"))"

rm -rf ${ROOT_SCRIPTDIR}/../artifacts

sh ${ROOT_SCRIPTDIR}/darwin/build.sh
sh ${ROOT_SCRIPTDIR}/linux/build.sh
sh ${ROOT_SCRIPTDIR}/windows/build.sh

cd ${PREVIOUSDIR}
