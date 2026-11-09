; Inno Setup 6 Script for ReAmbiBox
#define MyAppName "ReAmbiBox"
#define MyAppVersion "1.0.0"
#define MyAppPublisher "ReAmbiBox"
#define MyAppURL "https://github.com/Deus/ReAmbiBox"
#define MyAppExeName "reambibox.exe"

[Setup]
AppId={{D37E6148-562F-43B5-A5A4-681961A6394F}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
OutputDir=..\target\dist
OutputBaseFilename=ReAmbiBox-Setup-v{#MyAppVersion}
SetupIconFile=..\crates\reambibox-app\resources\app.ico
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\{#MyAppExeName}
PrivilegesRequired=lowest
CloseApplications=force

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"

[CustomMessages]
english.AutostartGroup=Startup:
english.AutostartTask=Start {#MyAppName} with Windows (minimized to system tray)
english.WebControlPanelShortcut={#MyAppName} Control Center

russian.AutostartGroup=Автозагрузка:
russian.AutostartTask=Запускать ReAmbiBox вместе с Windows (в трее)
russian.WebControlPanelShortcut=Панель управления {#MyAppName}

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "autostart"; Description: "{cm:AutostartTask}"; GroupDescription: "{cm:AutostartGroup}"

[Files]
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{cm:WebControlPanelShortcut}"; Filename: "http://localhost:3637"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "{#MyAppName}"; ValueData: """{app}\{#MyAppExeName}"" --minimized"; Flags: uninsdeletevalue; Tasks: autostart

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

