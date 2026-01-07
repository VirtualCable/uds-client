#!/bin/bash

VERSION=$( [ -f ../../../VERSION ] && cat ../../../VERSION || echo "devel" )
RELEASE=1

UID_HOST=$(id -u)
GID_HOST=$(id -g)
# Architecture
ARCH=$(uname -m)

TOP=$(pwd)
# Resolve $TOP/../..
CRATE_ROOT=$(realpath ${TOP}/../..)


for debian_version in 12 13; do
    # Compile first the binary using rustbuilder.py
    echo "=== Building udslauncher binary using rustbuilder.py ==="
    cd ${TOP}
    python3 rustbuilder.py Debian${debian_version}
    
    docker_image="rust-builder-udslauncher:Debian${debian_version}"
    # Debian based build inside docker
    
    echo "=== Building for Debian ${debian_version} using ${docker_image} ==="
    
    # Build the deb package, disable fakeroot beceuse on docker it takes a lot of time
    docker run --rm \
    -u ${UID_HOST}:${GID_HOST} \
    -e IN_DOCKER=1 \
    -e DISTRO=Debian${debian_version} \
    -v ${CRATE_ROOT}:/crate \
    -w /crate/building/linux \
    $docker_image \
    dpkg-buildpackage -b -us -uc -rfakeroot
    
    # Move to ../bin/debian${debian_version}
    outdir="${TOP}/../bin/debian${debian_version}"
    mkdir -p ${outdir}
    rm -f ${outdir}/udslauncher_*.deb
    mv ${TOP}/../udslauncher_*.deb ${outdir}/
done

for distro in Fedora openSUSE; do
    # We need to execute manually the Makefile to copy install files
    distro_lower=$(echo $distro | tr '[:upper:]' '[:lower:]')
    rpm_root=${TOP}/rpm-${distro_lower}
    install_root=${TOP}/rpm-${distro_lower}-root
    
    echo "=== Preparing install files for $distro ==="
    rm -rf "$install_root"
    mkdir -p $install_root
    make -C ${TOP} install-udslauncher \
    IN_DOCKER=0 \
    DISTRO=$distro \
    DESTDIR=$install_root
    
    echo "=== Preparing RPM build tree ==="
    rm -rf "$rpm_root"
    mkdir -p $rpm_root/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
    cp ${TOP}/udslauncher.spec $rpm_root/SPECS/udslauncher.spec
    
    docker_image="rust-builder-udslauncher:$distro"
    
    echo "=== Building for $distro using ${docker_image} ==="
    
    docker run --rm \
    -u ${UID_HOST}:${GID_HOST} \
    -e DISTRO=$distro \
    -v ${CRATE_ROOT}:/crate \
    -w /crate/building/linux \
    $docker_image \
    rpmbuild -bb \
    --define "_topdir /crate/building/linux/rpm-${distro_lower}" \
    --define "version ${VERSION}" \
    --define "release ${RELEASE}" \
    --define "DESTDIR /crate/building/linux/rpm-${distro_lower}-root" \
    /crate/building/linux/rpm-${distro_lower}/SPECS/udslauncher.spec
    
    # Move to ../bin/${distro}
    outdir="${TOP}/../bin/${distro_lower}"
    mkdir -p ${outdir}
    rm -f ${outdir}/udslauncher-*.rpm
    cp ${TOP}/rpm-${distro_lower}/RPMS/${ARCH}/udslauncher-*.rpm ${outdir}/
    # rpm --addsign ${outdir}/udslauncher-*.rpm
done


