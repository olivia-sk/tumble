<p align="center"><img src="assets/tumble.png" width="96" alt="Tumble icon"></p>

<h1 align="center">Tumble</h1>

<p align="center">A free, local file converter for Windows, macOS and Linux.</p>

---

Tumble adds a convert option to the right-click menu (File Explorer on Windows, Finder on macOS, and Dolphin, Nemo, Thunar and GNOME Files on Linux); pick a format and the converted file is saved next to the original.

Everything happens on your computer, so there is no account, upload, paywall, license key, telemetry, update check or network access, and tests check that no network library is compiled in.

## Features

- Right-click menu: it only lists formats the selected files can be converted to. On Windows 11 it's under "Show more options", or in the main menu after [one optional step](#windows-11-main-menu); on macOS it's under Quick Actions > Convert with Tumble, which asks for the format in a short list; on Linux it's in Dolphin, GNOME Files, Nemo and Thunar.
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

Download the file for your system from [Releases](../../releases):

| System | Installer | Without an installer | Needs |
|---|---|---|---|
| Windows | `tumble-<version>-setup.exe` | `tumble-<version>-win-x64-lgpl.zip` | Windows 10 (1809) or 11, x64 |
| macOS, Apple silicon (M1 and later) | `tumble-<version>-macos-arm64.dmg` | `tumble-<version>-macos-arm64-lgpl.zip` | macOS 11 or newer |
| macOS, Intel | `tumble-<version>-macos-x64.dmg` | `tumble-<version>-macos-x64-lgpl.zip` | macOS 11 or newer |
| Linux | `tumble-<version>-linux-x64.deb` (Ubuntu, Debian, Linux Mint) | `tumble-<version>-linux-x64-lgpl.tar.gz` (any distribution) | x64, Ubuntu 22.04 or Debian 12 or newer (glibc 2.35) |

To check which Mac you have, open the Apple menu > About This Mac: "Chip: Apple M…" means Apple silicon, "Processor: Intel" means Intel.

### Windows

Download `tumble-<version>-setup.exe` and run it. It installs Tumble for your user only (no admin rights needed), adds the right-click menu and puts `tumble` on your PATH. It also offers to install FFmpeg and LibreOffice; both are unchecked unless you tick them.

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

1. Download the `.dmg` for your Mac (`arm64` for Apple silicon, `x64` for Intel) and open it.
2. Drag Tumble onto the Applications folder in the window that opens.
3. Open Applications, right-click Tumble and choose Open, then click Open again. Tumble isn't signed by Apple yet, so the first time macOS says it can't check it for malware; after this once, it opens normally. On recent macOS versions, if there's no Open button, go to System Settings > Privacy & Security and click "Open Anyway" next to the message about Tumble.
4. When the Tumble window opens, the right-click menu is added. You can close the window; the menu stays.

To convert, select one or more files in Finder, right-click and choose Quick Actions > Convert with Tumble, then pick a format from the list. The list only shows formats every selected file can become. While a job runs, a gear in the menu bar shows its progress, and its stop button cancels it. When it ends, a notification says how it went; unlike on Windows and Linux, clicking it doesn't open the folder, since macOS doesn't allow that for this kind of notification.

If Convert with Tumble isn't under Quick Actions, choose Quick Actions > Customize (or go to System Settings > Keyboard > Keyboard Shortcuts > Services > Files and Folders) and tick it.

The `.dmg` doesn't put `tumble` on your PATH. To use the command line, either run it by its full path, `/Applications/Tumble.app/Contents/MacOS/tumble`, or link it into a folder on your PATH:

```bash
sudo ln -sf /Applications/Tumble.app/Contents/MacOS/tumble /usr/local/bin/tumble
```

If you install FFmpeg or LibreOffice later, run `tumble menu install` again (or `/Applications/Tumble.app/Contents/MacOS/tumble menu install`) to add their formats.

#### Uninstall

Run this in Terminal to remove the right-click menu, then drag Tumble from Applications to the Trash:

```bash
/Applications/Tumble.app/Contents/MacOS/tumble menu uninstall
```

To also remove your settings, presets and logs, delete `~/Library/Application Support/Tumble` and `~/Library/Logs/Tumble`, and `/usr/local/bin/tumble` if you linked it.

#### Without the .dmg

If you'd rather use a terminal, download `tumble-<version>-macos-<arch>-lgpl.zip`, unzip it, open a terminal in the unzipped folder and run:

```bash
sh install.sh
```

It installs Tumble for your user only (no admin rights needed) in `~/Applications/Tumble.app`, puts `tumble` on your PATH (in `~/.local/bin`) and adds the right-click menu. To remove all of it, including your settings, presets and logs, run:

```bash
sh ~/Library/Application\ Support/Tumble/uninstall.sh
```

### Linux

1. Download `tumble-<version>-linux-x64.deb`.
2. Double-click it to open it in your software installer and click Install, or install it in a terminal:

    ```bash
    sudo apt install ./tumble-<version>-linux-x64.deb
    ```

3. Open Tumble once from your apps menu. This adds the right-click menu; you can close the window afterwards.

`tumble` is on your PATH. The right-click menu is added to the file managers you have installed:

| File manager | Desktop | Menu |
|---|---|---|
| Dolphin | KDE | "Convert to" submenu |
| GNOME Files (Nautilus) | GNOME, Ubuntu | "Convert to" submenu, which needs nautilus-python (`sudo apt install python3-nautilus`); without it the entries are under Scripts > Tumble |
| Nemo | Cinnamon, Linux Mint | "Convert to PNG" style entries |
| Thunar | Xfce | "Convert to PNG" style entries |

Restart the file manager if the menu doesn't show up straight away (for GNOME Files, run `nautilus -q`). If you install another file manager or nautilus-python later, run `tumble menu install` again.

To convert, select one or more files, right-click and pick a format. The menu only lists formats the selected files can be converted to. A progress dialog appears for jobs that take longer than a second (Cancel stops it), and a notification says how it went; clicking it opens the folder. The progress dialog uses `zenity`, or `kdialog` on KDE, and the notification uses `notify-send` (`libnotify-bin`); most desktops come with them.

If you install FFmpeg or LibreOffice later, run `tumble menu install` again to add their formats.

#### Uninstall

```bash
tumble menu uninstall
sudo apt remove tumble
```

The first command removes the right-click menu and the second removes the program. To also remove your settings, presets and logs, delete `~/.config/tumble` and `~/.local/share/tumble`.

#### Other distributions

On Fedora, Arch, openSUSE and others, download `tumble-<version>-linux-x64-lgpl.tar.gz`, unpack it, open a terminal in the unpacked folder and run:

```bash
sh install.sh
```

It installs Tumble for your user only (no sudo needed) into `~/.local/share/tumble`, puts `tumble` on your PATH (in `~/.local/bin`), adds the desktop window to your apps and adds the right-click menu. The desktop window needs WebKitGTK 4.1 (`webkit2gtk4.1` on Fedora, `webkit2gtk-4.1` on Arch), which GNOME and KDE desktops usually already have; the right-click menu and the command line work without it. To remove all of it, including your settings, presets and logs, run:

```bash
sh ~/.local/share/tumble/app/uninstall.sh
```

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
