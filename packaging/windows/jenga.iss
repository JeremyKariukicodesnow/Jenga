#ifndef AppVersion
  #error AppVersion must be supplied by scripts/build-release.ps1
#endif
#ifndef PayloadDir
  #error PayloadDir must be supplied by scripts/build-release.ps1
#endif
#ifndef OutputDir
  #error OutputDir must be supplied by scripts/build-release.ps1
#endif

[Setup]
; Never change this ID between versions: it is the installation/upgrade identity.
AppId={{786CEB53-E382-4F02-97AB-706854377AC4}
AppName=Jenga
AppVersion={#AppVersion}
AppPublisher=Jenga contributors
DefaultDirName={localappdata}\Programs\Jenga
DefaultGroupName=Jenga
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UsePreviousAppDir=yes
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\Jenga.exe
LicenseFile={#PayloadDir}\LICENSE
OutputDir={#OutputDir}
OutputBaseFilename=Jenga-{#AppVersion}-windows-x64-Setup
VersionInfoVersion={#AppVersion}.0
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
SetupLogging=yes
ChangesAssociations=yes

[Files]
Source: "{#PayloadDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Icons]
Name: "{group}\Jenga"; Filename: "{app}\Jenga.exe"
Name: "{group}\Connect Assistant"; Filename: "{app}\Jenga.exe"; Parameters: "--connect-assistant"
Name: "{autodesktop}\Jenga"; Filename: "{app}\Jenga.exe"; Tasks: desktopicon

[Registry]
; Add an Open With handler without taking KiCad's default file association.
Root: HKCU; Subkey: "Software\Classes\Applications\Jenga.exe\shell\open\command"; ValueType: string; ValueData: """{app}\Jenga.exe"" ""%1"""; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\Jenga.exe\SupportedTypes"; ValueType: string; ValueName: ".kicad_sch"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\Applications\Jenga.exe\SupportedTypes"; ValueType: string; ValueName: ".kicad_pro"; ValueData: ""

[Run]
Filename: "{app}\Jenga.exe"; Description: "Open Jenga"; Flags: nowait postinstall skipifsilent

[Code]
function InitializeSetup(): Boolean;
var
  Installed: String;
  InstalledVersion, NewVersion: Int64;
begin
  Result := True;
  if RegQueryStringValue(HKCU,
    'Software\Microsoft\Windows\CurrentVersion\Uninstall\{786CEB53-E382-4F02-97AB-706854377AC4}_is1',
    'DisplayVersion', Installed) then
  begin
    if StrToVersion(Installed, InstalledVersion) and StrToVersion('{#AppVersion}', NewVersion) then
      if ComparePackedVersion(InstalledVersion, NewVersion) > 0 then
      begin
        Log('Downgrade rejected: installed version ' + Installed);
        if not WizardSilent then
          MsgBox('A newer Jenga version (' + Installed + ') is already installed. This installer will not downgrade it.', mbInformation, MB_OK);
        Result := False;
      end;
  end;
end;
