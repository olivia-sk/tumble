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
; Both are unchecked and only offered when winget is available and the
; program isn't installed yet. They are separate apps, so uninstalling
; Tumble leaves them alone.
Name: ffmpeg; GroupDescription: "Optional free programs:"; Flags: unchecked; Check: CanInstallFFmpeg; \
    Description: "FFmpeg (about 110 MB download): lets Tumble convert video and audio, like MP4 to MP3"
Name: libreoffice; GroupDescription: "Optional free programs:"; Flags: unchecked; Check: CanInstallLibreOffice; \
    Description: "LibreOffice (about 350 MB download): lets Tumble convert documents, slides and spreadsheets, like Word to PDF; Windows will ask for admin rights"
Name: desktopicon; GroupDescription: "Shortcuts:"; Flags: unchecked; \
    Description: "Create a desktop shortcut for the desktop window"

[Messages]
SelectTasksLabel2=Tumble converts images and PDFs on its own. To convert video, audio and documents too, it can install these free programs from their official sources. They install as separate apps, stay on your PC if you uninstall Tumble, and can also be installed later.

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
    '$s.Open(''ReadOnly''); if ($s.Certificates | Where-Object { $_.Subject -in ''CN=Tumble'', ''CN=Tumble Dev'' }) { exit 1 }"';
  CertRemove = '-NoProfile -NonInteractive -Command "' + CertStore +
    '$s.Open(''ReadWrite''); $s.Certificates | Where-Object { $_.Subject -in ''CN=Tumble'', ''CN=Tumble Dev'' } | ForEach-Object { $s.Remove($_) }"';

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

function OnPath(Exe: string): Boolean;
begin
  Result := FileSearch(Exe, GetEnv('PATH')) <> '';
end;

function DirMatches(Pattern: string): Boolean;
var
  Found: TFindRec;
begin
  Result := FindFirst(Pattern, Found);
  if Result then
    FindClose(Found);
end;

function HasWinget: Boolean;
begin
  Result := OnPath('winget.exe') or FileExists(ExpandConstant('{localappdata}\Microsoft\WindowsApps\winget.exe'));
end;

{ The same places Tumble looks (crates/tumble-engines/src/ffmpeg and office). }
function HasFFmpeg: Boolean;
begin
  Result := OnPath('ffmpeg.exe')
    or DirMatches(ExpandConstant('{localappdata}\Microsoft\WinGet\Packages\Gyan.FFmpeg*'))
    or FileExists(ExpandConstant('{localappdata}\Microsoft\WinGet\Links\ffmpeg.exe'))
    or FileExists(GetEnv('USERPROFILE') + '\scoop\apps\ffmpeg\current\bin\ffmpeg.exe')
    or FileExists('C:\ProgramData\chocolatey\bin\ffmpeg.exe')
    or FileExists('C:\Program Files\ffmpeg\bin\ffmpeg.exe')
    or FileExists('C:\ffmpeg\bin\ffmpeg.exe');
end;

function HasLibreOffice: Boolean;
begin
  Result := OnPath('soffice.exe')
    or FileExists('C:\Program Files\LibreOffice\program\soffice.exe')
    or FileExists('C:\Program Files (x86)\LibreOffice\program\soffice.exe')
    or FileExists(ExpandConstant('{localappdata}\Programs\LibreOffice\program\soffice.exe'));
end;

function CanInstallFFmpeg: Boolean;
begin
  Result := HasWinget and not HasFFmpeg;
end;

function CanInstallLibreOffice: Boolean;
begin
  Result := HasWinget and not HasLibreOffice;
end;

{ Programs this installer installed, one winget id per line. The uninstaller
  only offers to remove these, never ones the user installed some other way.
  The file is in the install folder, so it goes when Tumble does. }
function InstalledListFile: string;
begin
  Result := ExpandConstant('{app}\optional-programs.txt');
end;

