#!/usr/bin/env python3
"""Build an AppImage from the Tauri release binary.

Usage: ./scripts/build-appimage.py [--dry-run] [--force] [--skip-download] [--output <dir>]

Requirements: cargo, npm.  The Tauri CLI is the version installed by
``npm ci`` (``package.json``); no ``cargo install tauri-cli`` is needed.
appimagetool is downloaded automatically (no FUSE needed — the AppImage is
extracted on first use) or can be skipped with --skip-download to produce an
AppDir for manual packaging.

By default the build is skipped when the binary is newer than the
sources.  Pass --force to rebuild unconditionally (CI).
"""

import argparse
import os
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path


# ---------------------------------------------------------------------------
# helpers
# ---------------------------------------------------------------------------

def info(msg: str) -> None:
    print(f"==> {msg}")


def die(msg: str) -> None:
    print(f"error: {msg}", file=sys.stderr)
    sys.exit(1)


def ensure_command(cmd: str) -> bool:
    """Return True when *cmd* is on PATH."""
    return shutil.which(cmd) is not None


def run(args: list[str], **kwargs) -> subprocess.CompletedProcess:
    """Thin wrapper around subprocess.run with info logging."""
    info(" ".join(args))
    return subprocess.run(args, check=True, **kwargs)


# ---------------------------------------------------------------------------
# configuration (derived from tauri.conf.json + Cargo.toml)
# ---------------------------------------------------------------------------

APP_NAME = "markdown-blathy"
APP_TITLE = "MarkdownBláthy"
APP_COMMENT = "Markdown, transformed — Markdown editor with Typst-powered live preview and PDF export"
APP_CATEGORIES = "Office;WordProcessor;"
BINARY_NAME = APP_NAME

ROOT = Path(__file__).resolve().parent.parent
RUST_DIR = ROOT / "src-tauri"
ICON_SRC = RUST_DIR / "icons" / "icon.png"
TOOLS_DIR = ROOT / ".local" / "tools"


def app_version() -> str:
    """Read the version from Cargo.toml — never from the binary (needs display)."""
    cargo = RUST_DIR / "Cargo.toml"
    with open(cargo, "rb") as f:
        data = tomllib.load(f)
    return data["package"]["version"]


# ---------------------------------------------------------------------------
# appimagetool discovery and download
# ---------------------------------------------------------------------------

def find_appimagetool() -> Path | None:
    """Locate appimagetool: env var, PATH, or project-local cache."""
    # 1. explicit env var
    env_path = os.environ.get("APPIMAGETOOL")
    if env_path and shutil.which(env_path):
        return Path(env_path)

    # 2. system-installed
    if shutil.which("appimagetool"):
        return Path("appimagetool")  # let subprocess resolve it

    # 3. project-local cache
    cached = TOOLS_DIR / "appimagetool-x86_64.AppImage"
    if cached.exists() and os.access(cached, os.X_OK):
        return cached

    return None


def download_appimagetool() -> Path:
    """Download the continuous AppImage and mark it executable."""
    TOOLS_DIR.mkdir(parents=True, exist_ok=True)
    dest = TOOLS_DIR / "appimagetool-x86_64.AppImage"
    url = "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
    info(f"downloading appimagetool → {dest}")

    # Try curl first, then wget.
    try:
        run(["curl", "-fsSL", url, "-o", str(dest)])
    except subprocess.CalledProcessError:
        run(["wget", "-q", "-O", str(dest), url])
    dest.chmod(0o755)
    return dest


def resolve_appimagetool(skip_download: bool) -> Path | None:
    """Return path to appimagetool, downloading if needed.

    Returns None when skip_download is True and the tool is unavailable.
    """
    tool = find_appimagetool()
    if tool is not None:
        return tool
    if skip_download:
        info("appimagetool not found; will produce AppDir only (--skip-download)")
        return None
    return download_appimagetool()


