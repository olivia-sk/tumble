<#
.SYNOPSIS
  Downloads the pinned PDFium and libheif builds into vendor/ and checks their SHA-256.

.DESCRIPTION
  Developer tooling only. Tumble itself never downloads anything.

  PDFium comes from bblanchon/pdfium-binaries. libheif and the DLLs it
  loads come from conda-forge, which publishes MSVC builds:
    heif.dll       libheif 1.23.6       LGPL-3.0
    libde265.dll   HEVC decoder         LGPL-3.0
    libx265.dll    HEVC encoder         GPL-2.0   (skipped with -Lgpl)
    aom.dll        AV1 codec            BSD-2     (heif.dll links it)
    msvcp140.dll, vcruntime140.dll, vcruntime140_1.dll
                   Visual C++ runtime, so the zip runs without the redistributable
  heif.dll imports only these; conda's metadata also lists libavif and dav1d,
  which the DLL does not load.

  Each package's licence files are copied next to its DLLs.
  Re-running is safe: packages whose hash already matches are skipped.

.PARAMETER Lgpl
  Use the LGPL build of libheif without x265. HEIC can then be read but
  not written. Use this for builds you share (see PRD section 18).

.PARAMETER Force
  Download again even when up to date.
#>
param([switch]$Lgpl, [switch]$Force)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$root = Split-Path -Parent $PSScriptRoot
$vendor = Join-Path $root 'vendor'
New-Item -ItemType Directory -Force -Path $vendor | Out-Null

$conda = 'https://conda.anaconda.org/conda-forge/win-64'

# Pinned builds. Update the URL and hash together.
# Files: path inside the package -> name in vendor/.
$packages = @(
    @{
        Name   = 'pdfium'
        Url    = 'https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8086/pdfium-win-x64.tgz'
        Sha256 = '1fd8af952832dbb0eb16d9249f68fe09e5f5ebf7c3dd9f6066ea2720cc28487d'
        Files  = [ordered]@{ 'bin/pdfium.dll' = 'pdfium.dll'; 'LICENSE' = 'pdfium-LICENSE.txt' }
    }
    if ($Lgpl) {
        @{
            Name   = 'libheif'
            Url    = "$conda/libheif-1.23.6-lgpl_h8da1c84_0.conda"
            Sha256 = 'dc43e9edfb86514b4569950932f922e9194f7055892e427e5e05e1471ee748c6'
            Files  = [ordered]@{ 'Library/bin/heif.dll' = 'heif.dll'; 'info/licenses/COPYING' = 'libheif-LICENSE.txt' }
        }
    } else {
        @{
            Name   = 'libheif'
            Url    = "$conda/libheif-1.23.6-gpl_hbd75871_100.conda"
            Sha256 = '94319b47c1161018401c304ef0d7598c87ecd37e85c78bba25fca2e541eb1953'
            Files  = [ordered]@{ 'Library/bin/heif.dll' = 'heif.dll'; 'info/licenses/COPYING' = 'libheif-LICENSE.txt' }
        }
        @{
            Name   = 'x265'
            Url    = "$conda/x265-3.5-h3ee6617_4.conda"
            Sha256 = '684d28d37d808e4cefbcc87e8039f8f0cc3a6ed16c91d05731f6e1f0bf79a3eb'
            Files  = [ordered]@{ 'Library/bin/libx265.dll' = 'libx265.dll'; 'info/licenses/COPYING' = 'x265-LICENSE.txt' }
        }
    }
    @{
        Name   = 'libde265'
        Url    = "$conda/libde265-1.1.3-h477610d_1.conda"
        Sha256 = '1ad6ce985e8306ce74026a110898a7c91b41b927d1bfa57f3935270f5e199d5d'
        Files  = [ordered]@{ 'Library/bin/libde265.dll' = 'libde265.dll'; 'info/licenses/COPYING' = 'libde265-LICENSE.txt' }
    }
    @{
        Name   = 'aom'
        Url    = "$conda/aom-3.14.1-pl5321h06fc181_2.conda"
        Sha256 = 'f8c8f820dcac04954bd7d6c116f4278cf96143f291d15417c7b35b015c0c6423'
        Files  = [ordered]@{ 'Library/bin/aom.dll' = 'aom.dll'; 'info/licenses/LICENSE' = 'aom-LICENSE.txt' }
    }
    @{
        Name   = 'vc14_runtime'
        Url    = "$conda/vc14_runtime-14.51.36247-habf1de7_41.conda"
        Sha256 = '4e4cb599cdc41bf2109d1464c127b5bcbddf548ce3e322e612afb691338b48f8'
        Files  = [ordered]@{
            'Library/bin/msvcp140.dll'       = 'msvcp140.dll'
            'Library/bin/vcruntime140.dll'   = 'vcruntime140.dll'
            'Library/bin/vcruntime140_1.dll' = 'vcruntime140_1.dll'
            'info/licenses/LICENSE.TXT'      = 'vcruntime-LICENSE.txt'
        }
    }
)

