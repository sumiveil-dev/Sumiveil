# コマンドライン (`sumiveil`)

`sumiveil.exe` はコンソール用、`sumiveil-gui.exe` は GUI 用の実行ファイルです (同じ検出エンジンを使います)。
ヘルプ: `sumiveil --help`、`sumiveil <サブコマンド> --help` (表示言語は設定または `SUMIVEIL_LANG=ja|en`)。

## 入力

| 指定 | 内容 |
|---|---|
| `sumiveil file.txt` | ファイル |
| `sumiveil a.txt b.log` | 複数ファイル (`--out-dir` または接尾辞付きで横に出力) |
| `sumiveil -r folder` | フォルダ (サブフォルダ含む。対象は `--include` / 設定の `batch.include`) |
| `type file \| sumiveil` / `sumiveil -` | 標準入力 (パイプ) |
| `sumiveil -t "テキスト"` | 文字列を直接 |
| `sumiveil --from-clipboard` | クリップボード |

文字コードは自動判定 (`--encoding sjis` 等で指定可)。ファイル出力は入力と同じ文字コードで書き出します (`--output-encoding` で変更可)。標準出力は UTF-8 です。

### 対応しているファイル形式

形式は拡張子で判定します。

| 形式 | ファイルへの出力 (`-o` / `--out-dir`) |
|---|---|
| テキスト (`.txt` `.log` `.csv` `.json` `.md` `.har` ソースコードなど) | 同じ形式・同じ文字コード |
| メール `.eml` | 件名・宛先・本文 (ISO-2022-JP / Base64 なども解読) をマスクした `.eml` (UTF-8)。添付ファイルと経路情報のヘッダーは取り除く |
| Word / Excel / PowerPoint (`.docx` `.xlsx` `.pptx`) | 書式を保った同じ形式。本文・表のセル・コメント・ヘッダー / フッター・ノート・リンク先を処理 |
| PDF / Outlook の `.msg` | マスクしたテキスト (`report.pdf` → `report.pdf.txt`) |

- 標準出力に出すとき (`-o` なし) は、どの形式もマスクしたテキストを出します。
- Office 文書のプロパティ (作成者・最終更新者・会社名・コメントの作成者など) は `--properties mask|clear|keep` で扱いを指定します。省略時は設定の `files.properties` に従い、`"ask"` (既定) のときは `clear` (消す) として扱って通知します。
- `detect` / `check` と JSON 出力では、Office 文書などの検出位置を `顧客一覧!B3`・`スライド 2`・`件名`・`3 ページ` のような場所 (`location`) でも示します。

## 出力

| 指定 | 内容 |
|---|---|
| (なし) | 標準出力 |
| `-o out.txt` | ファイル (入力 1 つのとき) |
| `--out-dir dir` | フォルダ (フォルダ構成を維持) |
| (複数入力で `--out-dir` なし) | 入力の横に `report.masked.txt` のように出力 (`--suffix` で変更) |
| `-C` / `--to-clipboard` | クリップボードにもコピー |
| `-f json` / `--format json` | JSON (検出位置・種類・置換後の値・統計) |
| `--diff` | 変更された行だけを差分形式で表示 |
| `--report r.csv` / `r.json` | 集計レポート |
| `--stats` | 検出件数を標準エラーに表示 |
| `--properties clear` | Office 文書のプロパティ (作成者など) の扱い: `mask` / `clear` / `keep` |
| `-q` / `--quiet` | 標準エラーへの警告・集計を出さない |
| `--color auto` | 色付けの有無: `auto` / `always` / `never` |

入力ファイル自体を上書きすることはありません (出力先が入力と同じ場合はエラー)。

### JSON 出力の形式

```json
{
  "tool": "sumiveil", "version": "1.0.1", "source": "report.txt", "encoding": "Shift_JIS", "profile": "default",
  "masked": "……",
  "detections": [
    { "id": "email", "category": "contact", "label": "EMAIL", "name": "メールアドレス",
      "line": 3, "column": 5, "start": 42, "end": 61, "confidence": 0.95, "replacement": "<EMAIL_1>" }
  ],
  "stats": { "total": 1, "by_detector": { "email": 1 }, "by_category": { "contact": 1 } }
}
```

