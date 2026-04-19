#!/bin/bash

VERSION=$( [ -f ../../../VERSION ] && cat ../../../VERSION || echo "devel" )
RELEASE=1

UID_HOST=$(id -u)
GID_HOST=$(id -g)
# Architecture
ARCH=$(uname -m)

TOP=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# Resolve $TOP/../..
CRATE_ROOT=$(realpath "${TOP}/../..")

build_debian_based() {
    local debian_version="$1"
    local docker_image="rust-builder-udslauncher:Debian${debian_version}"
    local outdir="${TOP}/../bin/debian${debian_version}"
    local debian_lock_file="${TOP}/.build-packages-debian.lock"
    local lock_fd

    exec {lock_fd}>"${debian_lock_file}" || return 1
    flock "${lock_fd}" || return 1

    echo "=== Building udslauncher binary using rustbuilder.py for Debian ${debian_version} ==="
    cd "${TOP}" || return 1
    python3 rustbuilder.py "Debian${debian_version}" || return 1

    echo "=== Building for Debian ${debian_version} using ${docker_image} ==="

    # Build the deb package, disable fakeroot because on Docker it takes a lot of time.
    docker run --rm \
    -e USER_ID=${UID_HOST} \
    -e GROUP_ID=${GID_HOST} \
    -e IN_DOCKER=1 \
    -e DISTRO=Debian${debian_version} \
    -v ${CRATE_ROOT}:/crate \
    -w /crate/building/linux \
    "${docker_image}" \
    sh -c "dpkg-buildpackage -b -us -uc && chown ${UID_HOST}:${GID_HOST} /crate/building/udslauncher_*" || return 1

    mkdir -p "${outdir}"
    rm -f "${outdir}"/udslauncher_*.deb
    mv "${TOP}"/../udslauncher_*.deb "${outdir}"/ || return 1

    flock -u "${lock_fd}"
}

build_rpm_based() {
    local distro="$1"
    local distro_lower
    local rpm_root
    local install_root
    local docker_image="rust-builder-udslauncher:${distro}"
    local outdir

    distro_lower=$(printf '%s' "$distro" | tr '[:upper:]' '[:lower:]')
    rpm_root="${TOP}/rpm-${distro_lower}"
    install_root="${TOP}/rpm-${distro_lower}-root"
    outdir="${TOP}/../bin/${distro_lower}"

    echo "=== Preparing install files for ${distro} ==="
    rm -rf "${install_root}"
    mkdir -p "${install_root}"
    make -C "${TOP}" install-udslauncher \
    IN_DOCKER=0 \
    DISTRO="${distro}" \
    DESTDIR="${install_root}" || return 1

    echo "=== Preparing RPM build tree for ${distro} ==="
    rm -rf "${rpm_root}"
    mkdir -p "${rpm_root}"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
    cp "${TOP}/udslauncher.spec" "${rpm_root}/SPECS/udslauncher.spec" || return 1

    echo "=== Building for ${distro} using ${docker_image} ==="

    docker run --rm \
    -u ${UID_HOST}:${GID_HOST} \
    -e DISTRO=${distro} \
    -v ${CRATE_ROOT}:/crate \
    -w /crate/building/linux \
    "${docker_image}" \
    rpmbuild -bb \
    --define "_topdir /crate/building/linux/rpm-${distro_lower}" \
    --define "version ${VERSION}" \
    --define "release ${RELEASE}" \
    --define "DESTDIR /crate/building/linux/rpm-${distro_lower}-root" \
    "/crate/building/linux/rpm-${distro_lower}/SPECS/udslauncher.spec" || return 1

    mkdir -p "${outdir}"
    rm -f "${outdir}"/udslauncher-*.rpm
    cp "${TOP}/rpm-${distro_lower}/RPMS/${ARCH}"/udslauncher-*.rpm "${outdir}"/ || return 1
}

requested_targets=("$@")

if [ ${#requested_targets[@]} -eq 0 ]; then
    requested_targets=(Debian12 Debian13 Fedora openSUSE)
fi

build_pids=()
build_names=()
recognized_targets=0
scheduled_targets=()

for target in "${requested_targets[@]}"; do
    normalized_target=$(printf '%s' "$target" | tr '[:upper:]' '[:lower:]')

    if [[ " ${scheduled_targets[*]} " == *" ${normalized_target} "* ]]; then
        continue
    fi

    case "$normalized_target" in
        debian12)
            build_debian_based 12 &
            build_pids+=("$!")
            build_names+=("Debian12")
            scheduled_targets+=("debian12")
            recognized_targets=1
            ;;
        debian13)
            build_debian_based 13 &
            build_pids+=("$!")
            build_names+=("Debian13")
            scheduled_targets+=("debian13")
            recognized_targets=1
            ;;
        fedora)
            build_rpm_based Fedora &
            build_pids+=("$!")
            build_names+=("Fedora")
            scheduled_targets+=("fedora")
            recognized_targets=1
            ;;
        opensuse)
            build_rpm_based openSUSE &
            build_pids+=("$!")
            build_names+=("openSUSE")
            scheduled_targets+=("opensuse")
            recognized_targets=1
            ;;
        *)
            echo "Unknown build target: ${target}. Supported values: debian12, debian13, fedora, opensuse." >&2
            ;;
    esac
done

if [ ${recognized_targets} -eq 0 ]; then
    exit 1
fi

failed_builds=()

# We need to execute the selected builders independently and collect failures at the end.
for i in "${!build_pids[@]}"; do
    if ! wait "${build_pids[$i]}"; then
        failed_builds+=("${build_names[$i]}")
    fi
done

if [ ${#failed_builds[@]} -ne 0 ]; then
    echo "Build failed for: ${failed_builds[*]}" >&2
    exit 1
fi


