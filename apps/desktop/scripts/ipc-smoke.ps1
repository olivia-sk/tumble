<#
.SYNOPSIS
  End-to-end check of the built desktop app: starts tumble-desktop.exe with
  WebView2's debugging port open (for this run only), then calls the same
  Tauri commands the UI uses (engines, inspect, convert) on a real PNG and
  checks the converted file on disk.

  Developer tooling. The port listens on 127.0.0.1 and the app is closed
  at the end.
#>
param(
    [string]$Exe = (Join-Path $PSScriptRoot '..\src-tauri\target\release\tumble-desktop.exe'),
    [int]$Port = 9339
)
$ErrorActionPreference = 'Stop'

$work = Join-Path ([IO.Path]::GetTempPath()) ("tumble-ipc-" + [Guid]::NewGuid())
New-Item -ItemType Directory $work | Out-Null
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap 40, 30
[System.Drawing.Graphics]::FromImage($bmp).Clear([System.Drawing.Color]::OliveDrab)
$png = Join-Path $work 'sample image.png'
$bmp.Save($png, [System.Drawing.Imaging.ImageFormat]::Png)

$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port --remote-allow-origins=*"
$app = Start-Process $Exe -PassThru
Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
try {
    $target = $null
    for ($i = 0; $i -lt 50 -and -not $target; $i++) {
        Start-Sleep -Milliseconds 200
        try { $target = (Invoke-RestMethod "http://127.0.0.1:$Port/json") | Where-Object type -eq 'page' | Select-Object -First 1 } catch {}
    }
    if (-not $target) { throw "WebView2 debugging port did not come up" }

    $ws = New-Object System.Net.WebSockets.ClientWebSocket
    $ws.ConnectAsync([Uri]$target.webSocketDebuggerUrl, [Threading.CancellationToken]::None).Wait()
    $script:id = 0
    function Eval([string]$js) {
        $script:id++
        $msg = @{ id = $script:id; method = 'Runtime.evaluate'; params = @{ expression = $js; awaitPromise = $true; returnByValue = $true } } | ConvertTo-Json -Depth 5 -Compress
        $bytes = [Text.Encoding]::UTF8.GetBytes($msg)
        $ws.SendAsync([ArraySegment[byte]]$bytes, 'Text', $true, [Threading.CancellationToken]::None).Wait()
        while ($true) {
            $buf = New-Object byte[] 1048576; $sb = New-Object Text.StringBuilder
            do {
                $r = $ws.ReceiveAsync([ArraySegment[byte]]$buf, [Threading.CancellationToken]::None).Result
                [void]$sb.Append([Text.Encoding]::UTF8.GetString($buf, 0, $r.Count))
            } while (-not $r.EndOfMessage)
            $reply = $sb.ToString() | ConvertFrom-Json
            if ($reply.id -eq $script:id) {
                if ($reply.result.exceptionDetails) { throw "JS error: $($reply.result.exceptionDetails.exception.description)" }
                return $reply.result.result.value
            }
        }
    }

    # Wait for the page's script to load.
    for ($i = 0; $i -lt 50 -and -not (Eval "!!window.__TAURI_INTERNALS__"); $i++) { Start-Sleep -Milliseconds 100 }
    $invoke = 'window.__TAURI_INTERNALS__.invoke'

    $engines = Eval "$invoke('engines')"
    Write-Host ("engines: " + (($engines | ForEach-Object { "$($_.name)=$($_.available)" }) -join ', '))

    $jsPath = $png.Replace('\', '\\')
    $inspected = Eval "$invoke('inspect', { paths: ['$jsPath'] })"
    $file = $inspected.files[0]
    Write-Host ("inspect: {0} ({1}), {2} targets" -f $file.name, $file.format, $file.targets.Count)
    if ($file.format -ne 'PNG' -or -not ($file.targets.id -contains 'webp')) { throw "inspect returned the wrong thing" }

    $outputs = Eval "$invoke('convert', { job: 1, path: '$jsPath', to: 'webp', quality: 80, resize: null, preset: null, outDir: null })"
    Write-Host "convert: $outputs"
    $webp = Join-Path $work 'sample image.webp'
    if (-not (Test-Path $webp)) { throw "no WebP written" }
    $head = [IO.File]::ReadAllBytes($webp)[0..11]
    if ([Text.Encoding]::ASCII.GetString($head, 8, 4) -ne 'WEBP') { throw "output is not a WebP" }

    $bad = Eval "$invoke('convert', { job: 2, path: '$jsPath', to: 'svg', quality: null, resize: null, preset: null, outDir: null }).then(() => 'resolved', e => 'rejected: ' + e)"
    Write-Host "bad target: $bad"
    if ($bad -notlike 'rejected*') { throw "an input-only target should be refused" }
    Write-Host "OK: the window's commands convert real files through the engines"
}
finally {
    if (-not $app.HasExited) { Stop-Process -Id $app.Id }
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