元の値は既定では含めません (`--include-original` で追加)。`--no-masked` でマスク後テキストを省略できます。
Office 文書・メール・PDF では、検出ごとに `"location": "顧客一覧!B3"` のような場所と、文書全体の `warnings` (取り除いた添付ファイルなど) が加わります。

## 検出の調整

| 指定 | 内容 |
|---|---|
| `-p llm` / `--profile llm` | プロファイル |
| `--enable date,money` | 検出器・カテゴリを有効化 |
| `--disable network` | 検出器・カテゴリを無効化 |
| `--only email,secret` | 指定したものだけ |
| `--template "[{label}]"` | マスク表示 (全検出器) |
| `--min-confidence 0.7` | 信頼度のしきい値 |
| `--separate-numbering` | 連番をファイルごとにリセット |
| `--line-buffered` | 標準入力を 1 行ずつ処理 (`tail -f` 等) |
| `--include "*.log"` / `--exclude "*.min.js"` | フォルダ内で対象にする・除外するファイル名 |
| `--encoding sjis` / `--output-encoding utf-8` | 入力・出力の文字コード (既定は自動判定・入力と同じ) |
| `--config path` | 設定ファイルを指定 (環境変数 `SUMIVEIL_CONFIG` でも可) |

## サブコマンド

| コマンド | 内容 |
|---|---|
| `sumiveil mask ...` | マスク (既定。省略可) |
| `sumiveil detect ...` | マスクせず検出結果を一覧表示 (`--show-values` で値も表示) |
| `sumiveil check ...` | 検出があれば終了コード 1 (CI・pre-commit 用) |
| `sumiveil list-detectors [--json]` | 検出器とカテゴリの一覧 (現在の有効/無効付き) |
| `sumiveil config path` | 設定ファイルの場所 |
| `sumiveil config show [--resolved]` | 設定ファイル / 合成後の設定を表示 |
| `sumiveil config get <key>` | 値を取得 (未設定なら既定値) |
| `sumiveil config set <key> <value>` | 値を設定 (例: `masking.template '"[{label}]"'`) |
| `sumiveil config unset <key>` | 既定値に戻す |
| `sumiveil config enable/disable <id>...` | 検出器・カテゴリの切り替え |
| `sumiveil config init [--portable] [--force]` | コメント付きの既定設定を作成 (`--force` で上書き) |
| `sumiveil config export <file> [--resolved]` / `import <file> [--merge]` | 設定の共有 (`--resolved` は既定値も含めて書き出す) |
| `sumiveil config validate [file]` | 設定ファイルの検証 |
| `sumiveil config edit` | エディタで開く (`VISUAL` / `EDITOR`、既定はメモ帳) |
| `sumiveil config profiles` / `use <name>` | プロファイルの一覧・切り替え |
| `sumiveil diagnose` | 診断レポートのファイルを作る (設定ファイルと同じ場所の `diagnostics` フォルダ。送信はしない。入力した文章・辞書やルールに登録した語は含めない) |
| `sumiveil gui [file]` | GUI を開く |
| `sumiveil completions powershell` | シェル補完スクリプト |

`config set/enable/disable` に `--profile <name>` を付けると、そのプロファイル内の設定を変更します。
変更後の設定が読み込めない場合は保存しません。

## 終了コード

| コード | 意味 |
|---|---|
| 0 | 成功 (`check` では検出なし) |
| 1 | `check` で検出あり / `config validate` で問題あり |
| 2 | エラー (ファイルが無い・引数が不正など) |

## 使用例

```powershell
# Git の pre-commit で .env や設定ファイルに秘密情報が入っていないか確認
sumiveil check -r . --include "*.env" --include "*.yaml" --only secret -q

# ログを共有用に整形 (形式を保ったマスク)
sumiveil -p log_share -r .\logs --out-dir .\share --report .\share\report.csv

# 生成 AI に貼る前にクリップボードの内容をマスク
sumiveil --from-clipboard -C -q
```
