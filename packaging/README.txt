Tumble {VERSION}
================

Tumble converts files on this PC: right-click a file in File Explorer, pick
a format, and the converted file appears next to the original. The same
engine is available from the command line as tumble.exe.

Nothing leaves your PC. There is no account, licence key, telemetry, update
check or network access of any kind.


Install
-------

1. Unzip this folder somewhere permanent, for example
   C:\Users\<you>\Apps\Tumble. Keep every file together: the DLLs must sit
   next to tumble.exe.

2. Open a terminal in that folder and run:

       .\tumble.exe menu install

   This adds "Convert to" to the right-click menu for your user only (no
   admin rights). On Windows 11 it is under "Show more options" (Shift+F10).

If you move the folder later, run `menu install` again from the new place.


Video, audio and documents
--------------------------

Images, HEIC and PDF work out of the box. Two free programs add more:

    Video and audio (MP4, MKV, MP3, FLAC...):   winget install Gyan.FFmpeg
    Word, Excel, PowerPoint and friends:        winget install TheDocumentFoundation.LibreOffice

Install either one, then run `.\tumble.exe menu install` again so the menu
picks up the new formats. `.\tumble.exe engines` shows what is available.


Desktop window
--------------

If tumble-desktop.exe is in this folder, it opens a window: drop files or
folders on it, pick a format, and press Convert. It uses the same engines as
the menu and the command line.


Command line
------------

    tumble photo.heic --to jpg
    tumble "Holiday pics" -r --to webp -q 80 --resize 2048
    tumble clip.mov --preset small-video
    tumble formats              every format and what it converts to
    tumble targets photo.heic   what this file can become
    tumble presets              built-in and your own presets
    tumble --help

Outputs never overwrite anything: a second run writes "photo (1).jpg".

Settings (optional): %APPDATA%\Tumble\config.toml
Your own presets:    %APPDATA%\Tumble\presets.toml
Failure log:         %LOCALAPPDATA%\Tumble\logs\


Uninstall
---------

    .\tumble.exe menu uninstall

then delete this folder, and %LOCALAPPDATA%\Tumble if you like.


Licences
--------

See the licences folder. HEIC writing uses x265 (GPL-2.0) and HEVC, which
may be covered by patents where you live.