procedure RememberInstalled(Id: string);
begin
  SaveStringToFile(InstalledListFile, Id + #13#10, True);
end;

function WasInstalledByTumble(Id: string): Boolean;
var
  Lines: TArrayOfString;
  I: Integer;
begin
  Result := False;
  if LoadStringsFromFile(InstalledListFile, Lines) then
    for I := 0 to GetArrayLength(Lines) - 1 do
      if CompareText(Trim(Lines[I]), Id) = 0 then
        Result := True;
end;

procedure InstallWithWinget(Id, Name, Extra: string);
var
  Code: Integer;
begin
  WizardForm.StatusLabel.Caption := 'Downloading and installing ' + Name + '. This can take a few minutes...';
  WizardForm.ProgressGauge.Style := npbstMarquee;
  try
    if not Exec('winget.exe', 'install --id ' + Id + ' -e --silent --disable-interactivity ' +
        '--accept-package-agreements --accept-source-agreements' + Extra,
        '', SW_HIDE, ewWaitUntilTerminated, Code) or (Code <> 0) then
    begin
      Log(Format('winget install %s failed with code %d.', [Id, Code]));
      SuppressibleMsgBox(Name + ' could not be installed, but Tumble is fine without it. ' +
        'You can install it later by running this in a terminal:' + #13#10#13#10 +
        'winget install ' + Id + #13#10#13#10 +
        'Then run "tumble menu install" so the menu shows the new formats.',
        mbInformation, MB_OK, IDOK);
    end else
      RememberInstalled(Id);
  finally
    WizardForm.ProgressGauge.Style := npbstNormal;
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Code: Integer;
begin
  if CurStep = ssPostInstall then
  begin
    if WizardIsTaskSelected('ffmpeg') then
      InstallWithWinget('Gyan.FFmpeg.Essentials', 'FFmpeg', ' --scope user');
    if WizardIsTaskSelected('libreoffice') then
      InstallWithWinget('TheDocumentFoundation.LibreOffice', 'LibreOffice', '');
    { After the optional programs, so the menu lists their formats. }
    WizardForm.StatusLabel.Caption := 'Adding the right-click menu...';
    Exec(ExpandConstant('{app}\tumble.exe'), 'menu install', '', SW_HIDE, ewWaitUntilTerminated, Code);
  end;
end;

{ Asks before removing a program Tumble installed. No is the default, and a
  silent uninstall keeps it. }
procedure OfferToRemove(Id, Name, Note: string);
var
  Code: Integer;
begin
  if not WasInstalledByTumble(Id) then
    Exit;
  if SuppressibleMsgBox('Tumble''s installer also installed ' + Name + '. Do you want to remove it too?' + #13#10#13#10 +
      'Other apps on your PC might use it.' + Note,
      mbConfirmation, MB_YESNO or MB_DEFBUTTON2, IDNO) <> IDYES then
    Exit;
  UninstallProgressForm.StatusLabel.Caption := 'Removing ' + Name + '...';
  { -1978335212 (0x8A150014): no longer installed, which is fine. }
  if not Exec('winget.exe', 'uninstall --id ' + Id + ' -e --silent --disable-interactivity --accept-source-agreements',
      '', SW_HIDE, ewWaitUntilTerminated, Code) or ((Code <> 0) and (Code <> -1978335212)) then
  begin
    Log(Format('winget uninstall %s failed with code %d.', [Id, Code]));
    SuppressibleMsgBox(Name + ' could not be removed. You can remove it from Settings > Apps > Installed apps.',
      mbInformation, MB_OK, IDOK);
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  { Before the files go, since the list of installed programs is among them. }
  if CurUninstallStep = usUninstall then
  begin
    OfferToRemove('Gyan.FFmpeg.Essentials', 'FFmpeg', '');
    OfferToRemove('TheDocumentFoundation.LibreOffice', 'LibreOffice',
      ' Removing it needs admin rights, so Windows will ask.');
  end;
  if CurUninstallStep = usPostUninstall then
  begin
    RemoveFromPath(ExpandConstant('{app}'));
    RegDeleteKeyIncludingSubkeys(HKCU, NotificationsKey);
    RemoveExplorerMenuCertificate;
  end;
end;
