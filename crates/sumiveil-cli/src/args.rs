//! コマンドライン引数の定義と日本語ヘルプ。

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "sumiveil",
    version,
    about = "Mask personal and confidential information in text (offline).",
    long_about = None,
    args_conflicts_with_subcommands = true,
    disable_help_subcommand = true,
    after_help = "Examples:\n  sumiveil report.txt\n  type app.log | sumiveil --format json\n  sumiveil -t \"call 090-1234-5678\"\n  sumiveil -r logs --out-dir masked\n  sumiveil check src --include *.env\n  sumiveil config set detectors.hostname.enabled false"
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalOpts,

    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub mask: MaskArgs,
}

#[derive(Args, Debug, Clone)]
pub struct GlobalOpts {
    /// Config file to use (default: SUMIVEIL_CONFIG, portable sumiveil.toml (or sumiveil.portable marker), or %APPDATA%\Sumiveil\config.toml)
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Profile to use (e.g. llm, log_share, strict)
    #[arg(short = 'p', long, global = true, value_name = "NAME")]
    pub profile: Option<String>,

    /// When to use colors
    #[arg(long, global = true, value_enum, default_value_t = ColorWhen::Auto, value_name = "WHEN")]
    pub color: ColorWhen,

    /// Suppress warnings and summaries on stderr
    #[arg(short, long, global = true)]
    pub quiet: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum ColorWhen {
    Auto,
    Always,
    Never,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum, Default)]
pub enum Format {
    #[default]
    Text,
    Json,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Mask input (default command)
    Mask(MaskArgs),
    /// List detected items without masking
    Detect(MaskArgs),
    /// Exit with code 1 if anything sensitive is found (for CI / pre-commit)
    Check(MaskArgs),
    /// View or change settings
    #[command(subcommand)]
    Config(ConfigCmd),
    /// List available detectors and categories
    ListDetectors {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Write a diagnostic report file (never sent anywhere)
    Diagnose,
    /// Open the GUI
    Gui {
        /// File to open
        file: Option<PathBuf>,
    },
    /// Generate shell completion script
    Completions {
        /// Shell type
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Args, Debug, Clone, Default)]
pub struct MaskArgs {
    /// Input files or folders ("-" = stdin). Reads stdin when omitted
    #[arg(value_name = "INPUT")]
    pub inputs: Vec<PathBuf>,

    /// Mask this text directly instead of reading files
    #[arg(short = 't', long, value_name = "TEXT", conflicts_with_all = ["inputs", "from_clipboard"])]
    pub text: Option<String>,

    /// Read input from the clipboard
    #[arg(long, conflicts_with = "inputs")]
    pub from_clipboard: bool,

    /// Write output to this file (single input)
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Write outputs into this folder (multiple inputs / folders)
    #[arg(long, value_name = "DIR")]
    pub out_dir: Option<PathBuf>,

    /// Also copy the result to the clipboard
    #[arg(short = 'C', long)]
    pub to_clipboard: bool,

    /// Output format
    #[arg(short, long, value_enum, default_value_t = Format::Text)]
    pub format: Format,

    /// Process folders recursively
    #[arg(short, long)]
    pub recursive: bool,

    /// File name patterns to include in folders (default from config batch.include)
    #[arg(long, value_name = "GLOB")]
    pub include: Vec<String>,

    /// File name patterns to exclude
    #[arg(long, value_name = "GLOB")]
    pub exclude: Vec<String>,

    /// Input encoding (auto, utf-8, sjis, euc-jp, utf-16le, ...)
    #[arg(long, default_value = "auto", value_name = "ENC")]
    pub encoding: String,

    /// Output encoding (default: same as input for files, UTF-8 otherwise)
    #[arg(long, value_name = "ENC")]
    pub output_encoding: Option<String>,

    /// Enable detectors or categories (comma separated)
    #[arg(long, value_delimiter = ',', value_name = "ID")]
    pub enable: Vec<String>,

    /// Disable detectors or categories (comma separated)
    #[arg(long, value_delimiter = ',', value_name = "ID")]
    pub disable: Vec<String>,

    /// Use only these detectors or categories (comma separated)
    #[arg(long, value_delimiter = ',', value_name = "ID")]
    pub only: Vec<String>,

    /// Replacement template for all detectors (e.g. "[{label}]", "{fill:*}")
    #[arg(long, value_name = "TEMPLATE")]
    pub template: Option<String>,

    /// Ignore detections below this confidence (0.0-1.0)
    #[arg(long, value_name = "N")]
    pub min_confidence: Option<f32>,

    /// Include original values in JSON / reports (handle with care)
    #[arg(long)]
    pub include_original: bool,

