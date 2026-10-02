# Sumiveil

テキストに含まれる個人情報や機密情報 (氏名・メールアドレス・電話番号・住所・マイナンバー・API キー・パスワードなど) を検出し、
`<NAME_1>` のような置き換えでマスクする Windows 用のツールです。GUI とコマンドラインの両方で使えます。
ログやメール、チャットの内容を生成 AI や社外の相手に渡す前の匿名化を想定しています。

> 自動検出は完全ではありません。社外に出す前に、必ず結果を目で確認してください。

- **完全オフライン**: インターネットには一切接続しません (テレメトリなし)。
- **軽快**: 1 MB のテキストを約 0.1 秒でマスク。GUI の起動は一瞬、常駐時のメモリは約 130 MB。
- **左右比較**: 元のテキストとマスク後を並べて表示し、置換箇所を色分けハイライト。
- **67 種類の検出器** を 10 カテゴリで個別/一括に切り替え可能 ([一覧](docs/detectors.md))。
- **マスク表示を自由に指定**: `<EMAIL_1>` `[メールアドレス]` `●●●●` `***-****-5678` `[個人情報]` など。
- **設定はテキストファイル (TOML)**: GUI・コマンド・ファイル直接編集のどれでも変更でき、チームで共有可能。

## このプロジェクトについて