def run_appimagetool(tool_path: Path, appdir: Path, out_file: Path) -> None:
    """Run appimagetool, FUSE-free via --appimage-extract-and-run.

    When *tool_path* is an .AppImage file we use --appimage-extract-and-run
    so FUSE (not available in containers/CI) is never required.  When it is
    a native binary on PATH we call it directly.
    """
    if tool_path.name.endswith(".AppImage") or ".AppImage" in str(tool_path):
        # FUSE-free: extract and run the appimagetool AppImage in one shot.
        run(
            [str(tool_path), "--appimage-extract-and-run",
             str(appdir), str(out_file)],
            env={**os.environ, "ARCH": "x86_64"},
        )
    else:
        run(
            [str(tool_path), str(appdir), str(out_file)],
            env={**os.environ, "ARCH": "x86_64"},
        )


# ---------------------------------------------------------------------------
# build step
# ---------------------------------------------------------------------------

def rebuild_needed(binary: Path, force: bool) -> bool:
    """True when the release binary needs (re)building."""
    if force:
        return True
    if not binary.exists() or not os.access(binary, os.X_OK):
        return True

    # Check if any Rust source or Cargo.toml is newer than the binary.
    src_dir = RUST_DIR / "src"
    cargo_toml = RUST_DIR / "Cargo.toml"
    for base in [src_dir, cargo_toml]:
        if base.is_file():
            if base.stat().st_mtime > binary.stat().st_mtime:
                return True
        elif base.is_dir():
            for f in base.rglob("*"):
                if f.is_file() and f.stat().st_mtime > binary.stat().st_mtime:
                    return True

    return False


def build_binary(force: bool) -> Path:
    """Build the production binary via the npm-installed Tauri CLI.

    The build must go through the Tauri CLI (it sets the production mode,
    ``tauri://localhost`` custom protocol; a plain ``cargo build --release``
    can leave the binary in dev mode).  CI installs the CLI from
    ``package.json`` via ``npm ci``, so invoke the npm script rather than the
    ``cargo tauri`` subcommand — the latter requires a separate
    ``cargo install tauri-cli`` and fails in CI with "no such command: tauri".
    The binary lands at the well-known location: src-tauri/target/release/<name>.
    """
    binary = RUST_DIR / "target" / "release" / BINARY_NAME

    if rebuild_needed(binary, force):
        info("building production binary (npm run tauri -- build --bundles deb)")
        # Tauri's beforeBuildCommand (npm run build) covers the frontend.
        run(
            ["npm", "run", "tauri", "--", "build", "--bundles", "deb"],
            cwd=str(ROOT),
        )
    else:
        info("binary is current; skipping build (use --force to rebuild)")

    if not binary.exists() or not os.access(binary, os.X_OK):
        die(f"binary not found: {binary}")
    return binary


# ---------------------------------------------------------------------------
# AppDir assembly
# ---------------------------------------------------------------------------

def create_appdir(binary: Path, out_dir: Path) -> Path:
    """Build the AppDir tree.  Returns the AppDir path."""
    version = app_version()
    info(f"app version: {version}")

    appdir = out_dir / f"{APP_NAME}.AppDir"
    info(f"creating AppDir: {appdir}")
    if appdir.exists():
        shutil.rmtree(appdir)

    (appdir / "usr" / "bin").mkdir(parents=True)
    (appdir / "usr" / "share" / "applications").mkdir(parents=True)
    icon_dir = appdir / "usr" / "share" / "icons" / "hicolor" / "256x256" / "apps"
    icon_dir.mkdir(parents=True)

    # binary
    dest_bin = appdir / "usr" / "bin" / BINARY_NAME
    shutil.copy2(binary, dest_bin)
    dest_bin.chmod(0o755)

    # AppRun
    apprun = appdir / "AppRun"
    apprun.write_text(
        "#!/usr/bin/env bash\n"
        'HERE="$(dirname "$(readlink -f "$0")")"\n'
        f'exec "$HERE/usr/bin/{BINARY_NAME}" "$@"\n'
    )
    apprun.chmod(0o755)

    # icon — resize to 256×256 if ImageMagick is available
    if ICON_SRC.is_file():
        if ensure_command("convert"):
            run(["convert", str(ICON_SRC), "-resize", "256x256",
                 str(icon_dir / f"{APP_NAME}.png")])
        else:
            info("ImageMagick 'convert' not found; copying icon at original size")
            shutil.copy2(ICON_SRC, icon_dir / f"{APP_NAME}.png")
    else:
        die(f"icon not found: {ICON_SRC}")
    # Root-level copy that appimagetool looks for.
    shutil.copy2(icon_dir / f"{APP_NAME}.png", appdir / f"{APP_NAME}.png")

    # .desktop file
    desktop_path = appdir / "usr" / "share" / "applications" / f"{APP_NAME}.desktop"
    desktop_path.write_text(
        "[Desktop Entry]\n"
        "Type=Application\n"
        f"Name={APP_TITLE}\n"
        f"Exec={BINARY_NAME}\n"
        f"Icon={APP_NAME}\n"
        f"Comment={APP_COMMENT}\n"
        f"Categories={APP_CATEGORIES}\n"
        "Terminal=false\n"
        f"X-AppImage-Version={version}\n"
    )
    # Symlink at root — relative so it works inside the squashfs.
    root_link = appdir / f"{APP_NAME}.desktop"
    if root_link.exists(follow_symlinks=False):
        root_link.unlink()
    # appimagetool prefers root .desktop to *not* be a symlink (it reads
    # the file during packaging).  Copy instead.
    shutil.copy2(desktop_path, root_link)

    return appdir


