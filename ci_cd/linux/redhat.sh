#!/usr/bin/env sh
set -x

APPVERSION=$1
OUTPUT_PKGLOCATION=$2
IMAGES_PROJECTDIR=$3
LINUX_ARTIFACTSDIR=$4
APPNAME=entrusted
BUILDFOLDERNAME=${APPNAME}-${APPVERSION}-1_amd64
SCRIPTDIR="$(realpath $(dirname "$0"))"
RPMBUILD_TOPDIR="/tmp/entrusted-pkg-rpm"
RPMBUILD_SOURCE="${RPMBUILD_TOPDIR}/${APPNAME}-linux-amd64-${APPVERSION}-src"
RPMBUILD_BUILDROOT="${RPMBUILD_TOPDIR}/${APPNAME}-linux-amd64-${APPVERSION}-build"

RPMBUILD_SOURCE_SED=$(echo ${RPMBUILD_SOURCE} | sed 's_/_\\/_g')
RPMBUILD_BUILDROOT_SED=$(echo ${RPMBUILD_BUILDROOT} | sed 's_/_\\/_g')

test -d ${RPMBUILD_TOPDIR} && rm -rf ${RPMBUILD_TOPDIR}

mkdir -p ${RPMBUILD_BUILDROOT} ${RPMBUILD_SOURCE}
mkdir -p ${RPMBUILD_SOURCE}/usr/share/doc/${APPNAME}
mkdir -p ${RPMBUILD_SOURCE}/usr/share/man/man1
mkdir -p ${RPMBUILD_SOURCE}/usr/share/applications
mkdir -p ${RPMBUILD_SOURCE}/usr/share/icons
mkdir -p ${RPMBUILD_SOURCE}/usr/bin

cp -f ${SCRIPTDIR}/packaging/redhat_spec ${RPMBUILD_SOURCE}/entrusted.spec
cp -f ${LINUX_ARTIFACTSDIR}/entrusted-cli ${RPMBUILD_SOURCE}/usr/bin/
cp -f ${LINUX_ARTIFACTSDIR}/*.AppImage ${RPMBUILD_SOURCE}/usr/bin/entrusted-gui
cp -f ${SCRIPTDIR}/xdg/*.desktop ${RPMBUILD_SOURCE}/usr/share/applications/
cp -f ${IMAGES_PROJECTDIR}/Entrusted.png ${RPMBUILD_SOURCE}/usr/share/icons/entrusted-gui.png
cp -f ${SCRIPTDIR}/doc/copyright ${RPMBUILD_SOURCE}/usr/share/doc/${APPNAME}/
cp -f ${SCRIPTDIR}/doc/changelog ${RPMBUILD_SOURCE}/usr/share/doc/${APPNAME}/changelog

perl -pi -e "s/_APPVERSION_/${APPVERSION}/g" ${RPMBUILD_SOURCE}/entrusted.spec;
perl -pi -e "s/_RPMBUILD_SOURCE_/${RPMBUILD_SOURCE_SED}/g" ${RPMBUILD_SOURCE}/entrusted.spec
perl -pi -e "s/_RPMBUILD_BUILDROOT_/${RPMBUILD_BUILDROOT_SED}/g" ${RPMBUILD_SOURCE}/entrusted.spec

gzip -9 -n ${RPMBUILD_SOURCE}/usr/share/doc/${APPNAME}/changelog

cp ${SCRIPTDIR}/manpages/*.md ${RPMBUILD_SOURCE}/usr/share/man/man1/
perl -pi -e "s/_APPVERSION_/${APPVERSION}/g" ${RPMBUILD_SOURCE}/usr/share/man/man1/${APPNAME}-gui.1.md
perl -pi -e "s/_APPVERSION_/${APPVERSION}/g" ${RPMBUILD_SOURCE}/usr/share/man/man1/${APPNAME}-cli.1.md
pandoc --standalone --to man ${RPMBUILD_SOURCE}/usr/share/man/man1/${APPNAME}-gui.1.md -o  ${RPMBUILD_SOURCE}/usr/share/man/man1/${APPNAME}-gui.1
pandoc --standalone --to man ${RPMBUILD_SOURCE}/usr/share/man/man1/${APPNAME}-cli.1.md -o  ${RPMBUILD_SOURCE}/usr/share/man/man1/${APPNAME}-cli.1
gzip -9 -n ${RPMBUILD_SOURCE}/usr/share/man/man1/${APPNAME}-gui.1
gzip -9 -n ${RPMBUILD_SOURCE}/usr/share/man/man1/${APPNAME}-cli.1
rm -rf ${RPMBUILD_SOURCE}/usr/share/man/man1/*.md

rpmbuild --define "_topdir ${RPMBUILD_TOPDIR}" -v --buildroot="${RPMBUILD_BUILDROOT}" -bb ${RPMBUILD_SOURCE}/entrusted.spec

test -d ${RPMBUILD_SOURCE} && rm -rf ${RPMBUILD_SOURCE}
cp "${RPMBUILD_TOPDIR}"/RPMS/x86_64/*.rpm ${OUTPUT_PKGLOCATION}
test -d ${RPMBUILD_TOPDIR} && rm -rf ${RPMBUILD_TOPDIR}
