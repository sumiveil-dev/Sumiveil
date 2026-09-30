# ビルド用ツール (Inno Setup) をプロジェクト内 tools\ にポータブル配置する。
# レジストリ登録・アンインストーラー・スタートメニュー・ファイル関連付けは作成しない。

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$tools = Join-Path $root "tools"
$dl = Join-Path $tools "downloads"
$inno = Join-Path $tools "innosetup7"   # 旧版 (6.x) の tools\innosetup は使わない (消すかどうかは利用者に任せる)
$version = "7.1.0"
$url = "https://github.com/jrsoftware/issrc/releases/download/is-$($version -replace '\.','_')/innosetup-$version-x64.exe"   # 7 から配布ファイル名に -x64 / -x86 が付く

$installed = Join-Path $env:ProgramFiles "Inno Setup 7\ISCC.exe"
if (Test-Path $installed) {
    Write-Host "Inno Setup 7 is already installed: $(Split-Path $installed)"
    exit 0
}
if (Test-Path (Join-Path $inno "ISCC.exe")) {
    Write-Host "Inno Setup already present: $inno"
    exit 0
}
New-Item -ItemType Directory -Force $dl | Out-Null
$exe = Join-Path $dl "innosetup-$version-x64.exe"
if (-not (Test-Path $exe)) {
    Write-Host "Downloading $url"
    $ProgressPreference = "SilentlyContinue"
    Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $exe
}

# 署名の確認
$sig = Get-AuthenticodeSignature $exe
if ($sig.Status -ne "Valid") {
    throw "Invalid signature on $exe : $($sig.Status)"
}
Write-Host "Signature OK: $($sig.SignerCertificate.Subject)"

# ポータブルモードでプロジェクト内に展開
$args = @("/PORTABLE=1", "/CURRENTUSER", "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/NOICONS", "/TASKS=`"`"", "/DIR=`"$inno`"")
$p = Start-Process -FilePath $exe -ArgumentList $args -Wait -PassThru
if ($p.ExitCode -ne 0) { throw "Inno Setup install failed: exit $($p.ExitCode)" }
if (-not (Test-Path (Join-Path $inno "ISCC.exe"))) { throw "ISCC.exe not found after install" }
Write-Host "Inno Setup $version installed to $inno"