    /// Omit the masked text from JSON output
    #[arg(long)]
    pub no_masked: bool,

    /// Write a summary report (.json or .csv)
    #[arg(long, value_name = "FILE")]
    pub report: Option<PathBuf>,

    /// Show changed lines as a diff instead of the masked text
    #[arg(long)]
    pub diff: bool,

    /// Print detection counts to stderr
    #[arg(long)]
    pub stats: bool,

    /// Process stdin line by line (for `tail -f` style streams)
    #[arg(long)]
    pub line_buffered: bool,

    /// Restart placeholder numbering (NAME_1, NAME_2, ...) for each file (default: shared across files)
    #[arg(long)]
    pub separate_numbering: bool,

    /// File name suffix when writing next to inputs (default from config batch.suffix)
    #[arg(long, value_name = "SUFFIX")]
    pub suffix: Option<String>,

    /// Office document properties (author, company...): mask, clear or keep (default from config files.properties; "ask" = clear)
    #[arg(long, value_name = "MODE", value_parser = ["mask", "clear", "keep"])]
    pub properties: Option<String>,

    /// Show original values in detect/check output
    #[arg(long)]
    pub show_values: bool,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCmd {
    /// Show the config file path
    Path,
    /// Show the config file (or the resolved settings)
    Show {
        /// Show merged settings including defaults, includes and profile
        #[arg(long)]
        resolved: bool,
    },
    /// Get a value (e.g. masking.template)
    Get { key: String },
    /// Set a value (e.g. detectors.hostname.enabled false)
    Set { key: String, value: String },
    /// Remove a value (revert to default)
    Unset { key: String },
    /// Enable detectors or categories
    Enable {
        #[arg(required = true, value_delimiter = ',')]
        ids: Vec<String>,
    },
    /// Disable detectors or categories
    Disable {
        #[arg(required = true, value_delimiter = ',')]
        ids: Vec<String>,
    },
    /// Create a config file with commented defaults
    Init {
        /// Overwrite an existing file
        #[arg(long)]
        force: bool,
        /// Create sumiveil.toml next to sumiveil.exe (portable mode)
        #[arg(long)]
        portable: bool,
    },
    /// Export the config to a file (for sharing)
    Export {
        file: PathBuf,
        /// Export merged settings instead of the raw file
        #[arg(long)]
        resolved: bool,
    },
    /// Import a config file (the current one is backed up to .bak)
    Import {
        file: PathBuf,
        /// Merge values into the current file instead of replacing it
        #[arg(long)]
        merge: bool,
    },
    /// Check a config file for errors
    Validate { file: Option<PathBuf> },
    /// Open the config file in an editor
    Edit,
    /// List profiles
    Profiles,
    /// Switch the active profile
    Use { profile: String },
}

/// ヘルプの日本語化 (引数 ID → 説明)。
const JA_ARGS: &[(&str, &str)] = &[
    ("config", "使用する設定ファイル (既定: SUMIVEIL_CONFIG → exe 横の sumiveil.toml (目印の sumiveil.portable があるときも) → %APPDATA%\\Sumiveil\\config.toml)"),
    ("profile", "使用するプロファイル (例: llm, log_share, strict)"),
    ("color", "色付けの有無"),
    ("quiet", "標準エラーへの警告・集計を出さない"),
    ("inputs", "入力ファイルまたはフォルダ (\"-\" は標準入力)。省略時は標準入力を読む"),
    ("text", "ファイルの代わりにこの文字列をマスクする"),
    ("from_clipboard", "クリップボードから入力する"),
    ("output", "出力ファイル (入力が 1 つのとき)"),
    ("out_dir", "出力フォルダ (複数入力・フォルダ入力のとき)"),
    ("to_clipboard", "結果をクリップボードにもコピーする"),
    ("format", "出力形式"),
    ("recursive", "フォルダを再帰的に処理する"),
    ("include", "フォルダ内で対象にするファイル名パターン (既定は設定の batch.include)"),
    ("exclude", "除外するファイル名パターン"),
    ("encoding", "入力の文字コード (auto, utf-8, sjis, euc-jp, utf-16le など)"),
    ("output_encoding", "出力の文字コード (既定: ファイルは入力と同じ、それ以外は UTF-8)"),
    ("enable", "有効にする検出器またはカテゴリ (カンマ区切り)"),
    ("disable", "無効にする検出器またはカテゴリ (カンマ区切り)"),
    ("only", "指定した検出器・カテゴリだけを使う (カンマ区切り)"),
    ("template", "全検出器のマスク表示テンプレート (例: \"[{label}]\", \"{fill:*}\")"),
    ("min_confidence", "この信頼度未満の検出を無視 (0.0〜1.0)"),
    ("include_original", "JSON・レポートに元の値を含める (取り扱い注意)"),
    ("no_masked", "JSON 出力にマスク後テキストを含めない"),
    ("report", "集計レポートを書き出す (.json または .csv)"),
    ("diff", "マスク後テキストの代わりに変更行を差分形式で表示"),
    ("stats", "検出件数を標準エラーに表示"),
    ("line_buffered", "標準入力を 1 行ずつ処理 (tail -f などのストリーム用)"),
    ("separate_numbering", "置換後の連番 (NAME_1, NAME_2 …) をファイルごとにリセット (既定はファイル間で共通)"),
    ("suffix", "入力の横に書き出すときのファイル名接尾辞 (既定は設定の batch.suffix)"),
    ("properties", "Office 文書のプロパティ (作成者・会社名など) の扱い: mask / clear (消す) / keep (残す)。既定は設定の files.properties (\"ask\" のときは clear)"),
    ("show_values", "detect/check の出力に元の値を表示"),
    ("json", "JSON で出力"),
    ("file", "対象ファイル"),
    ("shell", "シェルの種類"),
    ("resolved", "既定値・include・プロファイルを合成した設定を対象にする"),
    ("key", "キー (例: masking.template)"),
    ("value", "値"),
    ("ids", "検出器 ID またはカテゴリ ID"),
    ("force", "既存のファイルを上書きする"),
    ("portable", "sumiveil.exe と同じフォルダに sumiveil.toml を作る (ポータブル)"),
    ("merge", "置き換えではなく現在の設定に値を上書き合成する"),
];

const JA_SUBCOMMANDS: &[(&str, &str)] = &[
    ("mask", "入力をマスクする (既定のコマンド)"),
    ("detect", "マスクせずに検出結果を一覧表示"),
    ("check", "機密情報が見つかったら終了コード 1 (CI・pre-commit 用)"),
    ("config", "設定の表示・変更"),
    ("list-detectors", "検出器とカテゴリの一覧"),
    ("diagnose", "診断レポートのファイルを作成 (送信はしない)"),
    ("gui", "GUI を開く"),
    ("completions", "シェル補完スクリプトを出力"),
];

const JA_CONFIG_SUBCOMMANDS: &[(&str, &str)] = &[
    ("path", "設定ファイルのパスを表示"),
    ("show", "設定ファイル (または合成後の設定) を表示"),
    ("get", "値を取得 (例: masking.template)"),
    ("set", "値を設定 (例: detectors.hostname.enabled false)"),
    ("unset", "値を削除して既定値に戻す"),
    ("enable", "検出器・カテゴリを有効化"),
    ("disable", "検出器・カテゴリを無効化"),
    ("init", "コメント付きの既定設定ファイルを作成"),
    ("export", "設定をファイルに書き出す (共有用)"),
    ("import", "設定ファイルを取り込む (現在の設定は .bak に退避)"),
    ("validate", "設定ファイルの誤りをチェック"),
    ("edit", "設定ファイルをエディタで開く"),
    ("profiles", "プロファイルの一覧"),
    ("use", "使用するプロファイルを切り替え"),
];

fn tr_args(mut cmd: clap::Command) -> clap::Command {
    let ids: Vec<String> = cmd.get_arguments().map(|a| a.get_id().to_string()).collect();
    for (id, ja) in JA_ARGS {
        if ids.iter().any(|x| x == id) {
            cmd = cmd.mut_arg(*id, |a| a.help(*ja));
        }
    }
    cmd
}

/// 日本語環境向けにヘルプを差し替える。
pub fn localize(cmd: clap::Command) -> clap::Command {
    let mut cmd = tr_args(cmd)
        .about("テキスト中の個人情報・機密情報をマスクします (完全オフライン)")
        .after_help("例:\n  sumiveil report.txt\n  type app.log | sumiveil --format json\n  sumiveil -t \"電話 090-1234-5678\"\n  sumiveil -r logs --out-dir masked\n  sumiveil check src --include *.env\n  sumiveil config set detectors.hostname.enabled false");
    for (name, ja) in JA_SUBCOMMANDS {
        cmd = cmd.mut_subcommand(*name, |sc| {
            let mut sc = tr_args(sc.about(*ja));
            if *name == "config" {
                for (cname, cja) in JA_CONFIG_SUBCOMMANDS {
                    sc = sc.mut_subcommand(*cname, |c| tr_args(c.about(*cja)));
                }
            }
            sc
        });
    }
    cmd
}
