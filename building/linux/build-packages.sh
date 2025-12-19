#!/bin/bash

VERSION=$( [ -f ../../../VERSION ] && cat ../../../VERSION || echo "devel" )
RELEASE=1

top=$(pwd)

# Debian based
dpkg-buildpackage -b

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
