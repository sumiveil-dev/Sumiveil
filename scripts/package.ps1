# リリースビルド → インストーラー (Inno Setup) とポータブル ZIP を dist\ に作成する。
# 使い方: .\scripts\package.ps1 [-SkipTests] [-GuideKit]
#   -GuideKit: 利用ガイドの撮影キット (target\guide-kit) も作る。VM の共有フォルダに登録して run-capture.cmd を実行する

param([switch]$SkipTests, [switch]$GuideKit)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
. (Join-Path $PSScriptRoot "env.ps1")

# Inno Setup 7 の ISCC.exe を探す (プロジェクト内の tools\innosetup7 → 通常のインストール先)
function Find-Iscc([string]$Root) {
    $candidates = @(
        (Join-Path $Root "tools\innosetup7\ISCC.exe"),
        (Join-Path $env:ProgramFiles "Inno Setup 7\ISCC.exe"),
        (Join-Path ${env:ProgramFiles(x86)} "Inno Setup 7\ISCC.exe"),
        (Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 7\ISCC.exe")
    )
    $candidates | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
}

# コピー直後の exe はウイルス対策ソフトの検査でロックされることがあるため再試行する
function Compress-WithRetry([string]$Path, [string]$Dest) {
    for ($i = 1; $i -le 10; $i++) {
        try {
            Compress-Archive -Path $Path -DestinationPath $Dest -Force -ErrorAction Stop
            return
        } catch {
            if ($i -eq 10) { throw }
            Start-Sleep -Seconds 2
        }
    }
}

$version = (Select-String -Path "Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
Write-Host "== Sumiveil $version"

if (-not $SkipTests) {
    Write-Host "== cargo test"
    cargo test --workspace --release -q
    if ($LASTEXITCODE -ne 0) { throw "tests failed" }
}

if (-not (Test-Path "assets\sumiveil.ico")) {
    cargo run -q -p sumiveil-gui --example make-icon
}

Write-Host "== cargo build --release"
cargo build --release -p sumiveil-cli -p sumiveil-gui
if ($LASTEXITCODE -ne 0) { throw "build failed" }

$about = Join-Path $env:CARGO_HOME "bin\cargo-about.exe"
if (Test-Path $about) {
    Write-Host "== third-party notices"
    cargo about generate about.hbs -o THIRD-PARTY-NOTICES.txt 2>$null
    if ($LASTEXITCODE -ne 0) { throw "cargo about failed (ライセンス表示を作れませんでした。cargo about generate about.hbs を直接実行して原因を確認してください)" }
}

# リリース版の依存物を使う (デバッグ版は num-traits などのビルドスクリプトを作り直すことになり、
# フォルダに ReadOnly 属性が付いている環境では autocfg が「書き込めない」と判定して失敗するため)
cargo run -q --release -p sumiveil-core --example gen-docs | Out-Null

Write-Host "== user guide (PDF)"
& (Join-Path $PSScriptRoot "build-guide.ps1")

if (-not (Test-Path "target\dict\ipadic\metadata.json")) {
    Write-Host "== morphology dictionary"
    & (Join-Path $PSScriptRoot "build-morph-dict.ps1")
}

$iscc = Find-Iscc $root
if (-not $iscc) {
    Write-Host "Inno Setup 7 not found. Install it, or run scripts\setup-tools.ps1 first."
} else {
    Write-Host "== installer"
    & $iscc /Q "/DAppVersion=$version" "installer\sumiveil.iss"
    if ($LASTEXITCODE -ne 0) { throw "ISCC failed" }
}

New-Item -ItemType Directory -Force "dist" | Out-Null
Copy-Item "docs\Sumiveil-UserGuide-ja.pdf" "dist\Sumiveil-$version-UserGuide-ja.pdf" -Force

Write-Host "== portable zip"
$stage = Join-Path $root "target\portable\Sumiveil"
Remove-Item -Recurse -Force (Split-Path $stage) -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item "target\release\sumiveil.exe", "target\release\sumiveil-gui.exe", "README.md", "LICENSE.txt", "crates\sumiveil-core\data\IPADIC-LICENSE.txt" $stage
if (Test-Path "THIRD-PARTY-NOTICES.txt") { Copy-Item "THIRD-PARTY-NOTICES.txt" $stage }
Copy-Item -Recurse "docs" (Join-Path $stage "docs")
# exe と同じフォルダの sumiveil.toml を使うポータブルモード
Copy-Item "config\default.toml" (Join-Path $stage "sumiveil.toml")
New-Item -ItemType Directory -Force "dist" | Out-Null
$zip = "dist\Sumiveil-$version-portable-x64.zip"
Remove-Item $zip -ErrorAction SilentlyContinue
Compress-WithRetry $stage $zip
# 形態素解析辞書は別 ZIP (展開して Sumiveil フォルダに重ねると dict\ipadic に入る)
if (Test-Path "target\dict\ipadic\metadata.json") {
    $dstage = Join-Path $root "target\portable-dict\Sumiveil\dict"
    Remove-Item -Recurse -Force (Join-Path $root "target\portable-dict") -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $dstage | Out-Null
    Copy-Item -Recurse "target\dict\ipadic" (Join-Path $dstage "ipadic")
    # IPADIC の使用条件により、辞書のコピーには必ず著作権表示と条件を添える
    Copy-Item "crates\sumiveil-core\data\IPADIC-LICENSE.txt" (Join-Path $root "target\portable-dict\Sumiveil")
    $dzip = "dist\Sumiveil-$version-morphology-dict.zip"
    Remove-Item $dzip -ErrorAction SilentlyContinue
    Compress-WithRetry (Join-Path $root "target\portable-dict\Sumiveil") $dzip
}

Get-ChildItem dist | Select-Object Name, @{n = "MB"; e = { [math]::Round($_.Length / 1MB, 1) } } | Format-Table -AutoSize

# 利用ガイドの撮影キット (VM で撮影する。この PC では撮影しない)
if ($GuideKit) {
    Write-Host "== guide kit"
    $capTarget = Join-Path $root "target\guide-capture"
    $prevTarget = $env:CARGO_TARGET_DIR
    $env:CARGO_TARGET_DIR = $capTarget
    cargo build --release -p sumiveil-gui --features guide-capture
    $code = $LASTEXITCODE
    $env:CARGO_TARGET_DIR = $prevTarget
    if ($code -ne 0) { throw "guide-capture build failed" }
    $setup = "dist\Sumiveil-Setup-$version-x64.exe"
    if (-not (Test-Path $setup)) { throw "$setup がありません (Inno Setup 7 を用意してから実行してください)" }
    $kit = Join-Path $root "target\guide-kit"
    Remove-Item -Recurse -Force $kit -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force (Join-Path $kit "app"), (Join-Path $kit "samples") | Out-Null
    Copy-Item (Join-Path $capTarget "release\sumiveil-gui.exe") (Join-Path $kit "app")
    Copy-Item $setup $kit
    Copy-Item -Recurse "docs\guide" (Join-Path $kit "guide")
    Copy-Item "assets\sumiveil.ico" $kit
    Copy-Item "tests\fixtures\sample-ja.txt", "crates\sumiveil-core\tests\fixtures\sample-ja.pdf" (Join-Path $kit "samples")
    Copy-Item "scripts\guide-kit\*" $kit
    $mb = [math]::Round(((Get-ChildItem -Recurse -File $kit | Measure-Object Length -Sum).Sum) / 1MB, 1)
    Write-Host "guide kit: $kit ($mb MB)"
}

