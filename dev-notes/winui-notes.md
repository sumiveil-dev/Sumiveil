# 将来 WinUI 3 を採用する場合のメモ

Sumiveil の GUI は egui で作り、配色・部品・アイコンはすべて独自のデザイン (「墨と和紙」) にしている。
このメモは、将来 WinUI 3 の見た目を採用したくなったときの判断材料と移行の手がかりをまとめたもの。
**現時点では WinUI 関連のコード・依存ライブラリ・ランタイムは一切含めていない** (ライセンス上の影響なし)。

調査日: 2026-09-30。バージョンや条件は変わるので、採用前に必ず最新の情報を確認すること。

## 1. 選択肢

| 方式 | 内容 | ライセンス |
|---|---|---|
| A. egui のまま WinUI 風の見た目にする | `theme.rs` の配色と `widgets.rs` の部品の描き方を変えるだけ。ランタイム不要 | 変化なし。ただし「WinUI」「Fluent」は Microsoft の名称なので、宣伝文に使わない |
| B. `windows-reactor` で WinUI 3 に移行する | Microsoft 公式の、Rust から WinUI 3 を使う宣言的 UI ライブラリ (windows-rs リポジトリ内) | ライブラリは MIT OR Apache-2.0。**ただし実行には Windows App SDK ランタイムが必要で、こちらは Microsoft ソフトウェア ライセンス条項** (下記) |

## 2. B を選ぶ場合に確認すること

- ライブラリ: `windows-reactor` (調査時 0.100.0、初公開 2026-04-27)。自己完結の配布には `windows-reactor-setup` (build.rs で NuGet からランタイムを取得して exe の横に置く)
  - https://github.com/microsoft/windows-rs/tree/master/crates/libs/reactor
  - https://github.com/microsoft/windows-rs/blob/master/docs/crates/windows-reactor-setup.md
- Windows App SDK ランタイムのライセンス (NuGet の Microsoft.WindowsAppSDK.Runtime のライセンス)
  - 再配布は可。ただし次が条件になる
    - アプリに重要な主機能を加えること
    - 利用者に、Microsoft を同等以上に保護する条項へ同意させること (MIT の LICENSE.txt だけでは足りない。インストーラーの使用許諾に追記が必要)
    - アプリに関する請求について Microsoft を補償・免責すること
    - Microsoft の商標を、推奨・関与を示唆する形で使わないこと
  - 「ソフトウェアが情報を収集し Microsoft に送信する場合がある」との条項がある。README の「完全オフライン・テレメトリなし」と両立するかは、採用前に実機の通信で確認すること (調査時点では未確認)
- 成熟度: 2026 年 8 月に閉じられた Issue (#4837, #4834) に機能不足の報告がある。採用前に、下の「難所」を小さな試作で確かめること
- 配布: ポータブル ZIP とインストーラーにランタイム一式を同梱する必要がある (サイズ増は未計測)。ビルド時に NuGet へ接続する

## 3. 移行するときの対応表 (今の部品 → WinUI の部品)

画面の部品はすべて `crates/sumiveil-gui/src/widgets.rs` に集めてあり、各画面はここを呼ぶだけにしている。
移行や見た目の差し替えは、原則としてこのファイルと `theme.rs`・`icons.rs` の中で閉じる。

| widgets.rs | 役割 | WinUI 3 で近い部品 |
|---|---|---|
| `command_button` / `command_toggle` | 上部の操作列 | CommandBar の AppBarButton / AppBarToggleButton |
| `split_button` | 主操作 + メニュー | SplitButton |
| `flyout_button` | ポップアップを開く | Button + Flyout |
| `tabs` | 上部のタブ | SelectorBar |
| `expander` | 折りたたみ | Expander |
| `card` / `setting_row` / `toggle_row` | 設定の行 | (Community Toolkit の SettingsCard 相当。WinUI 本体には無い) |
| `toggle` | オン/オフ | ToggleSwitch |
| `slider` | スライダー | Slider |
| `segmented` | 択一の切り替え | RadioButtons / SelectorBar |
| `nav_item` | 左のメニュー | NavigationView |
| `info_bar` | お知らせの帯 | InfoBar |
| `icons::Icon` | 自作アイコン 34 種 | Segoe Fluent Icons (FontIcon)。**Windows 上で参照するだけなら可、フォントの同梱は不可** (Microsoft Learn の Segoe Fluent Icons のページ) |

## 4. 難所 (試作で先に確かめること)

- 左右比較のエディタ: 検出箇所の色分け・下線、行番号、左右のスクロール同期、ホバーで詳細表示 (`mask_page.rs` の `build_job` など)。RichEditBox / TextBlock の書式指定で同じことができるか
- トレイ常駐とグローバルホットキー (今は tray-icon と global-hotkey)。windows-rs の `windows-notifyicon` で代替できるか
- 一括処理の進捗表示と中止 (別スレッドの worker との連携)
- 起動時間と常駐時のメモリ (今は約 130 MB。README の値)

## 5. A を選ぶ場合 (egui のまま見た目だけ寄せる)

- `theme.rs` の `Palette::new` の色、`widgets.rs` の角丸 (今は 3〜4px) と選択の印 (今は強調色の直線) を変える
- アイコンは自作のまま (Segoe のグリフを使う場合は、同梱せず system フォントを参照するだけにする)
- 名称「WinUI」「Fluent」を README や宣伝文に書かない (デザインの説明は「Windows 11 になじむ見た目」などにする)
