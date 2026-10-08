<p align="center"><img src="assets/tumble.png" width="96" alt="Tumble icon"></p>

<h1 align="center">Tumble</h1>

<p align="center">A free, local file converter for Windows.<br>Right-click a file and pick a format to convert it to.</p>

---

Tumble adds a convert option to the File Explorer right-click menu; pick a format and the converted file is saved next to the original. If you select several files, they convert as one batch with one progress dialog and one notification at the end.

Everything happens on your PC, so there is no account, upload, paywall, license key, telemetry, update check or network access, and tests check that no network library is compiled in.

There's also a command-line tool (`tumble`) for scripts and batch jobs, and an optional desktop window you can drag files onto.

## Features

- Right-click menu: shows in the Windows 11 main menu and under "Show more options", and it only lists formats the selected file can be converted to.
- Batches: selecting several files starts one job, and the files convert in parallel with one progress dialog (you can cancel it) and one notification.
- No overwriting: if the output file already exists, Tumble saves it as `photo (1).jpg` instead, and the original file is never changed.
- Command line: convert folders and subfolders, run parallel jobs, set quality, resize, use presets and get JSON progress output.
- Desktop window (optional): drop files or folders in, pick a format, and see the queue.
- Size: the menu and command line take up about 10 MB, and nothing runs in the background.

## Formats

| Kind | Formats | Converts to |
|---|---|---|
| Images | JPEG, PNG, WebP, HEIC, AVIF, GIF, TIFF, BMP, ICO, TGA, PPM, QOI, OpenEXR, SVG (read only) | every other image format. GIF also to video. |
| Video | MP4, MOV, WebM, MKV, AVI | other video, audio (the soundtrack), a still image (one frame), or an animated GIF |
| Audio | MP3, WAV, FLAC, AAC, M4A, OGG, Opus | every other audio format |
| PDF | PDF | images, one per page (`report-p001.png`, `report-p002.png`, …) |
| Documents | DOCX, DOC, ODT, RTF, TXT, HTML, Markdown | PDF, each other, and images. Markdown is read properly (headings, bold, lists, tables, links) and can be written from any of the others. |
| Slides | PPTX, PPT, ODP | PDF, each other, and images |
| Spreadsheets | XLSX, XLS, ODS, CSV | PDF, each other, and images |

