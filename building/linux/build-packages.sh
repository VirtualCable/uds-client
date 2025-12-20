#!/bin/bash

VERSION=$( [ -f ../../../VERSION ] && cat ../../../VERSION || echo "devel" )
RELEASE=1

UID_HOST=$(id -u)
GID_HOST=$(id -g)

top=$(pwd)
# Resolve %top/../..
crate=$(realpath ${top}/../..)


for debian_version in 12 13; do
    # Compile first the binary using rustbuilder.py
    echo "=== Building udslauncher binary using rustbuilder.py ==="
    cd ${top}
    python3 rustbuilder.py Debian${debian_version}

    docker_image="rust-builder-udslauncher:Debian${debian_version}"
    # Debian based build inside docker

    echo "=== Building for Debian ${debian_version} using ${docker_image} ==="

    docker run --rm \
    -u ${UID_HOST}:${GID_HOST} \
    -e IN_DOCKER=1 \
    -e DISTRO=Debian${debian_version} \
    -v $crate:/crate \
    -w /crate/building/linux \
    $docker_image \
    dpkg-buildpackage -b -us -uc

    # Move to ../bin/debian${debian_version}
    outdir="${top}/../bin/debian${debian_version}"
    mkdir -p ${outdir}
    rm -f ${outdir}/udslauncher_*.deb
    mv ${top}/../udslauncher_*.deb ${outdir}/
done

for DISTRO in Fedora openSUSE; do
    # We need to execute manually the Makefile to copy install files
    DISTRO_LOWER=$(echo ${DISTRO} | tr '[:upper:]' '[:lower:]')
    RPMROOT=${top}/rpm-${DISTRO_LOWER}
    INSTALLROOT=${top}/rpm-${DISTRO_LOWER}-root
    
    echo "=== Preparing install files for ${DISTRO} ==="
    rm -rf "${INSTALLROOT}"
    mkdir -p ${INSTALLROOT}
    make -C ${top} install-udslauncher \
    IN_DOCKER=0 \
    DISTRO=${DISTRO} \
    DESTDIR=${INSTALLROOT}
    
    echo "=== Preparing RPM build tree ==="
    rm -rf "${RPMROOT}"
    mkdir -p ${RPMROOT}/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
    cp ${top}/udslauncher.spec ${RPMROOT}/SPECS/udslauncher.spec
    
    docker_image="rust-builder-udslauncher:${DISTRO}"
    
    echo "=== Building for ${DISTRO} using ${docker_image} ==="
    
    docker run --rm \
    -u ${UID_HOST}:${GID_HOST} \
    -e DISTRO=${DISTRO} \
    -v $crate:/crate \
    -w /crate/building/linux \
    $docker_image \
    rpmbuild -bb \
    --define "_topdir /crate/building/linux/rpm-${DISTRO_LOWER}" \
    --define "version ${VERSION}" \
    --define "release ${RELEASE}" \
    --define "DESTDIR /crate/building/linux/rpm-${DISTRO_LOWER}-root" \
    /crate/building/linux/rpm-${DISTRO_LOWER}/SPECS/udslauncher.spec
    
    # Move to ../bin/${distro}
    outdir="${top}/../bin/${DISTRO_LOWER}"
    mkdir -p ${outdir}
    rm -f ${outdir}/udslauncher-*.rpm
    cp ${top}/rpm-${DISTRO_LOWER}/RPMS/x86_64/udslauncher-*.rpm ${outdir}/
    rpm --addsign ${outdir}/udslauncher-*.rpm
done


