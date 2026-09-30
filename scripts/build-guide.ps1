# 利用ガイド (docs\guide\guide.html) を PDF に変換する。Windows 標準の Microsoft Edge (ヘッドレス) を使う。
# ユーザーのブラウザー設定には触れないよう、一時フォルダのプロファイルで実行する。
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$html = Join-Path $root "docs\guide\guide.html"
$out = Join-Path $root "docs\Sumiveil-UserGuide-ja.pdf"

$edge = @(
    (Join-Path ${env:ProgramFiles(x86)} "Microsoft\Edge\Application\msedge.exe"),
    (Join-Path $env:ProgramFiles "Microsoft\Edge\Application\msedge.exe")
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $edge) { throw "Microsoft Edge not found" }

$profileDir = Join-Path $env:TEMP "sumiveil-guide-edge"
Remove-Item $out -ErrorAction SilentlyContinue
$url = "file:///" + ($html -replace '\\', '/')
$args = @("--headless=new", "--disable-gpu", "--no-first-run", "--no-pdf-header-footer", "--user-data-dir=`"$profileDir`"", "--print-to-pdf=`"$out`"", "`"$url`"")
$p = Start-Process -FilePath $edge -ArgumentList $args -Wait -PassThru -WindowStyle Hidden
Start-Sleep -Milliseconds 500
Remove-Item -Recurse -Force $profileDir -ErrorAction SilentlyContinue
if (-not (Test-Path $out)) { throw "PDF was not created (edge exit $($p.ExitCode))" }
Write-Host ("created {0} ({1:N1} MB)" -f $out, ((Get-Item $out).Length / 1MB))
