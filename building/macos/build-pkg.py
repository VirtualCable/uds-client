#!/usr/bin/env python3
import subprocess
import shutil
import typing
from pathlib import Path
import os
import sys

# Constants
SCRIPT_DIR: typing.Final[Path] = Path(__file__).resolve().parent
WORKSPACE_ROOT: typing.Final[Path] = SCRIPT_DIR.parent.parent
OUTPUT_DIR: typing.Final[Path] = SCRIPT_DIR / "dist"
APP_NAME: typing.Final[str] = "UDSLauncher.app"
APP_DIR: typing.Final[Path] = OUTPUT_DIR / APP_NAME


VERSION_FILE: typing.Final[Path] = WORKSPACE_ROOT.parent / "VERSION"
VERSION: typing.Final[str] = VERSION_FILE.read_text().strip() if VERSION_FILE.exists() else "DEVEL"
BINARIES: typing.Final[list[str]] = [
    "mac-launcher",
    "launcher",
]

RPATH_SEARCH_DIRS: typing.Final[list[Path]] = [
    Path("/opt/homebrew/lib"),
    Path("/usr/local/lib"),
    # ... add more if needed
]


FREERDP_BASE_LIBS: typing.Final[list[str]] = [
    "libfreerdp3.dylib",
    "libfreerdp-client3.dylib",
    "libwinpr3.dylib",
    "libwinpr-tools3.dylib",
]

# Default FREERDP_ROOT to /usr/local if not set
FREERDP_ROOT: Path = Path(os.environ.get("FREERDP_ROOT", "/usr/local/"))


# Hook for every binary after creation
def process_binary_hook(binary_path: Path) -> None:
    hook = os.environ.get("UDS_PROCESS_BINARY")
    if hook:
        print(f"[HOOK] Processing {binary_path.name} with {hook}")
        subprocess.run([hook, str(binary_path.resolve())], check=True)
    else:
        print(f"[HOOK] No binary hook defined for {binary_path.name}")


# Hook for the final package after creation
def process_pkg_hook(pkg_path: Path) -> None:
    hook = os.environ.get("UDS_PROCESS_PACKAGE")
    if hook:
        print(f"[HOOK] Processing package {pkg_path.name} with {hook}")
        subprocess.run([hook, str(pkg_path.resolve())], check=True)
    else:
        print(f"[HOOK] No package hook defined for {pkg_path.name}")


def remove_if_exists(path: Path) -> None:
    if path.exists():
        print(f"[CLEAN] Removing {path}")
        if path.is_dir():
            shutil.rmtree(path)
        else:
            path.unlink()


def get_macos_dependencies(binary: Path) -> list[str]:
    """
    Return the list of dynamic library dependencies for a given macOS binary
    using `otool -L`. The binary may be a symlink; otool will resolve it.
    """
    result = subprocess.run(
        ["otool", "-L", str(binary)],
        capture_output=True,
        text=True,
        check=True,
    )

    deps: list[str] = []
    lines = result.stdout.splitlines()

    # Skip first line (binary itself)
    for line in lines[1:]:
        line = line.strip()
        if not line:
            continue

        dep = line.split(" ", 1)[0]
        deps.append(dep)

    return deps


def resolve_rpath_dylib(dep: str) -> 'Path | None':
    """
    Resolve an @rpath dependency by searching known library directories.
    Returns the real path if found, otherwise None.
    """
    assert dep.startswith("@rpath/"), f"Not an @rpath entry: {dep}"

    filename = dep.split("/", 1)[1]  # libsharpyuv.0.dylib

    for base in RPATH_SEARCH_DIRS:
        candidate = base / filename
        if candidate.exists():
            return candidate

    return None


