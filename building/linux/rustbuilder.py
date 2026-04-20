#!/usr/bin/env python3
"""
Rust crate builder for multiple Linux distributions using Docker.
"""

import os
import sys
import subprocess
import pathlib
import typing
import argparse
import shutil

# === Type aliases ===
PathLike = str | pathlib.Path


def get_target_path(target_root: PathLike, debug: bool) -> pathlib.Path:
    """Get the target path for the build output."""
    target_root = pathlib.Path(target_root).resolve()
    return target_root / ("debug" if debug else "release")


def get_isolated_target_root(crate_path: PathLike, distro: str) -> pathlib.Path:
    """Return the per-distro target directory used during Docker builds."""
    crate_path = pathlib.Path(crate_path).resolve()
    return crate_path / "target" / "rustbuilder" / distro


def remove_target_root(target_root: pathlib.Path, image: str | None = None) -> None:
    """Remove a target directory, falling back to Docker if permissions prevent local cleanup."""
    if not target_root.exists():
        return

    try:
        shutil.rmtree(target_root)
        return
    except PermissionError:
        if image is None:
            raise

    subprocess.run(
        [
            "docker",
            "run",
            "--rm",
            "-v",
            f"{target_root.parent}:/cleanup",
            image,
            "sh",
            "-c",
            f"rm -rf /cleanup/{target_root.name}",
        ],
        check=True,
    )


def create_target_root(target_root: pathlib.Path, image: str) -> None:
    """Create a clean target directory for an isolated build."""
    remove_target_root(target_root, image)
    target_root.mkdir(parents=True, exist_ok=True)


def get_builders() -> list[str]:
    """Get the list of available builders (distros)."""
    builders_dir = pathlib.Path("builders")
    return [d.name for d in builders_dir.iterdir() if d.is_dir()]


def build_for_distro(distro: str, crate_path: PathLike, debug: bool, extra_docker_cmd: str) -> None:
    """Build the crate for a specific distro using Docker."""
    crate_path = pathlib.Path(crate_path).resolve()
    image_tag = f"rust-builder-udslauncher:{distro}"
    target_root = get_isolated_target_root(crate_path, distro)

    try:
        create_target_root(target_root, image_tag)
        exec_builder_for_distro(distro, crate_path, debug, extra_docker_cmd, target_root)
    finally:
        remove_target_root(target_root, image_tag)


def exec_builder_for_distro(
    distro: str,
    crate_path: pathlib.Path,
    debug: bool,
    extra_docker_cmd: str,
    target_root: pathlib.Path,
) -> None:
    """Execute the Docker-backed build and export artifacts for a distro."""
    build_dir = pathlib.Path("builders") / distro
    output_dir = build_dir / "output"
    image_tag = f"rust-builder-udslauncher:{distro}"
    dockerfile = build_dir / "Dockerfile"
    stamp = build_dir / "build.stamp"

    print(f"=== [{distro}{' (debug)' if debug else ''}] ===")

    # Build Docker image if needed
    if (
        not docker_image_exists(image_tag)
        or not stamp.exists()
        or dockerfile.stat().st_mtime > stamp.stat().st_mtime
    ):
        print(f"→ Building image {image_tag}...")
        subprocess.run(["docker", "build", "-t", image_tag, str(build_dir)], check=True)
        stamp.touch()

    # Build (skip cargo build for nix-portable since Nix handles it)
    if distro != "nix-portable":
        docker_run(crate_path, image_tag, (["cargo", "build", "--release"] if not debug else ["cargo", "build"]), target_root)

    # Extra command inside docker
    if extra_docker_cmd:
        docker_run(
            crate_path,
            image_tag,
            [
                "sh",
                "-c",
                extra_docker_cmd.replace("[TARGET]", "/crate/target/" + ("debug" if debug else "release")),
            ],
            target_root,
        )

    output_dir.mkdir(parents=True, exist_ok=True)
    release_dir = get_target_path(target_root, debug)

    # Copy binaries and .so files
    executables = [f for f in release_dir.iterdir() if f.is_file() and os.access(f, os.X_OK)]
    so_files = [f for f in release_dir.iterdir() if f.suffix == ".so"]
    symlinks = [f for f in release_dir.iterdir() if f.is_symlink() and '.so' in f.suffixes]

    if not executables:
        raise RuntimeError("No executables found in target/release")

    def copy(src: PathLike, dest: PathLike) -> None:
        src = pathlib.Path(src).resolve()
        dest = pathlib.Path(dest).resolve()
        print(f"→ Copying {src} to {dest}")
        dest.write_bytes(src.read_bytes())

    # Clean output directory
    for item in output_dir.iterdir():
        if item.is_file() or item.is_symlink():
            item.unlink()
        elif item.is_dir():
            subprocess.run(["rm", "-rf", str(item)], check=True)

    # Copy executables
    for exe in executables:
        copy(exe, output_dir / exe.name)
        # Strip binaries to reduce size (with local stripping tool)
        if not debug:
            subprocess.run(["strip", output_dir / exe.name], check=True)
        # Also, ensure executable permissions
        os.chmod(output_dir / exe.name, 0o755)

    # Copy .so files
    for so in so_files:
        copy(so, output_dir / so.name)
        # Strip .so files to reduce size
        subprocess.run(["strip", output_dir / so.name], check=True)

    # create symlinks for .so.X.Y files on
    for symlink in symlinks:
        target = symlink.resolve()
        link_name = output_dir / symlink.name
        target_name = output_dir / target.name
        print(f"→ Creating symlink {link_name} -> {target_name}")
        if not target_name.exists():
            raise RuntimeError(f"Target for symlink does not exist: {target_name}")
        link_name.symlink_to(target_name.name)


