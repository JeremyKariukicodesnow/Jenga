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
DisableDirPage=auto
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
Root: HKCU; Subkey: "Software\Jenga"; ValueType: string; ValueName: "InstallDir"; ValueData: "{app}"; Flags: uninsdeletevalue
; Add an Open With handler without taking KiCad's default file association.
Root: HKCU; Subkey: "Software\Classes\Applications\Jenga.exe\shell\open\command"; ValueType: string; ValueData: """{app}\Jenga.exe"" ""%1"""; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\Jenga.exe\SupportedTypes"; ValueType: string; ValueName: ".kicad_sch"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\Applications\Jenga.exe\SupportedTypes"; ValueType: string; ValueName: ".kicad_pro"; ValueData: ""

[Run]
Filename: "{app}\jenga-cli.exe"; Parameters: "repair-connections"; Flags: runhidden waituntilterminated
Filename: "{app}\Jenga.exe"; Description: "Open Jenga"; Flags: nowait postinstall skipifsilent

[Code]
var
  Maintenance: Boolean;
  MaintenanceAction: String;

function ShouldSkipPage(PageID: Integer): Boolean;
begin
  { Existing users enter maintenance directly: no repeated license or install choices. }
  Result := Maintenance and ((PageID = wpWelcome) or (PageID = wpLicense) or
    (PageID = wpSelectDir) or (PageID = wpSelectProgramGroup) or
    (PageID = wpSelectTasks));
end;

procedure InitializeWizard();
begin
  if Maintenance then
  begin
    WizardForm.Caption := MaintenanceAction + ' Jenga';
    WizardForm.ReadyLabel.Caption := MaintenanceAction + ' Jenga to version {#AppVersion}. ' +
      'Your projects, libraries and settings will be kept.';
    WizardForm.NextButton.Caption := MaintenanceAction;
  end;
end;

procedure CurPageChanged(CurPageID: Integer);
begin
  if Maintenance and (CurPageID = wpReady) then
    WizardForm.NextButton.Caption := MaintenanceAction;
end;

function InitializeSetup(): Boolean;
var
  Installed: String;
  InstalledVersion, NewVersion: Int64;
  Action, MessageText: String;
begin
  Result := True;
  if RegQueryStringValue(HKCU,
    'Software\Microsoft\Windows\CurrentVersion\Uninstall\{786CEB53-E382-4F02-97AB-706854377AC4}_is1',
    'DisplayVersion', Installed) then
  begin
    if StrToVersion(Installed, InstalledVersion) and StrToVersion('{#AppVersion}', NewVersion) then
    begin
      if ComparePackedVersion(InstalledVersion, NewVersion) > 0 then
        Action := 'Downgrade'
      else if ComparePackedVersion(InstalledVersion, NewVersion) = 0 then
        Action := 'Repair'
      else
        Action := 'Upgrade';
      Maintenance := True;
      MaintenanceAction := Action;
      MessageText := Action + ' Jenga ' + Installed + ' with version {#AppVersion}?' + #13#10 + #13#10 +
        'Your projects, personal libraries, settings and assistant configuration will be kept.';
      if Action = 'Downgrade' then
        MessageText := MessageText + #13#10 + 'Projects using newer features may not work in this older version. Keep a backup.';
      Log(Action + ': installed version ' + Installed);
      if WizardSilent then
      begin
        { An unattended downgrade needs an explicit command-line opt-in. }
        Result := (Action <> 'Downgrade') or (ExpandConstant('{param:ALLOWDOWNGRADE|0}') = '1');
      end
      { Interactive maintenance presents its action on the Ready page. }
    end;
  end;
end;
