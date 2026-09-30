# 形態素解析用の辞書 (Lindera 形式の IPADIC) を target\dict\ipadic に作る。
# ソース (mecab-ipadic, EUC-JP) が data\raw\mecab-ipadic に無ければ GitHub から取得する (開発時のみ)。

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
. (Join-Path $PSScriptRoot "env.ps1")

$src = Join-Path $root "data\raw\mecab-ipadic"
if (-not (Test-Path (Join-Path $src "matrix.def"))) {
    Write-Host "Downloading mecab-ipadic source from github.com/taku910/mecab"
    New-Item -ItemType Directory -Force $src | Out-Null
    $list = Invoke-RestMethod -UseBasicParsing -Uri "https://api.github.com/repos/taku910/mecab/contents/mecab-ipadic" -Headers @{ "User-Agent" = "sumiveil-dev" }
    $names = @("matrix.def", "char.def", "unk.def", "dicrc", "COPYING", "left-id.def", "right-id.def", "pos-id.def", "rewrite.def", "feature.def")
    $ProgressPreference = "SilentlyContinue"
    foreach ($f in $list | Where-Object { $_.type -eq "file" -and ($_.name -like "*.csv" -or $names -contains $_.name) }) {
        Invoke-WebRequest -UseBasicParsing -Uri $f.download_url -OutFile (Join-Path $src $f.name)
    }
}

cargo run --release -q -p sumiveil-core --example build-morph-dict -- $src (Join-Path $root "target\dict\ipadic")
if ($LASTEXITCODE -ne 0) { throw "dictionary build failed" }
