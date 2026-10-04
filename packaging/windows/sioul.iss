; Inno Setup script for Sioul on Windows.
;
; Before: cargo build --release -p sioul-app -p sioul-cli (MSVC, Qt 6 from Qt's
; installer), then gather Qt beside the program:
;   windeployqt --release --qmldir crates\sioul-app\qml target\release\sioul-app.exe
; Then: iscc packaging\windows\sioul.iss
;
; Attachments are checked by Windows' own antivirus (AMSI, Microsoft Defender
; by default): nothing more to install. Passwords go to the Credential Manager.

#define Version "0.0.1"

[Setup]
AppId={{6A3F2C1E-5D4B-4E8A-9C7F-2B1D0E9F8A31}
AppName=Sioul
AppVersion={#Version}
AppPublisher=Aurélien Pierre
AppPublisherURL=https://github.com/aurelienpierre/sioul
DefaultDirName={autopf}\Sioul
DefaultGroupName=Sioul
LicenseFile=..\..\LICENSE
OutputBaseFilename=sioul-{#Version}-setup
Compression=lzma2
SolidCompression=yes
PrivilegesRequired=lowest
ArchitecturesInstallIn64BitMode=x64compatible
; Sioul's own icon (tools/make-icons.py): the installer's, and the shortcuts'.
SetupIconFile=sioul.ico
UninstallDisplayIcon={app}\sioul.ico

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "french"; MessagesFile: "compiler:Languages\French.isl"

[Files]
; The program, the command line, and everything windeployqt gathered beside them.
Source: "..\..\target\release\*"; DestDir: "{app}"; Flags: recursesubdirs ignoreversion; Excludes: "*.pdb,*.d,build\*,deps\*,incremental\*,.fingerprint\*,examples\*"
Source: "sioul.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Sioul"; Filename: "{app}\sioul-app.exe"; IconFilename: "{app}\sioul.ico"
Name: "{autodesktop}\Sioul"; Filename: "{app}\sioul-app.exe"; IconFilename: "{app}\sioul.ico"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Run]
Filename: "{app}\sioul-app.exe"; Description: "{cm:LaunchProgram,Sioul}"; Flags: nowait postinstall skipifsilent