def docker_image_exists(image: str) -> bool:
    """Check if Docker image exists."""
    result = subprocess.run(
        ["docker", "image", "inspect", image], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
    )
    return result.returncode == 0


def docker_run(
    crate_path: pathlib.Path, image: str, command: list[str], target_root: pathlib.Path | None = None
) -> None:
    """Run a command inside Docker."""
    uid = os.getuid()
    gid = os.getgid()

    docker_command = [
        "docker",
        "run",
        "--rm",
        "-e",
        f"USER_ID={uid}",
        "-e",
        f"GROUP_ID={gid}",
    ]

    # Add environment variables from os.environ for PNAME and VERSION if they exist
    for key in ["PNAME", "VERSION", "TARGET_DIR"]:
        if key in os.environ:
            docker_command += ["-e", f"{key}={os.environ[key]}"]

    docker_command += ["-v", f"{crate_path}:/crate"]

    if target_root is not None:
        docker_command += ["-v", f"{target_root}:/crate/target"]

    subprocess.run(
        docker_command
        + [
            "-w",
            "/crate",
            image,
        ]
        + command,
        check=True,
    )


def main() -> None:
    if len(sys.argv) < 2:
        print("Usage: build.py <distro> <crate_path>")
        sys.exit(1)

    # Valid distributions
    valid_distros = get_builders()

    parser = argparse.ArgumentParser(description="Build Rust crate for specified Linux distro.")
    parser.add_argument("distro", type=str, choices=valid_distros, help="Target Linux distribution")
    parser.add_argument(
        "crate_path", type=str, help="Path to the Rust crate to build", default="../..", nargs="?"
    )
    parser.add_argument(
        "-d",
        "--debug",
        required=False,
        dest="debug",
        action="store_true",
        help="Compile in debug mode",
        default=False,
    )

    args = parser.parse_args()

    distro: str = args.distro
    crate_path = pathlib.Path(args.crate_path).resolve()
    debug: bool = args.debug
    # On debian 12 and openSUSE, copy .so files after build to target. Also, create the symlinks
    # to them for .so.X.Y files.
    SCRIPT: typing.Final[str] = (
        "cp /usr/local/lib/*so [TARGET] && "
        "cd [TARGET] && "
        "for f in /usr/local/lib/*.so.*; do ln --force -s \"${f%%.*}.so\" \"$(basename $f)\"; done"
    )

    NIX_SCRIPT: typing.Final[str] = (
        "TARGET_DIR=[TARGET] /usr/local/bin/build-nix.sh"
    )

    extra_docker_cmd: str = {
        "Debian12": SCRIPT,
        "openSUSE": SCRIPT.replace("/usr/local/lib", "/usr/local/lib64"),
        "nix-portable": NIX_SCRIPT,
    }.get(distro, "")

    # Set defaults for PNAME and VERSION if not present
    if "PNAME" not in os.environ:
        os.environ["PNAME"] = "udslauncher"
    if "VERSION" not in os.environ:
        # Try to read version from ../../../VERSION
        version_file = crate_path.parent.parent / "VERSION"
        if version_file.exists():
            os.environ["VERSION"] = version_file.read_text().strip()
        else:
            os.environ["VERSION"] = "5.0.0"

    build_for_distro(distro, crate_path, debug, extra_docker_cmd)
    print("=== Build completed ===")


if __name__ == "__main__":
    main()