# ---------------------------------------------------------------------------
# validation
# ---------------------------------------------------------------------------

def validate_appdir(appdir: Path) -> bool:
    """Check that every required file exists and the .desktop is valid."""
    info("validating AppDir")
    ok = True

    for name in ["AppRun", f"{APP_NAME}.desktop", f"{APP_NAME}.png",
                 f"usr/bin/{BINARY_NAME}"]:
        if (appdir / name).exists():
            print(f"  \u2713 {name}")
        else:
            print(f"  \u2717 missing: {name}")
            ok = False

    desktop = appdir / f"{APP_NAME}.desktop"
    if desktop.is_file():
        content = desktop.read_text()
        if "Type=Application" in content:
            print("  \u2713 .desktop has Type=Application")
        else:
            print("  \u2717 .desktop missing Type=Application")
            ok = False

    if ok:
        print("  \u2713 AppDir structure valid")
    return ok


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(description="Build an AppImage for MarkdownBláthy")
    parser.add_argument("--dry-run", action="store_true",
                        help="Assemble the AppDir but skip packaging")
    parser.add_argument("--force", action="store_true",
                        help="Force a full rebuild of the binary")
    parser.add_argument("--skip-download", action="store_true",
                        help="Produce the AppDir without downloading appimagetool")
    parser.add_argument("--output", default=str(ROOT / "target" / "appimage"),
                        help="Output directory (default: target/appimage)")
    args = parser.parse_args()

    out_dir = Path(args.output)

    # prerequisites
    for cmd in ("cargo", "npm"):
        if not ensure_command(cmd):
            die(f"'{cmd}' not found on PATH")

    appimagetool = resolve_appimagetool(args.skip_download)
    if appimagetool:
        info(f"appimagetool ready: {appimagetool}")

    # build
    binary = build_binary(args.force)

    # assemble AppDir
    appdir = create_appdir(binary, out_dir)

    # validate
    if not validate_appdir(appdir):
        die("AppDir validation failed")

    if args.dry_run:
        info(f"dry-run: AppDir ready at {appdir} (not packaged)")
        return

    if appimagetool is None:
        info(f"no appimagetool; AppDir ready at {appdir} (not packaged)")
        info(f"to package manually: appimagetool {appdir}")
        return

    # package
    version = app_version()
    appimage_out = out_dir / f"{APP_NAME}-{version}-x86_64.AppImage"
    info(f"packaging AppImage → {appimage_out}")
    out_dir.mkdir(parents=True, exist_ok=True)

    run_appimagetool(appimagetool, appdir, appimage_out)

    if appimage_out.is_file():
        size_mb = appimage_out.stat().st_size / (1024 * 1024)
        info(f"AppImage built: {appimage_out} ({size_mb:.0f} MB)")
    else:
        die("AppImage was not produced")


if __name__ == "__main__":
    main()
