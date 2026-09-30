# Sumiveil 開発環境の分離設定
# 使い方: . .\scripts\env.ps1   (ドットソースで読み込む)
# crates.io のダウンロードキャッシュ等をプロジェクト内 .cargo-home に閉じ込める。
# rustup のツールチェーン自体は既存のものを使い、変更しない。

$ProjectRoot = Split-Path -Parent $PSScriptRoot
$env:CARGO_HOME = Join-Path $ProjectRoot ".cargo-home"
$env:CARGO_TARGET_DIR = Join-Path $ProjectRoot "target"

# rustup プロキシ (cargo.exe / rustc.exe) は既存の場所から使う
$rustupBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (($env:PATH -split ';') -notcontains $rustupBin) {
    $env:PATH = "$rustupBin;$env:PATH"
}

# Inno Setup 7 (プロジェクト内ポータブル配置。無ければ通常のインストール先)
$isccDir = @((Join-Path $ProjectRoot "tools\innosetup7"), (Join-Path $env:ProgramFiles "Inno Setup 7")) | Where-Object { Test-Path (Join-Path $_ "ISCC.exe") } | Select-Object -First 1
if (-not $isccDir) { $isccDir = Join-Path $ProjectRoot "tools\innosetup7" }
if ((Test-Path $isccDir) -and (($env:PATH -split ';') -notcontains $isccDir)) {
    $env:PATH = "$isccDir;$env:PATH"
}

Write-Host "[sumiveil env] CARGO_HOME = $env:CARGO_HOME"