# Switching to -Lgpl must not leave the GPL encoder behind.
if ($Lgpl) {
    foreach ($stale in 'libx265.dll', 'x265-LICENSE.txt', 'x265.sha256') {
        $p = Join-Path $vendor $stale
        if (Test-Path $p) { Remove-Item -Force $p }
    }
}

# Unpacks an archive into $dest. A .conda file is a zip holding
# info-*.tar.zst and pkg-*.tar.zst; Windows' tar reads all of these.
function Expand-Package([string]$archive, [string]$dest) {
    New-Item -ItemType Directory -Force -Path $dest | Out-Null
    if ($archive.EndsWith('.conda')) {
        $outer = Join-Path $dest '.outer'
        New-Item -ItemType Directory -Path $outer | Out-Null
        tar -xf $archive -C $outer
        if ($LASTEXITCODE -ne 0) { throw "tar failed on $archive" }
        foreach ($inner in Get-ChildItem $outer -Filter '*.tar.zst') {
            tar -xf $inner.FullName -C $dest
            if ($LASTEXITCODE -ne 0) { throw "tar failed on $($inner.Name)" }
        }
    } else {
        tar -xzf $archive -C $dest
        if ($LASTEXITCODE -ne 0) { throw "tar failed on $archive" }
    }
}

$work = Join-Path ([IO.Path]::GetTempPath()) ("tumble-vendor-" + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $work | Out-Null
try {
    foreach ($p in $packages) {
        $label = "$($p.Name) ($(Split-Path -Leaf $p.Url))"
        $stamp = Join-Path $vendor "$($p.Name).sha256"
        $have = (Test-Path $stamp) -and ((Get-Content $stamp -Raw).Trim() -eq $p.Sha256) -and
                -not ($p.Files.Values | Where-Object { -not (Test-Path (Join-Path $vendor $_)) })
        if ($have -and -not $Force) {
            Write-Host "${label}: up to date"
            continue
        }

        Write-Host "${label}: downloading"
        $archive = Join-Path $work (Split-Path -Leaf $p.Url)
        Invoke-WebRequest -Uri $p.Url -OutFile $archive -UseBasicParsing
        $actual = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
        if ($actual -ne $p.Sha256) {
            throw "$($p.Name): SHA-256 mismatch. Expected $($p.Sha256), got $actual"
        }

        $extract = Join-Path $work $p.Name
        Expand-Package $archive $extract
        foreach ($src in $p.Files.Keys) {
            $from = Join-Path $extract $src
            if (-not (Test-Path $from)) { throw "$($p.Name): $src not found in package" }
            Copy-Item -Force $from (Join-Path $vendor $p.Files[$src])
        }
        Set-Content -Path $stamp -Value $p.Sha256 -NoNewline
        Write-Host "${label}: ok"
    }
}
finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}

$dlls = Get-ChildItem $vendor -Filter *.dll
Write-Host ("vendor/: {0} DLLs, {1:N1} MB" -f $dlls.Count, (($dlls | Measure-Object Length -Sum).Sum / 1MB))