def collect_dependencies(
    binary: Path,
    processed: set[str] | None = None,
    level: int = 0,
) -> set[str]:

    indent = "  " * level

    if processed is None:
        processed = set()

    print(f"{indent}>> Checking: {binary}")

    deps = get_macos_dependencies(binary)

    for dep in deps:

        # --- NEW: resolve @rpath entries ---
        if dep.startswith("@rpath/"):
            print(f"{indent}   - Resolving @rpath: {dep}")
            resolved = resolve_rpath_dylib(dep)
            if resolved is None:
                raise RuntimeError(f"Could not resolve @rpath dependency: {dep}")
            dep = str(resolved)
            print(f"{indent}     -> Resolved to: {dep}")

        # Skip system libs
        if dep.startswith("/usr/lib/") or dep.startswith("/System/Library/"):
            print(f"{indent}   - Skipping system lib: {dep}")
            continue

        # Skip already processed
        if dep in processed:
            print(f"{indent}   - Already processed: {dep}")
            continue

        print(f"{indent}   - External dependency: {dep}")
        processed.add(dep)

        dep_path = Path(dep)
        if dep_path.exists():
            collect_dependencies(dep_path, processed, level + 1)
        else:
            raise RuntimeError(f"{indent}ERROR: dependency not found on disk: {dep}")

    return processed


def copy_external_dependency(
    dep_path: Path,
    dst_lib_dir: Path,
    copied: set[str],
    strip: bool = True,
) -> None:
    """
    Copy only the real file of an external dependency.
    Do NOT recreate symlinks (Homebrew symlinks can be circular).
    """

    real_name = dep_path.name

    # Avoid duplicates
    if real_name in copied:
        print(f">> Skipping duplicate: {real_name}")
        return

    if not dep_path.exists():
        raise FileNotFoundError(f"Dependency not found: {dep_path}")

    dst_lib_dir.mkdir(parents=True, exist_ok=True)

    real_dst = dst_lib_dir / real_name

    # Copy real file only
    shutil.copy2(dep_path, real_dst)
    real_dst.chmod(0o755)
    print(f">> Copied external lib: {real_dst}")

    # Strip
    if strip:
        try:
            subprocess.run(["strip", "-x", str(real_dst)], check=True)
            print(f">> Stripped: {real_dst}")
        except Exception as exc:
            print(f">> WARN: strip failed for {real_dst}: {exc}")

    copied.add(real_name)


def fix_install_names(binary: Path) -> None:
    """
    Rewrite install_name and dependency paths for a binary inside the app bundle.
    - Sets the binary's own install_name to @rpath/<filename> if it's a dylib.
    - Rewrites external dependencies to @rpath/<filename>.
    - Rewrites dependencies for executables to @executable_path/../Frameworks/<filename>.
    """

    print(f">> Fixing install names for: {binary}")

    is_dylib = binary.suffix == ".dylib"

    # Fix the binary's own install_name (only for dylibs)
    if is_dylib:
        new_id = f"@rpath/{binary.name}"
        try:
            subprocess.run(
                ["install_name_tool", "-id", new_id, str(binary)],
                check=True,
            )
            print(f"   - Set install_name to {new_id}")
        except Exception as exc:
            print(f"   - WARN: failed to set install_name: {exc}")

    # Fix dependency paths
    deps = get_macos_dependencies(binary)

    for dep in deps:
        if dep.startswith("@rpath/"):
            continue
        if dep.startswith("/usr/lib/") or dep.startswith("/System/Library/"):
            continue

        dep_name = Path(dep).name

        if is_dylib:
            new_path = f"@rpath/{dep_name}"
        else:
            new_path = f"@executable_path/../Frameworks/{dep_name}"

        try:
            subprocess.run(
                ["install_name_tool", "-change", dep, new_path, str(binary)],
                check=True,
            )
            print(f"   - Rewrote {dep} -> {new_path}")
        except Exception as exc:
            print(f"   - WARN: failed to rewrite {dep}: {exc}")

    # Invoke hook here
    if binary.suffix == ".dylib": 
        process_binary_hook(binary)    


