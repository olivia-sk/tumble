# Phase 5: right-click menu on a real Explorer selection

PRD section 11 asks for the menu to be checked on real Explorer selections
of 1, 15, 16 and 100 files, with the results recorded here.

- Date: 2026-10-07
- Machine: Olivia's PC, Windows 11 Home (build 26300), x64
- Build: release, commit `81dd1cb`, menu pointing at `target\release\tumblew.exe`
- Installed with `tumble menu install`: 50 extensions, 40 formats, under
  `HKCU\Software\Classes` only
- Test files: generated 800x600 JPEGs in `tumble-menu-test\`, plus a 30 s
  1080p MP4

| Selection | Action | Result |
|---|---|---|
| 1 file | Convert to > PNG | Worked (reported by Olivia) |
| 15 files | Convert to > WebP | Worked. All 15 `.webp` files were written within the same second, consistent with one batch |
| 16 files | Convert to > WebP | Worked (reported by Olivia) |
| 100 files | Convert to > WebP | Worked (reported by Olivia) |
| 30 s video | Convert to > WebM, then Cancel | Worked (reported by Olivia); no `movie.webm` left behind |

Olivia deleted the outputs of the 1, 16 and 100 file runs before they were
inspected, so those rows rest on her report rather than on the files.

Not recorded separately: how many toasts each run showed, and whether the
progress dialog appeared for the 100-file run.

The menu entries appear under "Show more options" (Shift+F10) on Windows 11,
as expected for classic registry verbs; the top-level Windows 11 menu is
Phase 7.

## Uninstall on the real registry

`tumble menu uninstall`, run against the real install (50 extensions):

- No `SystemFileAssociations\.<ext>\shell\Tumble` verb left, and no
  `Tumble` or `AppUserModelId\Tumble.Converter` key left.
- 100 other keys were removed, every one listed in the install record's
  `CreatedKeys`. `SystemFileAssociations` itself was among them; it did not
  exist on this PC before the first install.
- `%LOCALAPPDATA%\Tumble\tumble.ico` was removed.

The menu was then installed again.