Images, HEIC and PDF work out of the box. Video and audio need [FFmpeg](https://ffmpeg.org), and documents (Markdown included) need [LibreOffice](https://www.libreoffice.org) 25.8 or newer. Both are free; Tumble uses them if they're installed and hides those formats if they aren't. To install them:

```bash
winget install Gyan.FFmpeg
```
```bash
winget install TheDocumentFoundation.LibreOffice
```

## Install

1. Download `tumble-<version>-win-x64.zip` from [Releases](../../releases) and unzip it somewhere permanent, for example `C:\Users\<you>\Apps\Tumble`. Keep all the files together.
2. In that folder, run:
   ```bash
   .\tumble.exe menu install
   ```
   This adds the right-click menu for your user only; no admin rights needed. On Windows 11 it's under "Show more options" until you add the top-level menu below.
3. Optional, Windows 11 top-level menu: see [apps/explorer](apps/explorer/scripts/explorer-menu.ps1). It needs a one-time certificate step in an elevated terminal.

If you install FFmpeg or LibreOffice later, run `.\tumble.exe menu install` again to add their formats.

To remove Tumble: `.\tumble.exe menu uninstall`, then delete the folder. Uninstalling removes only the registry keys Tumble added.

## Command line

```bash
tumble photo.heic --to jpg
tumble "Holiday pics" -r --to webp -q 80 --resize 2048
tumble clip.mov --preset small-video
tumble report.docx --to png -o out
tumble notes.md --to pdf
```

| Command | What it does |
|---|---|
| `tumble <files or folders> --to <format>` | convert |
| `tumble formats` | every format and what it converts to |
| `tumble targets <file>` | what this file can become |
| `tumble engines` | which engines are available, and how to get the missing ones |
| `tumble presets` | built-in presets (`web`, `small-video`, `voice`) and your own |
| `tumble menu install \| uninstall \| status` | the right-click menu |

| Option | Meaning |
|---|---|
| `--to <fmt>` | target format, by name or extension (`webp`, `.webp`) |
| `-o, --out <dir>` | output folder (default: next to each input) |
| `-r` | include subfolders |
| `-j <n>` | parallel jobs (default: half your CPU cores) |
| `-q, --quality <0-100>` | quality for lossy formats (WebP and HEIC: 100 means lossless) |
| `--resize <WxH or N>` | shrink to fit, keeping the aspect ratio |
| `--at <time>` | which frame to take from a video (default: 10% in) |
| `--preset <name>` | apply a preset |
| `--overwrite` | replace existing outputs instead of numbering them |
| `--json` | one JSON progress event per line |

Exit codes: `0` all converted, `1` some failed, `2` bad arguments, `3` no way to convert (or a missing engine).

**Settings** (optional): `%APPDATA%\Tumble\config.toml` sets a default output folder, quality, parallel jobs and tool paths. Your own presets go in `%APPDATA%\Tumble\presets.toml`. Failures from the right-click menu are logged in `%LOCALAPPDATA%\Tumble\logs\`.

## How it's built

| Part | Technology |
|---|---|
| Language | Rust (stable, edition 2024, MSVC) |
| Images | [`image`](https://crates.io/crates/image), libwebp, [rav1d](https://github.com/memorysafety/rav1d) and rav1e (AVIF), [resvg](https://github.com/linebender/resvg) (SVG), [`exr`](https://crates.io/crates/exr) |
| HEIC | [libheif](https://github.com/strukturag/libheif) with libde265 and x265, loaded at runtime |
| PDF | [PDFium](https://pdfium.googlesource.com/pdfium/), loaded at runtime |
| Video and audio | your installed FFmpeg, run as a subprocess |
| Documents | your installed LibreOffice, run headless with a throwaway profile |
| Command line | [clap](https://crates.io/crates/clap), [rayon](https://crates.io/crates/rayon) |
| Windows integration | [windows-rs](https://github.com/microsoft/windows-rs): registry menu, shell progress dialog, toast notifications, named-pipe batching, `IExplorerCommand` + sparse MSIX for the Windows 11 menu |
| Desktop window | [Tauri 2](https://tauri.app), TypeScript, Vite, [Bun](https://bun.sh) |

Conversions are routed through one registry of engines: each engine declares the direct steps it can do, and Tumble finds the shortest route (at most three steps, never through a lossy format when a lossless one works). A DOCX becomes PNGs by going DOCX → PDF (LibreOffice) → PNG (PDFium).

### Repository layout

| Path | What |
|---|---|
| `crates/tumble-core` | formats, engine trait, routing, jobs, config, presets; no native or Windows dependencies |
| `crates/tumble-engines` | the image, libheif, PDFium, FFmpeg and LibreOffice engines |
| `crates/tumble-shell` | Windows only: right-click menu, batching, progress dialog, toasts |
| `crates/tumble-cli` | `tumble.exe`, and `tumblew.exe`, the tiny windowless launcher the menu runs |
| `apps/desktop` | the optional desktop window (Tauri) |
| `apps/explorer` | the Windows 11 top-level menu (COM DLL + sparse MSIX) |
| `scripts/` | `fetch-vendor.ps1` (pinned, hash-checked DLLs), `package-release.ps1` (release zip) |
| `docs/verification/` | records of checks done by hand on real hardware |

## Building from source

Needs Windows 11 x64, Rust (MSVC), the Visual Studio C++ build tools, and Bun for the desktop window.

```bash
./scripts/fetch-vendor.ps1
cargo test --workspace
cargo clippy --all-targets -- -D warnings
./scripts/package-release.ps1 -Desktop
```

Tests that need FFmpeg, LibreOffice or the vendor DLLs skip with a message when those are missing.

Desktop window (`apps/desktop`, its own Cargo workspace):

```bash
cd apps/desktop && bun install && bun run tauri dev
cd apps/desktop/src-tauri && cargo test && cargo clippy --all-targets -- -D warnings
```

Windows 11 top-level menu (`apps/explorer`, its own workspace because the DLL runs inside explorer.exe and must never abort on a panic):

```bash
cd apps/explorer && cargo test && cargo clippy --all-targets -- -D warnings
./apps/explorer/scripts/explorer-menu.ps1 -Action build
./apps/explorer/scripts/explorer-menu.ps1 -Action install    # elevated
```

## License

Tumble's code is under the [MIT license](LICENSE).

Release zips also contain third-party libraries, each under its own license (see the `licences` folder in the zip): PDFium (BSD-3-Clause), libheif and libde265 (LGPL-3.0), aom (BSD-2-Clause) and the Microsoft Visual C++ runtime. Public releases use the LGPL build of libheif, which reads HEIC but cannot write it. Writing HEIC needs x265, which is GPL-2.0 and covered by HEVC patents in some countries; build it yourself with `./scripts/fetch-vendor.ps1` (without `-Lgpl`) if you want it.
