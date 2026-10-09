<p align="center"><img src="assets/tumble.png" width="96" alt="Tumble icon"></p>

<h1 align="center">Tumble</h1>

<p align="center">A free, local file converter for Windows, macOS and Linux.</p>

---

Tumble adds a convert option to the right-click menu (File Explorer on Windows, Finder on macOS, and Dolphin, Nemo, Thunar and GNOME Files on Linux); pick a format and the converted file is saved next to the original.

Everything happens on your computer, so there is no account, upload, paywall, license key, telemetry, update check or network access, and tests check that no network library is compiled in.

## Features

- Right-click menu: it only lists formats the selected files can be converted to. On Windows 11 it's under "Show more options", or in the main menu after [one optional step](#windows-11-main-menu); on macOS it's under Quick Actions > Convert with Tumble, which asks for the format in a short list.
- Batches: selecting several files starts one job, and the files convert in parallel with one progress dialog (you can cancel it) and one notification. On macOS the progress and its stop button are in the menu bar, where Finder shows running Quick Actions.
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

Images, HEIC and PDF work out of the box. Video and audio need [FFmpeg](https://ffmpeg.org), and documents (Markdown included) need [LibreOffice](https://www.libreoffice.org) 25.8 or newer. Both are free, and Tumble hides those formats if they aren't installed; the Windows installer can install them for you. Release downloads can read HEIC files but not write them (see [Development](docs/development.md#heic-writing) to build with HEIC writing). To install them yourself on Windows:

```bash
winget install Gyan.FFmpeg.Essentials
```
```bash
winget install TheDocumentFoundation.LibreOffice
```

On macOS, with [Homebrew](https://brew.sh):

```bash
brew install ffmpeg
```
```bash
brew install --cask libreoffice
```

On Linux, install `ffmpeg` with your package manager, and LibreOffice 25.8 or newer from your package manager or [libreoffice.org](https://www.libreoffice.org/download/) (some distributions still ship an older version, which converts documents but not Markdown).

## Install

### Windows

Download `tumble-<version>-setup.exe` from [Releases](../../releases) and run it. It installs Tumble for your user only (no admin rights needed), adds the right-click menu and puts `tumble` on your PATH. It also offers to install FFmpeg and LibreOffice; both are unchecked unless you tick them.

The installer isn't signed yet, so Windows may show "Windows protected your PC"; click "More info", then "Run anyway".

If you install FFmpeg or LibreOffice later, run `tumble menu install` again to add their formats.

#### Windows 11 main menu

On Windows 11, Tumble is under "Show more options" in the right-click menu. To put it in the main menu instead:

1. Open the `main-menu` folder inside Tumble's folder. For the installer, paste `%LOCALAPPDATA%\Programs\Tumble\main-menu` into File Explorer's address bar.
2. Double-click `Add to main menu.cmd`.
3. Click Yes when Windows asks for admin rights. This trusts Tumble's certificate, which Windows needs before it shows an app in the main menu. The certificate's private key was deleted after signing, so it can't be used to sign anything else.
4. If Tumble doesn't show up yet, restart File Explorer (Task Manager > Windows Explorer > Restart).

To undo it, double-click `Remove from main menu.cmd` in the same folder. Uninstalling Tumble also removes it.

#### Uninstall

Uninstall Tumble from Settings > Apps > Installed apps, or from "Uninstall Tumble" in the Start menu. This removes the program, the right-click menu, your settings and presets, logs and the PATH entry, so nothing is left behind. If the installer installed FFmpeg or LibreOffice for you, it asks whether to remove them too (No is the default); programs you installed yourself are never touched.

#### Portable zip

If you'd rather not run an installer, download `tumble-<version>-win-x64-lgpl.zip` instead, unzip it somewhere permanent and run `.\tumble.exe menu install` in that folder. To remove it, double-click `Remove from main menu.cmd` if you added the main menu, run `.\tumble.exe menu uninstall` and delete the folder, along with `%APPDATA%\Tumble` and `%LOCALAPPDATA%\Tumble` if they exist.

### macOS

Download `tumble-<version>-macos-arm64-lgpl.zip` (Apple silicon) or `tumble-<version>-macos-x64-lgpl.zip` (Intel) from [Releases](../../releases), unzip it, open a terminal in the unzipped folder and run:

```bash
sh install.sh
```

It installs Tumble for your user only (no admin rights needed): the desktop window goes in `~/Applications/Tumble.app`, `tumble` goes on your PATH, and the right-click menu is added. To convert, select files in Finder, right-click and choose Quick Actions > Convert with Tumble, then pick a format. When a job ends a notification says how it went, but unlike on Windows and Linux, clicking it doesn't open the folder, since macOS doesn't allow that for this kind of notification.

Tumble isn't signed by Apple yet. `install.sh` takes care of that, but if you open `Tumble.app` straight from the download, macOS may say it can't check it for malware; right-click it and choose Open to allow it once.

To uninstall, run `sh ~/Library/Application\ Support/Tumble/uninstall.sh`. It removes the program, the right-click menu, your settings and presets, and logs.

### Linux

Download `tumble-<version>-linux-x64-lgpl.tar.gz` from [Releases](../../releases), unpack it, open a terminal in the unpacked folder and run:

```bash
sh install.sh
```

It installs Tumble for your user only (no sudo needed) into `~/.local/share/tumble`, puts `tumble` on your PATH, adds the desktop window to your apps and adds the right-click menu to the file managers you have installed:

| File manager | Menu |
|---|---|
| Dolphin (KDE) | "Convert to" submenu |
| GNOME Files | "Convert to" submenu, which needs nautilus-python (`python3-nautilus` or `nautilus-python`); without it the entries are under Scripts > Tumble |
| Nemo (Cinnamon) | "Convert to PNG" style entries |
| Thunar (Xfce) | "Convert to PNG" style entries |

Restart the file manager if the menu doesn't show up straight away (for GNOME Files, run `nautilus -q`). The progress dialog uses `zenity`, or `kdialog` on KDE, and the notification uses `notify-send`; most desktops come with them. The desktop window needs WebKitGTK 4.1 (`libwebkit2gtk-4.1-0`), which GNOME and KDE desktops usually already have.

To uninstall, run `sh ~/.local/share/tumble/app/uninstall.sh`. It removes the program, the right-click menu, your settings and presets, and logs.

## CLI

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

**Settings** (optional): `config.toml` sets a default output folder, quality, parallel jobs and tool paths, and your own presets go in `presets.toml` in the same folder. Failures from the right-click menu are logged.

| | Settings and presets | Logs |
|---|---|---|
| Windows | `%APPDATA%\Tumble` | `%LOCALAPPDATA%\Tumble\logs` |
| macOS | `~/Library/Application Support/Tumble` | `~/Library/Logs/Tumble` |
| Linux | `~/.config/tumble` | `~/.local/share/tumble/logs` |

## Tech stack

| Part | Technology |
|---|---|
| Language | Rust |
| Images, HEIC and PDF | [`image`](https://crates.io/crates/image), [libheif](https://github.com/strukturag/libheif), [PDFium](https://pdfium.googlesource.com/pdfium/) |
| Video, audio and documents | your installed [FFmpeg](https://ffmpeg.org) and [LibreOffice](https://www.libreoffice.org) |
| Shell integration | [windows-rs](https://github.com/microsoft/windows-rs) on Windows; an Automator Quick Action on macOS; Dolphin service menus, Nemo actions, Thunar custom actions and [nautilus-python](https://gitlab.gnome.org/GNOME/nautilus-python) on Linux |
| Desktop window | [Tauri 2](https://tauri.app), TypeScript, [Bun](https://bun.sh) |

### Structure

| Path | What |
|---|---|
| `crates/tumble-core` | formats, engine trait, routing, jobs, config, presets; no native or Windows dependencies |
| `crates/tumble-engines` | the image, libheif, PDFium, FFmpeg and LibreOffice engines |
| `crates/tumble-shell` | right-click menu, batching, progress dialog and notifications for Windows, macOS and Linux |
| `crates/tumble-cli` | `tumble`, and on Windows `tumblew.exe`, the tiny windowless launcher the menu runs |
| `apps/desktop` | the optional desktop window (Tauri) |
| `apps/explorer` | the Windows 11 top-level menu (COM DLL + sparse MSIX) |
| `scripts/` | `fetch-vendor.ps1` and `fetch-vendor.sh` (pinned, hash-checked libraries), `package-release.ps1` and `package-release.sh` (release downloads) |
| `packaging/` | the Windows installer script, and `install.sh` and `uninstall.sh` for macOS and Linux |
| `.github/workflows/` | tests on Windows, macOS and Linux for every push, and the macOS and Linux release builds |
| `docs/verification/` | records of checks done by hand on real hardware |

## Development

On Windows it needs Windows 11 x64, Rust (MSVC), the Visual Studio C++ build tools, and Bun for the desktop window.

```bash
./scripts/fetch-vendor.ps1
cargo test --workspace
./scripts/package-release.ps1 -Lgpl -Desktop -Installer
```

On macOS and Linux it needs Rust, a C++ compiler and CMake (to build libheif), and Bun for the desktop window.

```bash
scripts/fetch-vendor.sh
cargo test --workspace
scripts/package-release.sh --lgpl --desktop
```

See [docs/development.md](docs/development.md) for building the desktop window and the Windows 11 menu.

## License

Tumble's code is under the [MIT license](LICENSE).

Release downloads include third-party libraries under their own licenses (see the `licences` folder in them).
