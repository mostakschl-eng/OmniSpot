[Setup]
AppId=7954d19d-b9c7-42b4-882b-0c03e632e75f
AppName=OmniSpot
AppVersion={#AppVersion}
AppVerName=OmniSpot {#AppVersion}
DefaultDirName={autopf}\OmniSpot
DefaultGroupName=OmniSpot
UninstallDisplayIcon={app}\OmniSpot.exe
SetupIconFile=..\desktop\assets\app-switcher.ico
Compression=lzma2
SolidCompression=yes
OutputDir=..\target\installer
OutputBaseFilename=OmniSpot Rust Installer
PrivilegesRequired=lowest
AppMutex=OmniSpotMutex
ArchitecturesAllowed=x64compatible
CloseApplications=yes
RestartApplications=yes

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#RustExe}"; DestDir: "{app}"; DestName: "OmniSpot.exe"; Flags: ignoreversion

[Dirs]
Name: "{userappdata}\OmniSpot"; Flags: uninsneveruninstall
Name: "{localappdata}\OmniSpot"; Flags: uninsneveruninstall

[UninstallDelete]
Type: files; Name: "{userstartup}\OmniSpot.lnk"

[Icons]
Name: "{group}\OmniSpot"; Filename: "{app}\OmniSpot.exe"
Name: "{commondesktop}\OmniSpot"; Filename: "{app}\OmniSpot.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\OmniSpot.exe"; Description: "{cm:LaunchProgram,OmniSpot}"; Flags: nowait postinstall skipifsilent

[Code]
procedure CurUninstallStepChanged(UninstallStep: TUninstallStep);
var
  RoamingDir: String;
  LocalDir: String;
begin
  if UninstallStep = usPostUninstall then
  begin
    RoamingDir := ExpandConstant('{userappdata}\OmniSpot');
    LocalDir := ExpandConstant('{localappdata}\OmniSpot');

    if not UninstallSilent and (DirExists(RoamingDir) or DirExists(LocalDir)) then
    begin
      if MsgBox('Do you want to delete all user data (settings and logs)?' #13#10 #13#10 +
                'This will permanently remove your configurations.',
                mbConfirmation, MB_YESNO) = idYes then
      begin
        if DirExists(RoamingDir) then
        begin
          Log('Removing Roaming data at: ' + RoamingDir);
          DelTree(RoamingDir, True, True, True);
        end;

        if DirExists(LocalDir) then
        begin
          Log('Removing Local data at: ' + LocalDir);
          DelTree(LocalDir, True, True, True);
        end;
      end;
    end;
  end;
end;
