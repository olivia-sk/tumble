# Phase 7: Windows 11 top-level menu

- Date: 2026-10-07
- Machine: Olivia's PC, Windows 11 Home (build 26300), x64
- Build: commit `0114f65`; `tumble_explorer.dll` next to
  `target\release\tumblew.exe`
- Package: `Tumble.ExplorerMenu` 0.1.0.0, sparse MSIX signed with the
  self-signed `CN=Tumble Dev` certificate, external location
  `target\release`

Steps:

1. `explorer-menu.ps1 -Action build`, run on the development machine: built the DLL,
   generated the manifest (50 extensions) and logos, packed and signed the
   MSIX. No certificate store or registry was touched.
2. `explorer-menu.ps1 -Action install`, run by Olivia in an elevated
   terminal: trusted the certificate in `LocalMachine\TrustedPeople` and
   registered the package.
3. Olivia right-clicked files in Explorer: "Convert to" appears in the
   main Windows 11 menu and converts as expected (reported working).

`Get-AppxPackage Tumble.ExplorerMenu` afterwards: status `Ok`.

The classic entries under "Show more options" remain as the fallback, as
the PRD asks.
