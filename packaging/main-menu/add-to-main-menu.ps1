# Adds Tumble to the main Windows 11 right-click menu. Run it with
# "Add to main menu.cmd" in this folder.
#
# Windows only shows apps in that menu when they come in a signed package,
# and it only accepts the signature once the certificate is trusted. Trusting
# tumble.cer needs admin rights, so Windows asks once; the package itself is
# then added for your user only. The private key for tumble.cer was deleted
# after signing this package, so the certificate can't vouch for anything else.

$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
$app = Split-Path $here
$msix = Join-Path $here 'tumble-main-menu.msix'
$cer = Join-Path $here 'tumble.cer'

if (-not (Test-Path (Join-Path $app 'tumblew.exe'))) {
    throw "tumblew.exe not found in $app. Keep this folder inside the Tumble folder."
}
# The main menu reads the classic menu's settings, so make sure it's there.
if (-not (Test-Path 'HKCU:\Software\Classes\Tumble')) {
    & (Join-Path $app 'tumble.exe') menu install | Out-Null
}

$thumb = [Security.Cryptography.X509Certificates.X509Certificate2]::new($cer).Thumbprint
$store = New-Object Security.Cryptography.X509Certificates.X509Store('TrustedPeople', 'LocalMachine')
$store.Open('ReadOnly')
$trusted = $store.Certificates.Find('FindByThumbprint', $thumb, $false).Count -gt 0
$store.Close()

if (-not $trusted) {
    Write-Host "Windows will ask for admin rights to trust Tumble's certificate; click Yes."
    # Also removes certificates from older Tumble releases.
    $path = $cer.Replace("'", "''")
    $command = "`$s = New-Object Security.Cryptography.X509Certificates.X509Store('TrustedPeople', 'LocalMachine'); " +
        "`$s.Open('ReadWrite'); " +
        "`$s.Certificates | Where-Object { `$_.Subject -eq 'CN=Tumble' } | ForEach-Object { `$s.Remove(`$_) }; " +
        "`$s.Add([Security.Cryptography.X509Certificates.X509Certificate2]::new('$path'))"
    try {
        Start-Process powershell.exe -Verb RunAs -Wait -WindowStyle Hidden `
            -ArgumentList '-NoProfile', '-NonInteractive', '-Command', $command
    } catch {
        throw 'Admin rights are needed to trust the certificate, so nothing was changed.'
    }
}

Get-AppxPackage -Name Tumble.ExplorerMenu | Remove-AppxPackage
Add-AppxPackage -Path $msix -ExternalLocation $app
Write-Host 'Done. Tumble is now in the main right-click menu.'
Write-Host 'If it does not show yet, restart File Explorer (Task Manager > Windows Explorer > Restart).'
