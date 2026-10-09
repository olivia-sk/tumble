Tumble {VERSION}
================

Tumble adds a convert option to the right-click menu of your file manager;
pick a format and the converted file is saved next to the original. There's
also a command-line tool (tumble) and an optional desktop window.

Everything happens on this computer, so there is no account, license key,
telemetry, update check or network access.


Install
-------

Open a terminal in this folder and run:

    sh install.sh

This installs Tumble for your user only (no sudo needed), puts `tumble` on
your PATH, adds the right-click menu and, if it's included, adds the
desktop window to your apps. The last line it prints says how to uninstall.

On macOS the right-click menu is under Quick Actions: select files in
Finder, right-click, then Quick Actions > Convert with Tumble, and pick a
format from the list. Tumble isn't signed by Apple yet; install.sh lets it
run, but if you open Tumble.app straight from this folder instead, macOS may
say it can't check it for malware. Right-click it and choose Open to allow
it once.

On Linux the menu is added to the file managers that are installed:
Dolphin (KDE) and GNOME Files get a "Convert to" submenu, Nemo (Cinnamon)
and Thunar (Xfce) get "Convert to PNG" style entries. GNOME Files needs
nautilus-python (python3-nautilus or nautilus-python) for the submenu;
without it, the entries are under Scripts > Tumble. Restart the file
manager if the menu doesn't show up straight away.


Without installing
------------------

You can also run ./tumble from this folder (on macOS with the desktop
window, Tumble.app/Contents/MacOS/tumble). Keep every file together, since
the libraries need to be next to it. `./tumble menu install` adds the menu
for that copy; if you move the folder, run it again.


Video, audio and documents
--------------------------

Images, HEIC and PDF work out of the box, and two free programs add more:

    Video and audio:   brew install ffmpeg (macOS), or your package manager
    Documents:         brew install --cask libreoffice (macOS), or
                       LibreOffice 25.8 or newer from your package manager
                       or libreoffice.org

After installing either one, run `tumble menu install` again so the menu
shows the new formats. `tumble engines` shows what is available.


CLI
---

    tumble photo.heic --to jpg
    tumble "Holiday pics" -r --to webp -q 80 --resize 2048
    tumble clip.mov --preset small-video
    tumble formats              every format and what it converts to
    tumble targets photo.heic   what this file can become
    tumble presets              built-in and your own presets
    tumble --help

If an output file already exists, Tumble saves the new one as
"photo (1).jpg" instead of overwriting it.

Settings (optional):   ~/.config/tumble/config.toml (Linux)
                       ~/Library/Application Support/Tumble/config.toml (macOS)
Your own presets:      presets.toml in the same folder
Failure log:           ~/.local/share/tumble/logs/ (Linux)
                       ~/Library/Logs/Tumble/ (macOS)


Uninstall
---------

Run the uninstall.sh that install.sh printed at the end. It removes the
program, the right-click menu, settings, presets and logs. FFmpeg and
LibreOffice are separate programs and stay installed.


License
-------

Tumble's code is under the MIT license. The bundled libraries are under
their own licenses; see the licences folder.
