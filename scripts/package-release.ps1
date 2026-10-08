<#
.SYNOPSIS
  Builds the release zip: dist/tumble-<version>-win-x64.zip.

.DESCRIPTION
  1. Builds tumble.exe and tumblew.exe in release mode.
  2. Makes sure vendor/ is populated (runs fetch-vendor.ps1 if not).
  3. Stages the exes, vendor DLLs, licences and README into
     dist/tumble-<version>-win-x64/ and zips it.
  4. Unzips the result into an empty temp folder and converts real files
     from there, so a broken zip is caught here and not by whoever opens it.

  Developer tooling only. The packaged app never touches the network.

.PARAMETER Lgpl
  Package the LGPL libheif build without x265 (HEIC read only). Use for
  builds you share widely.

.PARAMETER Desktop
  Also build and include the desktop window, tumble-desktop.exe (Phase 6).
  It needs Bun and the WebView2 runtime that ships with Windows 11.
#>
param([switch]$Lgpl, [switch]$Desktop)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$name = "tumble-$version-win-x64" + ($(if ($Lgpl) { '-lgpl' } else { '' }))
$dist = Join-Path $root 'dist'
$stage = Join-Path $dist $name
$zip = "$stage.zip"

Write-Host "== building release"
& (Join-Path $PSScriptRoot 'fetch-vendor.ps1') -Lgpl:$Lgpl
cargo build --release --bin tumble --bin tumblew
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

Write-Host "== staging $name"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
if (Test-Path $zip) { Remove-Item -Force $zip }
New-Item -ItemType Directory -Force $stage, (Join-Path $stage 'licences') | Out-Null
Copy-Item target\release\tumble.exe, target\release\tumblew.exe $stage
if ($Desktop) {
    Write-Host "== building the desktop window"
    Push-Location apps\desktop
    try {
        bun install --frozen-lockfile
        bun run tauri build
        if ($LASTEXITCODE -ne 0) { throw "tauri build failed" }
    }
    finally { Pop-Location }
    Copy-Item apps\desktop\src-tauri\target\release\tumble-desktop.exe $stage
}
Get-ChildItem vendor -Filter *.dll | Copy-Item -Destination $stage
Get-ChildItem vendor -Filter *-LICENSE.txt | Copy-Item -Destination (Join-Path $stage 'licences')
(Get-Content packaging\README.txt -Raw).Replace('{VERSION}', $version) |
    Set-Content (Join-Path $stage 'README.txt') -Encoding utf8

# The Rust crates compiled into tumble.exe, with their licences.
$crates = cargo tree --prefix none --edges normal,build --target x86_64-pc-windows-msvc `
    --format '{p} | {l}' -p tumble-cli |
    ForEach-Object { $_ -replace ' \(.*?\)', '' -replace ' \(\*\)$', '' } |
    Where-Object { $_ -notmatch '^tumble' }
if ($Desktop) {
    $crates += cargo tree --manifest-path apps\desktop\src-tauri\Cargo.toml --prefix none `
        --edges normal,build --target x86_64-pc-windows-msvc --format '{p} | {l}' |
        ForEach-Object { $_ -replace ' \(.*?\)', '' -replace ' \(\*\)$', '' } |
        Where-Object { $_ -notmatch '^tumble' }
}
$crates = $crates | Sort-Object -Unique
@(
    "Third-party software in tumble.exe, tumblew.exe and tumble-desktop.exe"
    "(Rust crates, compiled in; name version | licence)"
    ""
) + $crates + @(
    ""
    "DLLs shipped alongside: see the *-LICENSE.txt files in this folder."
) | Set-Content (Join-Path $stage 'licences\THIRD-PARTY.txt') -Encoding utf8

Compress-Archive -Path "$stage\*" -DestinationPath $zip -CompressionLevel Optimal

Write-Host "== smoke test from a clean folder"
$test = Join-Path ([IO.Path]::GetTempPath()) ("tumble-release-test-" + [Guid]::NewGuid())
New-Item -ItemType Directory $test | Out-Null
try {
    Expand-Archive $zip -DestinationPath (Join-Path $test 'app')
    $exe = Join-Path $test 'app\tumble.exe'
    $work = Join-Path $test 'files'
    New-Item -ItemType Directory $work | Out-Null

    # A small PNG and a two-page PDF, made with the packaged tumble itself.
    Set-Content (Join-Path $work 'card.svg') '<svg xmlns="http://www.w3.org/2000/svg" width="64" height="48"><rect width="64" height="48" fill="#7eb328"/></svg>'
    & $exe (Join-Path $work 'card.svg') --to png | Out-Null
    $pdf = "%PDF-1.4`n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj`n2 0 obj<</Type/Pages/Kids[3 0 R 4 0 R]/Count 2>>endobj`n3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 72 72]>>endobj`n4 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 72 72]>>endobj`ntrailer<</Root 1 0 R>>"
    [IO.File]::WriteAllText((Join-Path $work 'doc.pdf'), $pdf)

    $checks = @(
        @{ Args = @('engines'); Expect = 0 },
        @{ Args = @((Join-Path $work 'card.png'), '--to', 'avif'); Expect = 0; File = 'card.avif' },
        @{ Args = @((Join-Path $work 'card.png'), '--to', 'heic'); Expect = $(if ($Lgpl) { 3 } else { 0 }); File = $(if ($Lgpl) { $null } else { 'card.heic' }) },
        @{ Args = @((Join-Path $work 'doc.pdf'), '--to', 'jpg'); Expect = 0; File = 'doc-p002.jpg' }
    )
    foreach ($c in $checks) {
        $out = & $exe @($c.Args) 2>&1
        if ($LASTEXITCODE -ne $c.Expect) {
            throw "tumble $($c.Args -join ' ') exited $LASTEXITCODE (expected $($c.Expect)):`n$out"
        }
        if ($c.File -and -not (Test-Path (Join-Path $work $c.File))) {
            throw "tumble $($c.Args -join ' ') did not write $($c.File)"
        }
        Write-Host ("  ok: tumble " + (($c.Args | ForEach-Object { Split-Path $_ -Leaf }) -join ' '))
    }
    $engines = & $exe engines
    if (-not ($engines -match 'libheif \[ok\]') -or -not ($engines -match 'pdfium \[ok\]')) {
        throw "packaged DLLs not picked up:`n$engines"
    }
}
finally {
    Remove-Item -Recurse -Force $test -ErrorAction SilentlyContinue
}

$exes = (Get-Item "$stage\tumble.exe").Length + (Get-Item "$stage\tumblew.exe").Length
$dlls = (Get-ChildItem $stage -Filter *.dll | Measure-Object Length -Sum).Sum
Write-Host ("== {0}" -f $zip)
Write-Host ("   zip {0:N1} MB; tumble.exe + tumblew.exe {1:N1} MB (budget 15); DLLs {2:N1} MB" -f `
    ((Get-Item $zip).Length / 1MB), ($exes / 1MB), ($dlls / 1MB))
