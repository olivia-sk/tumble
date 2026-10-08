; Inno Setup script for the Tumble installer. Built by
; scripts/package-release.ps1 -Installer, which passes AppVersion and
; StageDir (the staged release folder).
;
; Installs for the current user only (no admin rights), adds tumble.exe to
; PATH and runs `tumble menu install`. The uninstaller removes everything
; Tumble creates: the menu's registry keys, settings, presets, logs, the
; desktop window's WebView data, the PATH entry, notification settings and,
; if it was set up, the Windows 11 top-level menu and its certificate.

#ifndef AppVersion
  #error AppVersion is not defined
#endif
#ifndef StageDir
  #error StageDir is not defined
#endif

[Setup]
AppId={{5B8E2F4A-9C3D-4E71-A6B0-1D2F3C4E5A6B}
AppName=Tumble
AppVersion={#AppVersion}
AppVerName=Tumble {#AppVersion}
AppPublisher=olivia-sk
AppPublisherURL=https://github.com/olivia-sk/tumble
AppSupportURL=https://github.com/olivia-sk/tumble/issues
DefaultDirName={userpf}\Tumble
DefaultGroupName=Tumble
DisableProgramGroupPage=yes
DisableReadyPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
ChangesEnvironment=yes
CloseApplications=yes
RestartApplications=no
LicenseFile=..\LICENSE
SetupIconFile=..\assets\tumble.ico
UninstallDisplayIcon={app}\tumble-desktop.exe
UninstallDisplayName=Tumble
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
OutputDir=..\dist
OutputBaseFilename=tumble-{#AppVersion}-setup

[Tasks]
Name: desktopicon; Description: "Create a desktop shortcut for the desktop window"; Flags: unchecked

[Files]
; README.txt explains the zip, so the installer leaves it out.
Source: "{#StageDir}\*"; DestDir: "{app}"; Excludes: "README.txt"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{group}\Tumble"; Filename: "{app}\tumble-desktop.exe"
Name: "{group}\Uninstall Tumble"; Filename: "{uninstallexe}"
Name: "{userdesktop}\Tumble"; Filename: "{app}\tumble-desktop.exe"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; \
    ValueData: "{olddata};{app}"; Check: NeedsAddPath(ExpandConstant('{app}'))

[Run]
Filename: "{app}\tumble.exe"; Parameters: "menu install"; Flags: runhidden; \
    StatusMsg: "Adding the right-click menu..."

[UninstallRun]
Filename: "{app}\tumble.exe"; Parameters: "menu uninstall"; Flags: runhidden; \
    RunOnceId: "MenuUninstall"
Filename: "powershell.exe"; \
    Parameters: "-NoProfile -NonInteractive -Command ""Get-AppxPackage -Name Tumble.ExplorerMenu | Remove-AppxPackage"""; \
    Flags: runhidden; RunOnceId: "ExplorerMenuUninstall"

[UninstallDelete]
Type: filesandordirs; Name: "{app}"
Type: filesandordirs; Name: "{userappdata}\Tumble"
Type: filesandordirs; Name: "{localappdata}\Tumble"
Type: filesandordirs; Name: "{localappdata}\dev.tumble.desktop"

[Code]
const
  EnvKey = 'Environment';
  NotificationsKey = 'Software\Microsoft\Windows\CurrentVersion\Notifications\Settings\Tumble.Converter';
  { X509Store instead of the Cert: drive, which needs a module that fails to
    load when PSModulePath comes from PowerShell 7. }
  CertStore = '$s = New-Object Security.Cryptography.X509Certificates.X509Store(''TrustedPeople'', ''LocalMachine''); ';
  CertCheck = '-NoProfile -NonInteractive -Command "' + CertStore +
    '$s.Open(''ReadOnly''); if ($s.Certificates | Where-Object { $_.Subject -eq ''CN=Tumble Dev'' }) { exit 1 }"';
  CertRemove = '-NoProfile -NonInteractive -Command "' + CertStore +
    '$s.Open(''ReadWrite''); $s.Certificates | Where-Object { $_.Subject -eq ''CN=Tumble Dev'' } | ForEach-Object { $s.Remove($_) }"';

{ Path entries are compared case-insensitively, ignoring a trailing slash. }
function PathHas(Paths, Dir: string): Boolean;
begin
  Result := Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(Paths) + ';') > 0;
end;

function NeedsAddPath(Dir: string): Boolean;
var
  Paths: string;
begin
  if not RegQueryStringValue(HKCU, EnvKey, 'Path', Paths) then
    Result := True
  else
    Result := not PathHas(Paths, Dir) and not PathHas(Paths, Dir + '\');
end;

procedure RemoveFromPath(Dir: string);
var
  Paths, Rest, Entry, Kept: string;
  I: Integer;
begin
  if not RegQueryStringValue(HKCU, EnvKey, 'Path', Paths) then
    Exit;
  Rest := Paths + ';';
  Kept := '';
  while Rest <> '' do
  begin
    I := Pos(';', Rest);
    Entry := Copy(Rest, 1, I - 1);
    Delete(Rest, 1, I);
    if (Entry <> '') and (CompareText(RemoveBackslashUnlessRoot(Entry), Dir) <> 0) then
    begin
      if Kept <> '' then
        Kept := Kept + ';';
      Kept := Kept + Entry;
    end;
  end;
  if Kept <> Paths then
    RegWriteExpandStringValue(HKCU, EnvKey, 'Path', Kept);
end;

{ The Windows 11 top-level menu's certificate is in the machine store, so
  removing it needs admin rights. Only ask when it is actually there. }
procedure RemoveExplorerMenuCertificate;
var
  Code: Integer;
begin
  if not Exec('powershell.exe', CertCheck, '', SW_HIDE, ewWaitUntilTerminated, Code) then
    Log(Format('Certificate check did not run: %s', [SysErrorMessage(Code)]))
  else
    Log(Format('Certificate check exited with code %d.', [Code]));
  if Code = 1 then
  begin
    Log('Windows 11 menu certificate found; asking for admin rights to remove it.');
    if ShellExec('runas', 'powershell.exe', CertRemove, '', SW_HIDE, ewWaitUntilTerminated, Code) then
      Log(Format('Certificate removal exited with code %d.', [Code]))
    else
      Log(Format('Certificate removal did not run: %s', [SysErrorMessage(Code)]));
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    RemoveFromPath(ExpandConstant('{app}'));
    RegDeleteKeyIncludingSubkeys(HKCU, NotificationsKey);
    RemoveExplorerMenuCertificate;
  end;
end;
