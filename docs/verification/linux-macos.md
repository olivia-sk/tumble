# macOS and Linux: checks still to do by hand

The macOS and Linux versions were written on Windows. CI builds them and runs
every test on real macOS and Linux machines, including installing the menu
into a temporary home folder, converting with the packaged binaries and
running `install.sh` and `uninstall.sh`. What CI can't do is click: these
checks need a person at a real desktop, and nobody has done them yet.

Record the date, machine and results here once they are done, like the
Windows records in this folder.

## macOS

1. Unzip the release, run `sh install.sh` in Terminal.
2. Finder: select one JPEG, right-click > Quick Actions > Convert with
   Tumble. The list should offer PNG, WebP, AVIF, GIF, TIFF and ICO, and
   converting should write the file next to the original.
3. Select several files of different image types and do the same: one list,
   one notification at the end ("3 files converted").
4. Convert a long video: the gear in the menu bar should show while it
   runs, and its stop button should cancel the job ("Cancelled after ...").
5. Check the format list comes to the front (it is shown by `osascript`; if
   it opens behind Finder, `tumble pick` needs a different way to show it).
6. Open Tumble.app from ~/Applications: drop files, convert, reveal output.
7. Run the uninstall.sh that install.sh printed: the Quick Action, the app
   and `~/Library/Application Support/Tumble` should be gone.

## Linux

For each file manager available (Dolphin, GNOME Files with and without
nautilus-python, Nemo, Thunar):

1. Run `sh install.sh` from the unpacked release.
2. Right-click a JPEG: "Convert to" (or "Convert to PNG" entries) should
   list only formats a JPEG can become; a `.txt` file should not also show
   the CSV or Markdown entries in Dolphin.
3. Select several files and convert: one job, one notification; clicking
   the notification should open the folder.
4. Convert a long video: the zenity (or kdialog on KDE) progress dialog
   should appear after about a second, and Cancel should stop the job.
5. Thunar: existing custom actions (such as "Open Terminal Here") must
   still be there after install and uninstall.
6. Open Tumble from the app launcher.
7. Run the uninstall.sh that install.sh printed.
