# Development

On Windows it needs Windows 11 x64, Rust (MSVC), the Visual Studio C++ build tools, and Bun for the desktop window. On macOS and Linux it needs Rust, a C++ compiler, CMake and curl, and Bun for the desktop window; on Linux the desktop window also needs the WebKitGTK development packages ([Tauri's list](https://v2.tauri.app/start/prerequisites/#linux)).

## Build and test

On Windows:

```bash
./scripts/fetch-vendor.ps1
cargo test --workspace
cargo clippy --all-targets -- -D warnings
./scripts/package-release.ps1 -Lgpl -Desktop -Installer
```

`fetch-vendor.ps1` downloads pinned, hash-checked DLLs into `vendor/`, and `package-release.ps1` builds the release zip in `dist/`; `-Installer` also builds the installer, which needs [Inno Setup 6](https://jrsoftware.org/isinfo.php).

On macOS and Linux:

```bash
scripts/fetch-vendor.sh
cargo test --workspace
cargo clippy --all-targets -- -D warnings
scripts/package-release.sh --lgpl --desktop
```

`fetch-vendor.sh` downloads PDFium and builds libheif, libde265 and x265 from pinned, hash-checked sources into `vendor/` (the conda-forge macOS builds link a Homebrew library, so they can't be shipped). `package-release.sh` builds `dist/tumble-<version>-linux-x64-lgpl.tar.gz` or `dist/tumble-<version>-macos-<arch>-lgpl.zip`, then unpacks it, converts real files with it and runs its `install.sh` and `uninstall.sh` against a temporary home folder.

Tests that need FFmpeg, LibreOffice or the vendor libraries skip with a message when those are missing.

## Continuous integration

`.github/workflows/ci.yml` runs formatting, clippy and every test on Windows, macOS and Linux for each push, with FFmpeg, LibreOffice and the vendor libraries installed, and checks the menu files a real `tumble menu install` writes. `.github/workflows/release.yml` builds the macOS (Apple silicon and Intel) and Linux downloads when a `v*` tag is pushed and attaches them to that tag's release, creating a draft release if there is none. The Windows downloads are still built locally with `package-release.ps1`.

## Desktop window

The desktop window lives in `apps/desktop` and has its own Cargo workspace.

```bash
cd apps/desktop && bun install && bun run tauri dev
cd apps/desktop/src-tauri && cargo test && cargo clippy --all-targets -- -D warnings
```

## Windows 11 menu

The Windows 11 top-level menu lives in `apps/explorer`. It has its own workspace because the DLL runs inside explorer.exe and must never abort on a panic.

```bash
cd apps/explorer && cargo test && cargo clippy --all-targets -- -D warnings
./apps/explorer/scripts/explorer-menu.ps1 -Action build
./apps/explorer/scripts/explorer-menu.ps1 -Action install    # elevated
```

For releases, `package-release.ps1` runs `explorer-menu.ps1 -Action build -Release`, which signs the package with a new certificate and deletes its private key right after, so trusting that certificate can't let anything else in. The scripts users run to add or remove the main menu are in `packaging/main-menu`.

## How conversions work

Conversions go through one registry of engines. Each engine declares the direct steps it can do, and Tumble finds the shortest route (at most three steps, never through a lossy format when a lossless one works). For example, a DOCX becomes PNGs by going DOCX → PDF (LibreOffice) → PNG (PDFium).

| Part | Technology |
|---|---|
| Images | [`image`](https://crates.io/crates/image), libwebp, [rav1d](https://github.com/memorysafety/rav1d) and rav1e (AVIF), [resvg](https://github.com/linebender/resvg) (SVG), [`exr`](https://crates.io/crates/exr) |
| HEIC | [libheif](https://github.com/strukturag/libheif) with libde265 and x265, loaded at runtime |
| PDF | [PDFium](https://pdfium.googlesource.com/pdfium/), loaded at runtime |
| Video and audio | the installed FFmpeg, run as a subprocess |
| Documents | the installed LibreOffice, run headless with a throwaway profile |
| CLI | [clap](https://crates.io/crates/clap), [rayon](https://crates.io/crates/rayon) |
| Windows integration | [windows-rs](https://github.com/microsoft/windows-rs): registry menu, shell progress dialog, toast notifications, named-pipe batching, `IExplorerCommand` and sparse MSIX for the Windows 11 menu |
| macOS integration | an Automator Quick Action that runs `tumble pick` (a `choose from list` dialog, then the same menu mode as Windows), `osascript` notifications, Unix-socket batching |
| Linux integration | Dolphin service menus, Nemo actions, Thunar custom actions and a nautilus-python extension (or Nautilus scripts), `zenity` or `kdialog` progress, `notify-send` notifications, Unix-socket batching |

## HEIC writing

Release downloads use the LGPL build of libheif, which reads HEIC but can't write it. Writing HEIC needs x265, which is GPL-2.0 and covered by HEVC patents in some countries; to build with it, run `./scripts/fetch-vendor.ps1` without `-Lgpl`, or `scripts/fetch-vendor.sh` without `--lgpl`.

## Third-party licenses

Windows release zips include PDFium (BSD-3-Clause), libheif and libde265 (LGPL-3.0), aom (BSD-2-Clause) and the Microsoft Visual C++ runtime; macOS and Linux downloads include PDFium, libheif and libde265. Each is under its own license in the download's `licences` folder.
