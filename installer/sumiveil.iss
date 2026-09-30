; ============================================================================
;  Sumiveil インストーラー (Inno Setup 7.1 以降)
;  ビルド: scripts\package.ps1  (ISCC /DAppVersion=x.y.z installer\sumiveil.iss)
; ============================================================================

#ifndef AppVersion
  #define AppVersion "1.0.0"
#endif
#define AppName "Sumiveil"
#define AppPublisher "sumiveil-dev"
#define AppExe "sumiveil-gui.exe"
#define CliExe "sumiveil.exe"
#define AppUserModelID "Sumiveil.Sumiveil"
; 変えてはいけない (変えると上書き更新ができなくなる)
#define AppGuid "580C8692-CEF5-4400-AB31-1443A0EB7620"

[Setup]
AppId={{{#AppGuid}}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
VersionInfoVersion={#AppVersion}
VersionInfoProductName={#AppName}
VersionInfoDescription={#AppName} Setup
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
; ようこそ画面は上書き更新・再インストールのときだけ出す ([Code] の ShouldSkipPage)
DisableWelcomePage=no
; 現在のユーザーのみ / 全ユーザー を起動時に選択 (コマンドラインの /CURRENTUSER /ALLUSERS でも指定可)
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog commandline
UsePreviousPrivileges=yes
; 64 ビット版の Setup で作る (Inno Setup 7 から)
SetupArchitecture=x64
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
; Windows のライト/ダーク設定に追従するテーマ
WizardStyle=modern dynamic windows11
; ようこそ・完了画面の画像も自作 (Inno Setup 付属の画像は使わない)
WizardImageFile=..\assets\wizard-large.bmp
WizardImageFileDynamicDark=..\assets\wizard-large-dark.bmp
WizardSmallImageFile=..\assets\wizard-small.bmp
WizardSmallImageFileDynamicDark=..\assets\wizard-small-dark.bmp
SetupIconFile=..\assets\sumiveil.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
OutputDir=..\dist
OutputBaseFilename=Sumiveil-Setup-{#AppVersion}-x64
Compression=lzma2/ultra64
SolidCompression=yes
ChangesEnvironment=yes
ChangesAssociations=yes
CloseApplications=yes
RestartApplications=no
ShowLanguageDialog=auto
LicenseFile=..\LICENSE.txt

[Languages]
Name: "ja"; MessagesFile: "compiler:Languages\Japanese.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[LangOptions]
; 既定の Segoe UI には日本語の字形が無く、Windows のフォント代替に頼ると
; Windows サンドボックス等の最小構成で文字化けする。日本語は Windows 10/11 に標準で入っている
; Yu Gothic UI を明示する (無い環境では [Code] の InitializeWizard で代替フォントに切り替える)。
ja.DialogFontName=Yu Gothic UI
ja.DialogFontSize=9
ja.WelcomeFontName=Yu Gothic UI
ja.WelcomeFontSize=14

[CustomMessages]
ja.AddToPath=コマンドラインから sumiveil を使えるように PATH に追加する
en.AddToPath=Add to PATH (use "sumiveil" from the command line)
ja.SystemIntegration=Windows との連携:
en.SystemIntegration=Windows integration:
ja.StartWithWindows=Windows の起動時にトレイで常駐させる (ホットキーを常に使えるように)
en.StartWithWindows=Start in the tray when Windows starts (hotkey always available)
ja.SendTo=エクスプローラーの「送る」メニューに追加する
en.SendTo=Add to the Explorer "Send to" menu
ja.ContextMenu=ファイルの右クリックメニューに「Sumiveil で開く」を追加する
en.ContextMenu=Add "Open with Sumiveil" to the file context menu
ja.OpenWith=Sumiveil で開く
en.OpenWith=Open with Sumiveil
ja.DeleteSettings=Sumiveil の設定ファイル (%APPDATA%\Sumiveil) も削除しますか?%n%nチームで共有している設定などがある場合は「いいえ」を選んでください。
en.DeleteSettings=Also delete Sumiveil settings (%APPDATA%\Sumiveil)?%n%nChoose "No" if you want to keep your settings.
ja.CommandLine=Sumiveil コマンドライン
en.CommandLine=Sumiveil command line
ja.GuideName=利用ガイド
en.GuideName=User Guide (Japanese)
ja.TypeCustom=カスタム
en.TypeCustom=Custom
ja.UpgradeTitle=Sumiveil の更新
en.UpgradeTitle=Update Sumiveil
ja.UpgradeText=Sumiveil %1 がインストールされています。%2 に更新します。%n%nインストール先・設定・前回選んだオプションはそのまま引き継がれます。オプションは次の画面で変えることもできます。%n%n続行するには「次へ」をクリックしてください。
en.UpgradeText=Sumiveil %1 is installed. It will be updated to %2.%n%nThe install location, settings and the options you chose last time are kept. You can change the options on the next pages.%n%nClick Next to continue.
ja.ReinstallTitle=Sumiveil の再インストール
en.ReinstallTitle=Reinstall Sumiveil
ja.ReinstallText=Sumiveil %1 はすでにインストールされています。同じバージョンを入れ直します (修復)。%n%nインストール先・設定・前回選んだオプションはそのまま引き継がれます。%n%n続行するには「次へ」をクリックしてください。
en.ReinstallText=Sumiveil %1 is already installed. It will be installed again (repair).%n%nThe install location, settings and the options you chose last time are kept.%n%nClick Next to continue.
ja.UpgradeMemo=更新:
en.UpgradeMemo=Update:
ja.MainComponent=本体・コマンドライン
en.MainComponent=application and command line
ja.MorphComponent=形態素解析辞書 (敬称のない人名・地名の検出精度を高める。約 45 MB)
en.MorphComponent=Morphology dictionary (better detection of names/places; about 45 MB)
ja.AppRunning=Sumiveil が実行中です (タスクトレイに常駐している場合があります)。%n%nSumiveil を終了して続行しますか?
en.AppRunning=Sumiveil is running (it may be in the system tray).%n%nClose Sumiveil and continue?

[Types]
; 形は「本体だけ」か「辞書も入れる」の 2 通りしかないので、型は 1 つだけにして、形態素解析辞書はチェックボックスで選ばせる。
; Inno Setup は、チェックを手で変えると必ず iscustom の型に切り替える。型が複数あると、同じ内容でも「完全」と「カスタム」に分かれて見えてしまう。
; iscustom の型が 1 つだけなら、型の選択欄は自動で隠れ、前回の選択も Inno Setup が自動で引き継ぐ。
Name: "custom"; Description: "{cm:TypeCustom}"; Flags: iscustom

[Components]
Name: "main"; Description: "Sumiveil ({cm:MainComponent})"; Types: custom; Flags: fixed
Name: "morph"; Description: "{cm:MorphComponent}"; ExtraDiskSpaceRequired: 0

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "addtopath"; Description: "{cm:AddToPath}"; GroupDescription: "{cm:SystemIntegration}"
Name: "sendto"; Description: "{cm:SendTo}"; GroupDescription: "{cm:SystemIntegration}"
Name: "contextmenu"; Description: "{cm:ContextMenu}"; GroupDescription: "{cm:SystemIntegration}"; Flags: unchecked
Name: "startup"; Description: "{cm:StartWithWindows}"; GroupDescription: "{cm:SystemIntegration}"; Flags: unchecked

[Files]
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\{#CliExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\config\default.toml"; DestDir: "{app}"; DestName: "sumiveil.default.toml"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\docs\*"; DestDir: "{app}\docs"; Flags: ignoreversion recursesubdirs skipifsourcedoesntexist
Source: "..\LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\THIRD-PARTY-NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\crates\sumiveil-core\data\IPADIC-LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\dict\ipadic\*"; DestDir: "{app}\dict\ipadic"; Components: morph; Flags: ignoreversion skipifsourcedoesntexist

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"; AppUserModelID: "{#AppUserModelID}"; Comment: "Mask personal and confidential information"
Name: "{autoprograms}\{#AppName} {cm:GuideName}"; Filename: "{app}\docs\Sumiveil-UserGuide-ja.pdf"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; AppUserModelID: "{#AppUserModelID}"; Tasks: desktopicon
Name: "{autostartup}\{#AppName}"; Filename: "{app}\{#AppExe}"; Parameters: "--tray"; AppUserModelID: "{#AppUserModelID}"; Tasks: startup
Name: "{usersendto}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: sendto

[Registry]
; Win+R で sumiveil-gui と入力して起動できるように
Root: HKA; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\{#AppExe}"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExe}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\{#CliExe}"; ValueType: string; ValueName: ""; ValueData: "{app}\{#CliExe}"; Flags: uninsdeletekey
; 右クリックメニュー (すべてのファイル)
Root: HKA; Subkey: "Software\Classes\*\shell\Sumiveil"; ValueType: string; ValueName: ""; ValueData: "{cm:OpenWith}"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\*\shell\Sumiveil"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\{#AppExe}"",0"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\*\shell\Sumiveil\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: contextmenu

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: files; Name: "{app}\sumiveil.toml.tmp"

[Code]
const
  UserEnvKey = 'Environment';
  SystemEnvKey = 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment';
  AppMutexName = 'Local\Sumiveil.SingleInstance';

// 実行中の Sumiveil (トレイ常駐を含む) を終了させる。対話モードでは確認する。
function CloseRunningApp(Silent: Boolean): Boolean;
var
  RC: Integer;
begin
  Result := True;
  if not CheckForMutexes(AppMutexName) then
    Exit;
  if not Silent then
    if MsgBox(CustomMessage('AppRunning'), mbConfirmation, MB_YESNO) <> IDYES then
    begin
      Result := False;
      Exit;
    end;
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM {#AppExe}', '', SW_HIDE, ewWaitUntilTerminated, RC);
  Sleep(700);
end;

// 既にインストールされている Sumiveil のバージョン (無ければ空)。InitializeWizard で読む
var
  PrevVersion: String;

function UninstallKey: String;
begin
  Result := 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{{#AppGuid}}_is1';
end;

function IsUpgrade: Boolean;
begin
  Result := PrevVersion <> '';
end;

// 上書き更新・再インストールのときは、ようこそ画面をその案内に差し替える
procedure SetupUpgradeWelcome;
var
  Root: Integer;
begin
  PrevVersion := '';
  if WizardForm.PrevAppDir = '' then
    Exit;
  if IsAdminInstallMode then
    Root := HKEY_LOCAL_MACHINE
  else
    Root := HKEY_CURRENT_USER;
  if not RegQueryStringValue(Root, UninstallKey, 'DisplayVersion', PrevVersion) or (PrevVersion = '') then
    PrevVersion := '?';
  if PrevVersion = '{#AppVersion}' then
  begin
    WizardForm.WelcomeLabel1.Caption := CustomMessage('ReinstallTitle');
    WizardForm.WelcomeLabel2.Caption := FmtMessage(CustomMessage('ReinstallText'), [PrevVersion]);
  end else begin
    WizardForm.WelcomeLabel1.Caption := CustomMessage('UpgradeTitle');
    WizardForm.WelcomeLabel2.Caption := FmtMessage(CustomMessage('UpgradeText'), [PrevVersion, '{#AppVersion}']);
  end;
end;

// 新規インストールではようこそ画面を省く。上書き更新・再インストールでは使用許諾を省く (同意済みのため)
function ShouldSkipPage(PageID: Integer): Boolean;
begin
  Result := False;
  if PageID = wpWelcome then
    Result := not IsUpgrade
  else if PageID = wpLicense then
    Result := IsUpgrade;
end;

// 準備完了画面の一覧の先頭に「更新: 旧 → 新」を出す
function UpdateReadyMemo(Space, NewLine, MemoUserInfoInfo, MemoDirInfo, MemoTypeInfo, MemoComponentsInfo, MemoGroupInfo, MemoTasksInfo: String): String;
begin
  Result := '';
  if IsUpgrade then
    Result := CustomMessage('UpgradeMemo') + NewLine + Space + PrevVersion + ' -> {#AppVersion}' + NewLine + NewLine;
  if MemoDirInfo <> '' then
    Result := Result + MemoDirInfo + NewLine + NewLine;
  if MemoComponentsInfo <> '' then
    Result := Result + MemoComponentsInfo + NewLine + NewLine;
  if MemoGroupInfo <> '' then
    Result := Result + MemoGroupInfo + NewLine + NewLine;
  if MemoTasksInfo <> '' then
    Result := Result + MemoTasksInfo;
end;

// 日本語表示で Yu Gothic UI が無い環境では、標準で入っている別の日本語フォントに切り替える
procedure InitializeWizard;
var
  Fallback: String;
begin
  SetupUpgradeWelcome;
  if (ActiveLanguage = 'ja') and not FontExists('Yu Gothic UI') then
  begin
    if FontExists('Meiryo UI') then
      Fallback := 'Meiryo UI'
    else if FontExists('MS UI Gothic') then
      Fallback := 'MS UI Gothic'
    else
      Fallback := '';
    if Fallback <> '' then
      WizardForm.Font.Name := Fallback;
  end;
end;

function InitializeSetup(): Boolean;
begin
  Result := CloseRunningApp(WizardSilent);
end;

function InitializeUninstall(): Boolean;
begin
  Result := CloseRunningApp(UninstallSilent);
end;

function EnvRoot: Integer;
begin
  if IsAdminInstallMode then
    Result := HKEY_LOCAL_MACHINE
  else
    Result := HKEY_CURRENT_USER;
end;

function EnvKey: String;
begin
  if IsAdminInstallMode then
    Result := SystemEnvKey
  else
    Result := UserEnvKey;
end;

function SamePath(A, B: String): Boolean;
begin
  Result := CompareText(RemoveBackslashUnlessRoot(Trim(A)), RemoveBackslashUnlessRoot(Trim(B))) = 0;
end;

function PathHas(Paths, Dir: String): Boolean;
var
  Rest, Item: String;
  I: Integer;
begin
  Result := False;
  Rest := Paths + ';';
  while Length(Rest) > 0 do
  begin
    I := Pos(';', Rest);
    Item := Copy(Rest, 1, I - 1);
    Rest := Copy(Rest, I + 1, Length(Rest));
    if (Item <> '') and SamePath(Item, Dir) then
    begin
      Result := True;
      Exit;
    end;
  end;
end;

procedure AddToPath(Dir: String);
var
  Paths: String;
begin
  if not RegQueryStringValue(EnvRoot, EnvKey, 'Path', Paths) then
    Paths := '';
  if PathHas(Paths, Dir) then
    Exit;
  if (Paths <> '') and (Copy(Paths, Length(Paths), 1) <> ';') then
    Paths := Paths + ';';
  Paths := Paths + Dir;
  RegWriteExpandStringValue(EnvRoot, EnvKey, 'Path', Paths);
end;

procedure RemoveFromPath(Dir: String);
var
  Paths, Rest, Item, NewPaths: String;
  I: Integer;
begin
  if not RegQueryStringValue(EnvRoot, EnvKey, 'Path', Paths) then
    Exit;
  Rest := Paths + ';';
  NewPaths := '';
  while Length(Rest) > 0 do
  begin
    I := Pos(';', Rest);
    Item := Copy(Rest, 1, I - 1);
    Rest := Copy(Rest, I + 1, Length(Rest));
    if (Item <> '') and not SamePath(Item, Dir) then
    begin
      if NewPaths <> '' then
        NewPaths := NewPaths + ';';
      NewPaths := NewPaths + Item;
    end;
  end;
  if NewPaths <> Paths then
    RegWriteExpandStringValue(EnvRoot, EnvKey, 'Path', NewPaths);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    if WizardIsTaskSelected('addtopath') then
      AddToPath(ExpandConstant('{app}'))
    else
      RemoveFromPath(ExpandConstant('{app}'));
  end;
end;

// Path が Base フォルダの中にあれば、Base からの相対パスを Rel に入れて True を返す
function RelativeTo(Base, Path: String; var Rel: String): Boolean;
begin
  Base := AddBackslash(Base);
  Result := CompareText(Copy(Path, 1, Length(Base)), Base) = 0;
  if Result then
    Rel := Copy(Path, Length(Base) + 1, Length(Path));
  Result := Result and (Rel <> '');
end;

procedure DeleteUserKey(Key: String);
begin
  if RegKeyExists(HKEY_CURRENT_USER, Key) then
    RegDeleteKeyIncludingSubkeys(HKEY_CURRENT_USER, Key);
end;

// Windows がスタートメニューや通知のために Sumiveil 専用に作るキーを消す (現在のユーザー分のみ)。
// タイル情報は "W~<AppUserModelID>" か "W~<既知フォルダーの GUID>\<インストール先の相対パス>" に作られる。
// AppListBackup の値は全アプリ共有のバックアップなので触らない
procedure DeleteShellTraces;
var
  Tiles, App, Rel: String;
begin
  Tiles := 'Software\Microsoft\Windows\CurrentVersion\Start\TileProperties\';
  DeleteUserKey(Tiles + 'W~{#AppUserModelID}');
  App := RemoveBackslashUnlessRoot(ExpandConstant('{app}'));
  if RelativeTo(ExpandConstant('{userpf}'), App, Rel) then
    DeleteUserKey(Tiles + 'W~{5CD7AEE2-2219-4A67-B85D-6C9CE15660CB}\' + Rel)
  else if RelativeTo(ExpandConstant('{commonpf64}'), App, Rel) then
    DeleteUserKey(Tiles + 'W~{6D809377-6AF0-444B-8957-A3773F02200E}\' + Rel)
  else if RelativeTo(ExpandConstant('{commonpf32}'), App, Rel) then
    DeleteUserKey(Tiles + 'W~{7C5A40EF-A0FB-4BFC-874A-C0F2E0B9FA8E}\' + Rel);
  DeleteUserKey('Software\Microsoft\Windows\CurrentVersion\Notifications\Settings\{#AppUserModelID}');
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    RemoveFromPath(ExpandConstant('{app}'));
    DeleteShellTraces;
    if not UninstallSilent then
      if MsgBox(CustomMessage('DeleteSettings'), mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
        DelTree(ExpandConstant('{userappdata}\Sumiveil'), True, True, True);
  end;
end;
