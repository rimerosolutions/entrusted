#!/usr/bin/env sh

APPVERSION=$1
OUTPUT_PKGLOCATION=$2
IMAGES_PROJECTDIR=$3
LINUX_ARTIFACTSDIR=$4
CPU_ARCH=$5
APPNAME=entrusted
BUILDFOLDERNAME=${APPNAME}-${APPVERSION}-1_${CPU_ARCH}
BUILDTOPDIR="/tmp/entrusted-pkg-deb/build"
SCRIPTDIR="$(realpath $(dirname "$0"))"

test -d ${BUILDTOPDIR}/${BUILDFOLDERNAME} && rm -rf ${BUILDTOPDIR}/${BUILDFOLDERNAME}
test -f ${OUTPUT_PKGLOCATION} && rm ${OUTPUT_PKGLOCATION}

mkdir -p ${BUILDTOPDIR}/${BUILDFOLDERNAME}/DEBIAN
mkdir -p ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/bin
mkdir -p ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/share/applications
mkdir -p ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/share/doc/${APPNAME}
mkdir -p ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/share/pixmaps
mkdir -p ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/share/man/man1

cp -f ${SCRIPTDIR}/packaging/debian_spec ${BUILDTOPDIR}/${BUILDFOLDERNAME}/DEBIAN/control
cp -f ${LINUX_ARTIFACTSDIR}/entrusted-cli ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/bin/
cp -f ${LINUX_ARTIFACTSDIR}/entrusted-gui ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/bin/entrusted-gui
cp -f ${SCRIPTDIR}/xdg/*.desktop ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/share/applications/
cp -f ${IMAGES_PROJECTDIR}/Entrusted_icon.png ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/share/pixmaps/entrusted-gui.png
cp -f ${SCRIPTDIR}/doc/copyright ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/share/doc/${APPNAME}/
cp -f ${SCRIPTDIR}/doc/changelog ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/share/doc/${APPNAME}/changelog

perl -pi -e "s/_APPVERSION_/${APPVERSION}/g" ${BUILDTOPDIR}/${BUILDFOLDERNAME}/DEBIAN/control
perl -pi -e "s/_CPU_ARCH_/${CPU_ARCH}/g" ${BUILDTOPDIR}/${BUILDFOLDERNAME}/DEBIAN/control
gzip -9 -n ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/share/doc/${APPNAME}/changelog

cp ${SCRIPTDIR}/manpages/${APPNAME}*.md ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/share/man/man1/
perl -pi -e "s/_APPVERSION_/${APPVERSION}/g" ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/share/man/man1/${APPNAME}-cli.1.md
perl -pi -e "s/_APPVERSION_/${APPVERSION}/g" ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/share/man/man1/${APPNAME}-gui.1.md
pandoc --standalone --to man ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/share/man/man1/${APPNAME}-cli.1.md -o ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/share/man/man1/${APPNAME}-cli.1
pandoc --standalone --to man ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/share/man/man1/${APPNAME}-gui.1.md -o ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/share/man/man1/${APPNAME}-gui.1
gzip -9 -n ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/share/man/man1/${APPNAME}-gui.1
gzip -9 -n ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/share/man/man1/${APPNAME}-cli.1
rm ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/share/man/man1/*.md

SIZE_IN_KB="$(du -s ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/ | awk '{print $1;}')"
echo "Installed-Size: ${SIZE_IN_KB}" >> ${BUILDTOPDIR}/${BUILDFOLDERNAME}/DEBIAN/control

find ${BUILDTOPDIR}/${BUILDFOLDERNAME}/ -type d -exec chmod 0755 {} \;
find ${BUILDTOPDIR}/${BUILDFOLDERNAME}/ -type f -exec chmod 0644 {} \;
find ${BUILDTOPDIR}/${BUILDFOLDERNAME}/usr/bin -type f -exec chmod 0755 {} \;

dpkg-deb --root-owner-group --build ${BUILDTOPDIR}/${BUILDFOLDERNAME} ${OUTPUT_PKGLOCATION}

test -d ${BUILDTOPDIR}/${BUILDFOLDERNAME} && rm -rf ${BUILDTOPDIR}/${BUILDFOLDERNAME}
