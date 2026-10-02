# 設定ファイル

形式は TOML です。書いていない項目は既定値が使われるので、変更したい項目だけ書けば十分です。
コメント付きの全項目は `sumiveil config init` で作成できます (内容は `config/default.toml` と同じ)。

## 場所 (優先順)

1. `--config <path>` (CLI)
2. 環境変数 `SUMIVEIL_CONFIG`
3. `sumiveil.exe` / `sumiveil-gui.exe` と同じフォルダの `sumiveil.toml` (ポータブルモード)
4. `%APPDATA%\Sumiveil\config.toml`

GUI・CLI・直接編集のどれで変更しても、実行中の GUI に自動で反映されます。GUI や `sumiveil config set` で変更してもコメントや書式は保持されます。

## チームでの共有

```toml
# 共通ルールを先に読み込み、この後に書いた内容で上書き・追加する
include = ["\\\\fileserver\\share\\sumiveil-team.toml", "%USERPROFILE%\\sumiveil-extra.toml"]
```

- 相対パスはこのファイルのフォルダ基準、`%VAR%` は環境変数として展開されます。
- `custom_rules` `keywords` `allowlist.*` のリストは **追加** (合成)、それ以外の値は **上書き** です。
- `sumiveil config export/import`、GUI の「設定」→「ファイル」からも受け渡しできます。

## マスク表示のテンプレート

`masking.template` (全体) → `categories.<id>.template` → `detectors.<id>.template` の順に上書きされます。

| 変数 | 内容 | 例 (元: `090-1234-5678`, 電話番号) |
|---|---|---|
| `{label}` / `{label_ja}` | ラベル | `PHONE` / `電話番号` |
| `{category}` / `{category_ja}` | カテゴリ | `contact` / `連絡先` |
| `{id}` | 検出器 ID | `phone_jp` |
| `{n}` | 同じ値には同じ連番 (表記ゆれも同一視) | `1` |
| `{len}` | 元の文字数 | `13` |
| `{hash}` / `{hash:N}` | HMAC-SHA256 の先頭 N 桁 (`masking.hash_salt` を鍵に使用) | `3fa9c2e1` |
| `{prefix:N}` / `{suffix:N}` | 先頭 / 末尾 N 文字 | `090` / `5678` |
| `{fill}` / `{fill:X}` / `{fill:X:N}` | 文字 X を元の長さ分 (または N 回) | `*************` |
| `{shape}` / `{shape:X}` / `{shape:X:K}` | 英数字だけ X に置き換え、区切りは残す (末尾 K 文字は残す) | `***-****-5678` |
| `{fake}` | 形式を保ったダミー値 (例: `user1@example.com`, `192.0.2.1`) | `090-0000-0001` |

固定の語や記号もそのまま使えます (`[個人情報]`、`■`、`[REDACTED]` など)。`{{` `}}` は `{` `}` そのもの。

## 主な項目

```toml
[general]
language = "auto"          # auto | ja | en
theme = "system"           # light | dark | system
active_profile = "default"

[masking]
template = "<{label}_{n}>"
min_confidence = 0.5       # 0.0〜1.0。下げると検出漏れが減り誤検出が増える
hash_salt = ""

[categories.network]       # contact personal jp_id intl_id finance network secret location organization custom
enabled = true
template = "[{label}]"

[detectors.hostname]       # ID は docs/detectors.md または sumiveil list-detectors
enabled = false
template = "{fake}"
min_confidence = 0.7
priority = 50              # 重なったときは優先度 → 信頼度 → 長さ の順で採用

[names]
propagate = true           # 確度の高い人名を文中の他の出現箇所にも適用
use_dictionary = true      # 内蔵の姓名辞書 (敬称なしの人名)
dictionary_threshold = 0.6
use_morphology = false     # 形態素解析 (Lindera + IPADIC)。追加の辞書が必要 (インストーラーで「形態素解析辞書」にチェック、ポータブル版は morphology-dict.zip を同じフォルダに展開)
morphology_dict = ""       # 辞書フォルダ。空なら <exe>\dict\ipadic、<exe>\..\dict\ipadic、%LOCALAPPDATA%\Sumiveil\dict\ipadic の順に探す

[[keywords]]               # 必ずマスクする語
label = "CLIENT"
name = "取引先"
words = ["アクメ商事", "Acme Corp"]
case_sensitive = false
whole_word = false

[[custom_rules]]           # 正規表現 (名前付きグループ v があればその部分だけ)
id = "ticket"
name = "チケット番号"
label = "TICKET"
pattern = 'TCK-\d{6}'
priority = 70
template = "<{label}_{n}>"
case_insensitive = false

[allowlist]                # マスクしない
values = ["support@example.ne.jp"]
domains = ["example.com", "example.ne.jp"]   # メール・ホスト名・URL (サブドメイン含む)
patterns = ['192\.168\..*']                   # 値全体に一致する正規表現

[gui]
font_size = 14.0
auto_mask = true
debounce_ms = 250
tray_enabled = true
close_to_tray = false
hotkey = "Ctrl+Alt+M"
accent_color = "auto"      # アクセントカラー: auto (Windows の設定に従う) | "#RRGGBB"
renderer = "auto"          # 描画方式: auto | opengl | directx | software (表示がおかしい・真っ白なときは software。再起動後に反映)

[gui.shortcuts]            # アプリ内のショートカットキー ("" で無効)
open = "Ctrl+O"            # ファイルを開く
paste = "Ctrl+Shift+V"     # クリップボードから貼り付け (置き換え)
copy = "Ctrl+Shift+C"      # マスク結果をコピー
save = "Ctrl+S"            # マスク結果を保存
run = "F5"                 # マスクを再実行
settings = "Ctrl+,"        # 設定を開く
find = "Ctrl+F"            # 検索バーを開く

[batch]
include = ["*.txt", "*.log", "*.csv", "*.eml", "*.docx", "*.xlsx", "*.pdf"]   # 既定はテキスト・メール・Office 文書・PDF など
suffix = ".masked"
output_encoding = "same"   # テキストファイルの出力: same | utf-8 | utf-8-bom | shift_jis | utf-16le

[files]
properties = "ask"         # Office 文書のプロパティ (作成者・会社名など): ask (その都度確認) | mask | clear (消す) | keep (残す)
```

ショートカットキーは、GUI の「設定」→「キー操作とトレイ」→「アプリ内のショートカット」で、実際にキーを押して登録することもできます。
Ctrl か Alt を含めてください (F1〜F24 は単独でも可)。Ctrl+C / V / X / A / Z / Y は入力欄で使うため割り当てられません。
同じキーを 2 つの操作やグローバルホットキー (`hotkey`) に割り当てると、`sumiveil config validate` で警告が出ます。

## プロファイル

`[profiles.<名前>]` の下に、基本設定との差分を書きます。

```toml
[profiles.customer_mail]
description = "顧客へのメール転送用"
[profiles.customer_mail.masking]
template = "[{label_ja}]"
[profiles.customer_mail.categories.network]
enabled = false
```

切り替え: GUI のツールバー、`sumiveil config use customer_mail`、または一時的に `sumiveil -p customer_mail ...`。
GUI の設定画面でプロファイル使用中に「検出対象」(人名・地名を含む)「マスクの表示」を変更すると、そのプロファイル内に保存されます。

同梱のプロファイル:

| 名前 | 用途 |
|---|---|
| `llm` | 生成 AI への送信用 (文脈を保つ連番タグ) |
| `log_share` | ログ共有用 (形式を保つマスク、IP はダミー値) |
| `strict` | 厳格 (日付・金額・URL・地名・高エントロピー文字列も対象) |
