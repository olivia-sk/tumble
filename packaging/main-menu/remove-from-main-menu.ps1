# Removes Tumble from the main Windows 11 right-click menu and stops trusting
# its certificate. Run it with "Remove from main menu.cmd" in this folder.
# Tumble stays under "Show more options".

$ErrorActionPreference = 'Stop'

Get-AppxPackage -Name Tumble.ExplorerMenu | Remove-AppxPackage

$store = New-Object Security.Cryptography.X509Certificates.X509Store('TrustedPeople', 'LocalMachine')
$store.Open('ReadOnly')
$found = @($store.Certificates | Where-Object { $_.Subject -in 'CN=Tumble', 'CN=Tumble Dev' }).Count
$store.Close()

if ($found) {
    Write-Host "Windows will ask for admin rights to remove Tumble's certificate; click Yes."
    $command = "`$s = New-Object Security.Cryptography.X509Certificates.X509Store('TrustedPeople', 'LocalMachine'); " +
        "`$s.Open('ReadWrite'); " +
        "`$s.Certificates | Where-Object { `$_.Subject -in 'CN=Tumble', 'CN=Tumble Dev' } | ForEach-Object { `$s.Remove(`$_) }"
    try {
        Start-Process powershell.exe -Verb RunAs -Wait -WindowStyle Hidden `
            -ArgumentList '-NoProfile', '-NonInteractive', '-Command', $command
    } catch {
        Write-Host 'The menu was removed, but the certificate is still trusted because admin rights were declined.'
        exit 1
    }
}
Write-Host 'Done. Tumble is back under "Show more options" only.'
