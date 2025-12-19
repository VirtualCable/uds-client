#!/bin/bash

VERSION=$( [ -f ../../../VERSION ] && cat ../../../VERSION || echo "devel" )
RELEASE=1

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
    -e IN_DOCKER=1 \
    -e DISTRO=Debian${debian_version} \
    -e DEB_VERSION_UPSTREAM="${VERSION}-deb${debian_version}-${RELEASE}" \
    -v $crate:/crate \
    -w /crate/building/linux \
    $docker_image \
    dpkg-buildpackage -b -us -uc
    
    # Move to ../bin/debian${debian_version}
    outdir="${top}/../bin/debian${debian_version}"
    mkdir -p ${outdir}
    mv ${top}/../udslauncher_*.deb ${outdir}/
done
exit 0
for DISTRO in Fedora openSUSE; do
    # managed an unmanaged
    for kind in managed unmanaged; do
        # convert distro for pkg name, "" if Fedora41, "suse" if openSUSE15
        case "$DISTRO" in
            Fedora)
                PKG_DISTRO=""
            ;;
            openSUSE)
                PKG_DISTRO="-suse"
            ;;
        esac
        case "$kind" in
            managed)
                PKGNAME="udsactor"
            ;;
            unmanaged)
                PKGNAME="udsactor-unmanaged"
            ;;
        esac
        PKGNAME_BASE=${PKGNAME}${PKG_DISTRO}
        PKGNAME=${PKGNAME}${PKG_DISTRO}-${VERSION}.spec
        #
        # Generate spec file
        cat udsactor-template.spec | \
        sed -e "s/version 0.0.0/version ${VERSION}/g" \
        -e "s/release 1/release ${RELEASE}/g" \
        -e "s/DISTRO=rh/DISTRO=${DISTRO}/g" \
        -e "s/%define name PKGNAME/%define name ${PKGNAME_BASE}/g" \
        > ${PKGNAME}
        
        
        # Prepare RPM structure
        rm -rf rpm
        for folder in SOURCES BUILD RPMS SPECS SRPMS; do
            mkdir -p rpm/$folder
        done
        
        # Build RPM
        rpmbuild -v -bb --clean --target x86_64 ${PKGNAME} 2>&1
    done
done

# Sign RPMs
rpm --addsign ../*.rpm
