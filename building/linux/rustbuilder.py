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
import collections.abc

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
        if image is None or not docker_image_exists(image):
            raise

    # This is due to permissions from the user. Use absolute path for Docker volume mount.
    abs_parent = target_root.parent.resolve()
    subprocess.run(
        [
            "docker",
            "run",
            "--rm",
            "-v",
            f"{abs_parent}:/cleanup",
            image,
            "sh",
            "-c",
            f"rm -rf /cleanup/{target_root.name}",
        ],
        check=False,
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
    stamp = build_dir / "build.stamp"

    print(f"=== [{distro}{' (debug)' if debug else ''}] ===")

    # Build Docker image if needed
    dockerfile = build_dir / "Dockerfile"
    needs_rebuild = (
        not docker_image_exists(image_tag)
        or not stamp.exists()
        or (dockerfile.exists() and dockerfile.stat().st_mtime > stamp.stat().st_mtime)
    )
    if not needs_rebuild:
        for item in build_dir.iterdir():
            if item.is_file() and item.name != "build.stamp" and item.name != "output":
                if item.stat().st_mtime > stamp.stat().st_mtime:
                    needs_rebuild = True
                    break

    if needs_rebuild:
        print(f"→ Building image {image_tag}...")
        subprocess.run(["docker", "build", "-t", image_tag, str(build_dir)], check=True)
        stamp.touch()

    # Artifact handlers
    def handle_standard_artifacts() -> None:
        release_dir = get_target_path(target_root, debug)
        executables = [f for f in release_dir.iterdir() if f.is_file() and os.access(f, os.X_OK)]
        so_files = [f for f in release_dir.iterdir() if f.suffix == ".so"]
        symlinks = [f for f in release_dir.iterdir() if f.is_symlink() and '.so' in f.suffixes]

        if not executables:
            raise RuntimeError(f"No executables found in {release_dir}")

        def copy(src: PathLike, dest: PathLike) -> None:
            src = pathlib.Path(src).resolve()
            dest = pathlib.Path(dest).resolve()
            print(f"→ Copying {src} to {dest}")
            dest.write_bytes(src.read_bytes())

        # Copy executables
        for exe in executables:
            copy(exe, output_dir / exe.name)
            if not debug:
                subprocess.run(["strip", output_dir / exe.name], check=True)
            os.chmod(output_dir / exe.name, 0o755)

        # Copy .so files
        for so in so_files:
            copy(so, output_dir / so.name)
            subprocess.run(["strip", output_dir / so.name], check=True)

        # Create symlinks
        for symlink in symlinks:
            target = symlink.resolve()
            link_name = output_dir / symlink.name
            target_name = output_dir / target.name
            print(f"→ Creating symlink {link_name} -> {target_name}")
            if not target_name.exists():
                raise RuntimeError(f"Target for symlink does not exist: {target_name}")
            link_name.symlink_to(target_name.name)

    def handle_appimage_artifacts() -> None:
        # AppImage is generated in the crate_path (root of project)
        print("→ Moving AppImage artifacts to output directory")
        for item in crate_path.iterdir():
            if item.is_file():
                if item.name.endswith(".AppImage"):
                    # Move to output_dir
                    dest = output_dir / item.name
                    print(f"  Moving {item.name} to {dest}")
                    shutil.move(str(item), str(dest))
                elif item.name.endswith(".zsync"):
                    # Clean up unwanted zsync files
                    print(f"  Removing unwanted {item.name}")
                    item.unlink()

    # Clean output directory BEFORE build to avoid permission issues and ensure fresh results
    remove_target_root(output_dir, image_tag)
    output_dir.mkdir(parents=True, exist_ok=True)

    # Execute build steps
    build_cmd = ["cargo", "build", "--release"] if not debug else ["cargo", "build"]
    docker_run(crate_path, image_tag, build_cmd, target_root)

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

    # Dispatch artifact handling
    handlers: typing.Final[dict[str, collections.abc.Callable[[], None]]] = {
        "AppImage": handle_appimage_artifacts,
    }
    handlers.get(distro, handle_standard_artifacts)()


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
    for key in ["PNAME", "VERSION", "TARGET_DIR", "NP_GIT"]:
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

    extra_docker_cmd: str = {
        "Debian12": SCRIPT,
        "AppImage": "/usr/local/bin/build-appimage.sh",
        "openSUSE": SCRIPT.replace("/usr/local/lib", "/usr/local/lib64"),
    }.get(distro, "")

    build_for_distro(distro, crate_path, debug, extra_docker_cmd)
    print("=== Build completed ===")


if __name__ == "__main__":
    main()