def copy_freerdp_lib(
    lib_name: str,
    src_lib_dir: Path,
    dst_lib_dir: Path,
) -> None:
    """
    Copy a FreeRDP library preserving its full versioned structure:
    - base symlink (e.g., libfreerdp3.dylib)
    - intermediate symlink (e.g., libfreerdp3.0.dylib)
    - real file (e.g., libfreerdp3.0.0.dylib)

    The real file is stripped using `strip -x` to reduce size.
    Symlinks are recreated inside the destination directory.
    """

    # Base symlink (e.g., libfreerdp3.dylib)
    base = src_lib_dir / lib_name
    if not base.exists():
        raise FileNotFoundError(f"Library not found: {base}")
    if not base.is_symlink():
        raise RuntimeError(f"{base} is not a symlink; unexpected library structure")

    # Get intermediate symlink (e.g., libfreerdp3.0.dylib)
    intermediate_name = base.readlink()
    intermediate = src_lib_dir / intermediate_name
    if not intermediate.is_symlink():
        raise RuntimeError(f"{intermediate} is not a symlink; unexpected library structure")

    # Resolve real file (e.g., libfreerdp3.0.0.dylib)
    real_name = intermediate.readlink()
    real = src_lib_dir / real_name
    if not real.is_file():
        raise RuntimeError(f"{real} is not a file; unexpected library structure")

    dst_lib_dir.mkdir(parents=True, exist_ok=True)

    # Copy real file
    real_dst = dst_lib_dir / real.name
    shutil.copy2(real, real_dst)

    # Strip real file to reduce size
    try:
        subprocess.run(["strip", "-x", str(real_dst)], check=True)
        print(f">> Stripped: {real_dst}")
    except Exception as exc:
        print(f">> WARN: strip failed for {real_dst}: {exc}")

    # Recreate intermediate symlink
    intermediate_dst = dst_lib_dir / intermediate.name
    if intermediate_dst.exists():
        intermediate_dst.unlink()
    intermediate_dst.symlink_to(real_dst.name)
    print(f">> Symlink {intermediate_dst} -> {real_dst.name}")

    # Recreate base symlink
    base_dst = dst_lib_dir / base.name
    if base_dst.exists():
        base_dst.unlink()
    base_dst.symlink_to(intermediate_dst.name)
    print(f">> Symlink {base_dst} -> {intermediate_dst.name}")


def fail(msg: str) -> typing.NoReturn:
    print(f"ERROR: {msg}", file=sys.stderr)
    sys.exit(1)


def ensure_freerdp_libs() -> None:
    lib_dir = FREERDP_ROOT / "lib"
    if not lib_dir.is_dir():
        fail(f"FREERDP_ROOT directory does not exist or lacks 'lib': {lib_dir}")

    for lib in FREERDP_BASE_LIBS:
        lib_path = lib_dir / lib
        if not lib_path.is_file():
            fail(f"Required FreeRDP library not found: {lib_path}")


def validate_bundle_dependencies(app_dir: Path) -> bool:
    """
    Validate that all binaries and dylibs inside the app bundle only depend on:
      - @rpath/...
      - /usr/lib/...
      - /System/Library/...
    Returns True if everything is valid, False otherwise.
    """

    print("==> Validating bundle dependencies")

    valid_prefixes = (
        "@rpath/",
        "/usr/lib/",
        "/System/Library/",
    )

    # Collect all binaries and dylibs inside MacOS/ and Frameworks/
    binaries: list[Path] = []

    for subdir in ["Contents/MacOS", "Contents/Frameworks"]:
        for path in (app_dir / subdir).glob("*"):
            if path.is_file() and os.access(path, os.X_OK):
                binaries.append(path)

    all_ok = True

    for binary in binaries:
        print(f">> Checking: {binary}")
        deps = get_macos_dependencies(binary)

        for dep in deps:
            if dep.startswith(valid_prefixes):
                continue

            print(f"   !! INVALID dependency: {dep}")
            all_ok = False

    if all_ok:
        print("==> Bundle validation PASSED")
    else:
        print("==> Bundle validation FAILED")

    return all_ok