- 個人が趣味で開発している小規模なプロジェクトです。質問への回答や不具合の修正には時間がかかることがあります。あらかじめご了承ください。
- 開発には、Anthropic の AI コーディング支援ツール「Claude Code」を併用しています。
- 不具合の報告や要望は [Issues](https://github.com/sumiveil-dev/Sumiveil/issues) へお寄せください。
  セキュリティ上の問題など、公開の場に書きにくい内容は、リポジトリの「Security」タブから非公開で報告できます。
  Issues には、個人情報や機密情報を含む文章をそのまま貼り付けないでください。

## English

Sumiveil is a Windows tool that detects personal and confidential data in text
(names, email addresses, phone numbers, addresses, national ID numbers, API keys, passwords, and more)
and masks it with placeholders such as `<NAME_1>`. It comes with both a GUI and a command-line interface.
It is meant for anonymizing logs, emails, and chat transcripts before you share them with AI services or outside parties.
The user interface and the user guide are mainly in Japanese (the GUI also has an English mode).

- Runs fully offline and never sends your input anywhere (no telemetry)
- 67 detectors that you can turn on or off by category
- Automatic detection is not perfect. Always review the result before sharing it

Download `Sumiveil-Setup-<version>-x64.exe` from [Releases](https://github.com/sumiveil-dev/Sumiveil/releases) and run it
(per-user install needs no administrator rights). A portable ZIP and an optional morphological-analysis dictionary ZIP are also available.

About this project:

- This is a small hobby project maintained by one person. Replies and fixes may take a while. Thank you for your patience.
- Development uses Claude Code, an AI coding assistant by Anthropic.
- Please report bugs and requests via [Issues](https://github.com/sumiveil-dev/Sumiveil/issues).
  For security problems or anything you would rather not post publicly, use private reporting from the repository's "Security" tab.
  Please do not paste text that contains personal or confidential data into an issue.

## 主な機能

| 機能 | 内容 |
|---|---|
| 左右比較表示 | 行番号・スクロール同期・ホバーで詳細表示・クリックで該当箇所へジャンプ |
| 検出一覧 | 1 行 1 件の一覧・種類での絞り込み・チェックを外して一時的に除外・許可リストへ追加 |
| 検索 (Ctrl+F) | 左右の一致箇所を強調・前後へ移動・正規表現と大文字小文字の区別。見つけた語を「今だけマスク」または「キーワード辞書に登録」。検出一覧も検索語で絞り込み |
| テーマ | ライト / ダーク / システム (OS に追従)。和紙と墨をモチーフにした独自のデザインと自作アイコン。アクセントカラーは Windows の設定に追従 (手動指定も可) |
| 言語 | 日本語 / English (自動判定・切り替え) |
| プロファイル | 「LLM 送信用」「ログ共有用」「厳格」などを切り替え (独自に作成も可) |
| 連番の一貫性 | 同じ値には同じ番号 (`<NAME_1>`)。表記ゆれ (全角/半角、ハイフン有無) も同一視 |
| 人名・地名 | 敬称・項目名などの文脈ルール + 内蔵辞書 (姓名約 3 万・地名約 6.6 万。ローマ字の日本人名 (Hanako Suzuki / YAMADA Taro / Tanaka-san) にも対応) + スコア統合。辞書にない珍しい姓 (小鳥遊・五百旗頭 など) も、敬称や辞書の名と並んでいれば姓の字の特徴から検出。任意で形態素解析 (Lindera + IPADIC) も追加可能 |
| カスタムルール | 正規表現 (先読み・後読み可) とキーワード辞書 (社名・顧客名・コードネーム) |
| 許可リスト | マスクしない値・ドメイン・正規表現 |
| トレイ常駐 + ホットキー | どのアプリからでも **Ctrl+Alt+M** でクリップボードをその場でマスク |
| ショートカットキー | 開く・貼り付け・コピー・保存・再実行・設定・検索とホットキーを、設定画面でキーを押して自由に変更 (`[gui.shortcuts]`) |
| 一括処理 | フォルダをサブフォルダ込みで一括マスク (CSV/JSON レポート、進捗・中止)。フォルダ内のファイルを横断して検索し、結果からその位置を開くことも可能 |
| 診断レポート | 問題が起きたときに、バージョン・Windows・描画方式・設定の要約をテキストファイルに書き出す (内部エラー時は自動、「設定」→「情報」または `sumiveil diagnose` で手動)。**送信はしない**。入力した文章や辞書に登録した語は含めない |
| 文字コード | UTF-8 / UTF-8 BOM / UTF-16 / Shift_JIS / EUC-JP / ISO-2022-JP を自動判定し、同じ文字コードで保存 |
| 対応ファイル | テキスト・ログ・CSV・JSON・ソースコード・`.har` など、メール (`.eml` は解読して同じ形式で、`.msg` はテキストで)、Word / Excel / PowerPoint (書式を保ったまま)、PDF (テキストで出力)。Office 文書の作成者などのプロパティは「確認 / マスク / 消す / 残す」を選択 |
| コマンドライン | パイプ対応、テキスト/JSON 出力、`check` で CI・pre-commit に組み込み |

## インストール

[GitHub の Releases](https://github.com/sumiveil-dev/Sumiveil/releases) から `Sumiveil-Setup-<version>-x64.exe` をダウンロードして実行します。

- 起動時に **「現在のユーザーのみ」(管理者権限不要) / 「すべてのユーザー」** を選べます。
- オプション: デスクトップのショートカット、**PATH への追加** (コマンドラインで `sumiveil` が使える)、「送る」メニュー、右クリックメニュー、Windows 起動時にトレイ常駐。
- コンポーネントの画面で「形態素解析辞書」にチェックを入れると、辞書 (約 45 MB) も入ります。設定の「検出対象」→「人名・地名」→「形態素解析を使う」で有効になります。
- 新しいバージョンのインストーラーを実行すると、上書き更新になります (インストール先・設定・前回のオプションは引き継がれ、使用許諾の画面は省かれます)。
- インストーラーは Windows のライト/ダーク設定に合わせた配色で表示されます。
- サイレントインストール: `Sumiveil-Setup-x.y.z-x64.exe /VERYSILENT /CURRENTUSER` (全ユーザーは `/ALLUSERS`)、タスク指定は `/TASKS="desktopicon,addtopath"`。

インストールせずに使う場合は `Sumiveil-<version>-portable-x64.zip` を展開してください (形態素解析辞書は `Sumiveil-<version>-morphology-dict.zip` を同じフォルダに重ねて展開します)。同じフォルダに `sumiveil.portable` (または設定ファイル `sumiveil.toml`) があると、設定をそのフォルダの `sumiveil.toml` に保存するポータブルモードになります。ZIP には設定ファイルを入れていないので、新しい版を同じフォルダに上書き展開しても設定は消えません (コメント付きの既定の設定は `sumiveil.default.toml` にあります)。

> コード署名していないため、初回起動時に Microsoft Defender SmartScreen の警告が表示されることがあります。配布元 (GitHub の Releases) を確認したうえで「詳細情報」→「実行」で起動してください。社内で配布する場合は、社内のコード署名証明書で署名することもできます。


## 利用ガイド (PDF)

画面写真付きの利用者向けガイド (PDF) は、[Releases](https://github.com/sumiveil-dev/Sumiveil/releases) の `Sumiveil-<version>-UserGuide-ja.pdf` です。インストーラー版・ポータブル版にも同梱しています (スタートメニューの「Sumiveil 利用ガイド」、アプリの「設定」→「情報」からも開けます)。
インストール手順・基本操作・ホットキー・フォルダ一括処理・設定・よくあるつまずきと対処 (Q&A) をまとめています。
元の HTML は `docs/guide/guide.html` で、`scripts/build-guide.ps1` で PDF を作り直せます (`scripts/package.ps1` が自動で行います)。

## GUI の使い方

1. テキストを貼り付ける (Ctrl+V)、ファイルを開く (Ctrl+O)、またはファイルをウィンドウにドロップします。
2. 入力と同時に右側にマスク後のテキストが表示されます。
3. 「結果をコピー」(Ctrl+Shift+C) で使います。右の ▾ から「ファイルに保存」(Ctrl+S) と「検出レポートを保存 (JSON)」も選べます。

画面は、上の操作列 (開く・貼り付け・検索・⋯ / プロファイル・結果をコピー)、左右のエディタ、右の検出一覧、下のステータスバーで構成されています。
Office 文書などを開くと、形式・注意・プロパティの扱いが 1 行の状態バーに表示され、押すと詳しい内容を確認できます。

| キー | 動作 |
|---|---|
| Ctrl+O | ファイルを開く |
| Ctrl+Shift+V | クリップボードから貼り付け (入力を置き換え) |
| Ctrl+Shift+C | マスク結果をコピー |
| Ctrl+S | マスク結果を保存 (元のファイルと同じ文字コード) |
| F5 | マスクを再実行 |
| Ctrl+, | 設定 |
| Ctrl+F | 検索 (Enter / F3 で次、Shift+Enter / Shift+F3 で前、Esc で閉じる) |
| Ctrl+Alt+M (変更可) | **どのアプリからでも** クリップボードをその場でマスク |

## コマンドラインの使い方

```powershell
sumiveil report.txt                      # マスクして標準出力へ
sumiveil report.txt -o masked.txt        # ファイルへ
type app.log | sumiveil --format json    # パイプ + JSON
sumiveil -t "電話 090-1234-5678"          # 文字列を直接
sumiveil -r .\logs --out-dir .\masked --report report.csv   # フォルダ一括
sumiveil check .\src --include *.env -r  # 見つかったら終了コード 1 (CI 用)
sumiveil -p llm --template "[{label}]" memo.txt             # プロファイル・表示の指定
sumiveil config set detectors.hostname.enabled false        # 設定変更
sumiveil diagnose                        # 診断レポートのファイルを作る (送信はしない)
```

GUI (`sumiveil-gui.exe`) の起動オプション: `ファイル` (開く)、`フォルダ` (一括処理の対象にする)、`--find 文字列` (検索バーに入れて開く。一括処理の画面ならフォルダを横断検索)、`--page mask|batch[:search]|settings[:項目]`。

詳しくは [docs/cli.md](docs/cli.md) を参照してください。

## 設定

設定ファイル (TOML) の場所は `sumiveil config path` で確認できます (既定: `%APPDATA%\Sumiveil\config.toml`)。
GUI の設定画面、`sumiveil config ...` コマンド、ファイルの直接編集のどれで変更しても、実行中の GUI に自動で反映されます。

```toml
include = ["\\\\fileserver\\share\\sumiveil-team.toml"]  # チーム共通の設定を読み込む

[masking]
template = "<{label}_{n}>"

[detectors.credit_card]
template = "****-****-****-{suffix:4}"

[[keywords]]
label = "CLIENT"
words = ["アクメ商事", "Acme Corp"]

[allowlist]
domains = ["example.com", "example.ne.jp"]
```

詳しくは [docs/config.md](docs/config.md) を参照してください。

## 注意事項

- 自動検出は完全ではありません。社外に出す前に、左右比較画面で結果を必ず目視確認してください。
- 検出の感度は「最低信頼度」やプロファイルで調整できます。見逃しが多い場合は `strict` プロファイルを試してください。
- 会社名は「株式会社」「(株)」などの法人格を手がかりに検出します。法人格の付かない取引先名・製品名・案件名は、「設定」→「辞書とルール」の「キーワード辞書」に登録すると確実に隠せます。
- 敬称や項目名の付かない姓だけの記載は、「田中・佐藤・鈴木の 3 名」「出席者: 田中、佐藤」のような並びや、「田中が担当」「山本に連絡」「田中より」のような書き方なら検出します。それ以外の位置にある姓だけの記載は見逃すことがあります (「森林」「出口」のように姓と同じ表記の一般語が多いため)。`strict` プロファイルでは、行に姓だけがある場合なども検出します。形態素解析 (追加の辞書) やキーワード辞書も使えます。
- 問題が起きたときは「設定」→「情報」の「診断レポートを作成」で作ったファイル (設定ファイルと同じ場所の `diagnostics` フォルダ) を、内容を確認してから [GitHub の Issues](https://github.com/sumiveil-dev/Sumiveil/issues) に添付してください (個人情報が含まれていないことを必ず確認してください)。Sumiveil が自動で送信することはありません。

## ビルド (開発者向け)

必要なもの:

- Rust (stable、MSVC ターゲット) と Visual Studio Build Tools (C++ ビルドツール)
- Inno Setup 7.1 以降 (インストーラーを作る場合。未インストールなら `setup-tools.ps1` がプロジェクト内 `tools\` にポータブル配置します)
- Microsoft Edge (利用ガイドの PDF 作成に使用。Windows 10/11 に標準搭載)
- 任意: `cargo install cargo-about` (`THIRD-PARTY-NOTICES.txt` を作り直す場合)

crates.io のキャッシュなどはプロジェクト内 (`.cargo-home`) に閉じ込めています。形態素解析辞書は、初回の `package.ps1` が mecab-ipadic を GitHub から取得して作ります。

```powershell
. .\scripts\env.ps1          # CARGO_HOME をプロジェクト内 .cargo-home に設定
cargo test --workspace       # テスト
.\scripts\setup-tools.ps1    # Inno Setup 7 が無ければプロジェクト内 tools\ にポータブル配置 (初回のみ)
.\scripts\package.ps1        # リリースビルド → dist\ にインストーラーとポータブル ZIP
```

| パス | 内容 |
|---|---|
| `crates/sumiveil-core` | 検出エンジン・設定・文字コード処理 (GUI/CLI 共通) |
| `crates/sumiveil-cli` | `sumiveil.exe` (コマンドライン) |
| `crates/sumiveil-gui` | `sumiveil-gui.exe` (egui による GUI) |
| `installer/sumiveil.iss` | Inno Setup スクリプト |
| `config/default.toml` | コメント付きの既定設定 |
| `crates/sumiveil-core/data` | 内蔵辞書 (mecab-ipadic と米国国勢調査局の公開データから生成済み) と `IPADIC-LICENSE.txt` |

## ライセンス

MIT License (`LICENSE.txt`)。Copyright (c) 2026 sumiveil-dev

内蔵辞書は mecab-ipadic (`crates/sumiveil-core/data/IPADIC-LICENSE.txt`。配布物には同梱) と米国国勢調査局の公開データ (パブリックドメイン) から作成しています。
使用しているライブラリのライセンスは `THIRD-PARTY-NOTICES.txt` を参照してください。
プライバシーポリシーは [PRIVACY.md](PRIVACY.md) にあります (情報は一切収集しません)。

アイコン・アプリのアイコン・画面のデザインは Sumiveil の開発者による独自のものです。
記載されている会社名・製品名・サービス名は、各社の商標または登録商標です。Sumiveil は、それらの会社とは関係がありません。
