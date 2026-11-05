#ifndef AppVersion
  #define AppVersion "1.0.0"
#endif
#ifndef ProfileName
  #define ProfileName "folder"
#endif

[Setup]
AppId=7954d19d-b9c7-42b4-882b-0c03e632e75f
AppName=OmniSpot
AppVersion={#AppVersion}
AppVerName=OmniSpot {#AppVersion}
DefaultDirName={autopf}\OmniSpot
DefaultGroupName=OmniSpot
UninstallDisplayIcon={app}\OmniSpot.exe
OutputDir=Inno_Files
OutputBaseFilename=OmniSpot Rust {#ProfileName} Installer
Compression=lzma2
SolidCompression=yes
PrivilegesRequired=lowest
AppMutex=OmniSpotMutex
CloseApplications=yes
RestartApplications=yes
LicenseFile=package\LICENSE.md
SetupIconFile=package\app-switcher.ico

[Files]
Source: "package\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Dirs]
Name: "{userappdata}\OmniSpot"; Flags: uninsneveruninstall
Name: "{localappdata}\OmniSpot"; Flags: uninsneveruninstall

[UninstallDelete]
Type: files; Name: "{userstartup}\OmniSpot.lnk"

[Icons]
Name: "{group}\OmniSpot"; Filename: "{app}\OmniSpot.exe"; IconFilename: "{app}\app-switcher.ico"
Name: "{commondesktop}\OmniSpot"; Filename: "{app}\OmniSpot.exe"; IconFilename: "{app}\app-switcher.ico"

[Run]
Filename: "{app}\OmniSpot.exe"; Description: "Launch OmniSpot"; Flags: nowait postinstall skipifsilent
