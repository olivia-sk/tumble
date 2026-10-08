Tumble {VERSION}
================

Tumble adds a convert option to the File Explorer right-click menu; pick a
format and the converted file is saved next to the original. There's also a
command-line tool (tumble.exe) and an optional desktop window.

Everything happens on this PC, so there is no account, license key,
telemetry, update check or network access.


Install
-------

1. Unzip this folder somewhere permanent, for example
   C:\Users\<you>\Apps\Tumble. Keep every file together, since the DLLs
   need to be next to tumble.exe.

2. Open a terminal in that folder and run:

       .\tumble.exe menu install

   This adds the right-click menu for your user only (no admin rights
   needed). On Windows 11 it's under "Show more options" (Shift+F10).

If you move the folder later, run `menu install` again from the new place.


Video, audio and documents
--------------------------

Images, HEIC and PDF work out of the box, and two free programs add more:

    Video and audio (MP4, MKV, MP3, FLAC...):   winget install Gyan.FFmpeg
    Word, Excel and PowerPoint files:           winget install TheDocumentFoundation.LibreOffice

After installing either one, run `.\tumble.exe menu install` again so the
menu shows the new formats. `.\tumble.exe engines` shows what is available.


Desktop window
--------------

tumble-desktop.exe opens a window where you can drop files or folders, pick
a format and press Convert. It uses the same engines as the menu and the CLI.


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

Settings (optional): %APPDATA%\Tumble\config.toml
Your own presets:    %APPDATA%\Tumble\presets.toml
Failure log:         %LOCALAPPDATA%\Tumble\logs\


Uninstall
---------

    .\tumble.exe menu uninstall

Then delete this folder, and %LOCALAPPDATA%\Tumble if you want to remove
the logs too.


License
-------

Tumble's code is under the MIT license. The bundled libraries are under
their own licenses; see the licences folder.