def build_pkg() -> Path:
    print("=== Building .pkg ===")
    pkgname = f"UDSLauncher-{VERSION}.pkg"
    pkg_path = OUTPUT_DIR / pkgname

    subprocess.run(["productbuild", "--component", str(APP_DIR), "/Applications", str(pkg_path)], check=True)
    process_pkg_hook(pkg_path)

    return pkg_path


def main() -> None:
    # Ensure FreeRDP libs are present, fails if not
    ensure_freerdp_libs()

    print("==> Building Rust binaries (cargo build --release)")
    subprocess.run(
        ["cargo", "build", "--release"],
        cwd=WORKSPACE_ROOT,
        check=True,
    )

    # Determine build directory (uds-client/building/macos)

    plist_source = WORKSPACE_ROOT / "crates" / "mac-launcher" / "Info.plist"
    plist_dest = APP_DIR / "Contents" / "Info.plist"

    print("==> Cleaning dist directory")
    remove_if_exists(OUTPUT_DIR)

    print("==> Creating .app bundle structure")
    for subdir in [
        (APP_DIR / "Contents" / "MacOS"),
        (APP_DIR / "Contents" / "Frameworks"),
        (APP_DIR / "Contents" / "Resources"),
    ]:
        subdir.mkdir(parents=True, exist_ok=True)

    print("==> Copying application icon")
    icon_src = WORKSPACE_ROOT / "assets" / "macos" / "uds.icns"
    shutil.copy(icon_src, APP_DIR / "Contents" / "Resources")

    print("==> Copying Info.plist")
    if not plist_source.is_file():
        fail(f"plist file not found at: {plist_source}")
    shutil.copy(plist_source, plist_dest)

    # Copy built binaries
    print("==> Copying binaries")
    for binary in BINARIES:
        binary_src = WORKSPACE_ROOT / "target" / "release" / binary
        if not binary_src.is_file():
            fail(f"Built binary not found: {binary_src}")
        shutil.copy(binary_src, APP_DIR / "Contents" / "MacOS")

    src_lib_dir = FREERDP_ROOT / "lib"
    dst_lib_dir = APP_DIR / "Contents" / "Frameworks"
    dependencies: set[str] = set()
    for lib in FREERDP_BASE_LIBS:
        copy_freerdp_lib(lib, src_lib_dir, dst_lib_dir)
        dependencies.update(collect_dependencies(dst_lib_dir / lib))

    print("==> Copying external dependencies")
    copied_deps: set[str] = set()
    for dep in dependencies:
        dep_path = Path(dep)
        copy_external_dependency(dep_path, dst_lib_dir, copied_deps)

    print("==> Fixing install names")

    # FreeRDP libs
    for lib in FREERDP_BASE_LIBS:
        fix_install_names(dst_lib_dir / lib)

    # External deps
    for dep_name in copied_deps:
        fix_install_names(dst_lib_dir / dep_name)

    # Executables
    fix_install_names(APP_DIR / "Contents" / "MacOS" / "mac-launcher")
    fix_install_names(APP_DIR / "Contents" / "MacOS" / "launcher")

    # Validate bundle
    if not validate_bundle_dependencies(APP_DIR):
        fail("App bundle contains invalid dependencies")

    print("==> App bundle structure created successfully")
    print(f"Output path: {APP_DIR}")
    
    print("==> Final hook processing for executables")
    for exe in ["mac-launcher", "launcher"]:
        process_binary_hook(APP_DIR / "Contents" / "MacOS" / exe)

    # Build .pkg
    pkg_name = build_pkg()
    print(f"Package created at: {pkg_name}")

    # If an argument is given, move the package to that location
    if len(sys.argv) > 1:
        destination = Path(sys.argv[1]).resolve()
        print(f"Moving package to {destination}")
        # If already exists, remove
        shutil.copy(pkg_name, destination)


if __name__ == "__main__":
    main()
