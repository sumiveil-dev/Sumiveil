//! 診断レポート (GUI)。内部エラー・起動失敗のときに自動で作り、設定 →「情報」からも手動で作れる。
//! ファイルを作るだけで、どこにも送信しない (内容は sumiveil_core::diag を参照)。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use sumiveil_core::config::{self, LoadedConfig};
use sumiveil_core::diag::{self, Info, Reason};

static RENDERER: OnceLock<String> = OnceLock::new();

/// 使っている描画方式 (GPU 名など) を記録する (レポートに書く)。
pub fn set_renderer(info: &str) {
    let _ = RENDERER.set(info.to_string());
}

fn extra(more: Vec<(&'static str, String)>) -> Vec<(&'static str, String)> {
    let mut v = vec![];
    if let Some(r) = RENDERER.get() {
        v.push(("描画方式 (実際)", r.clone()));
    }
    if let Ok(r) = std::env::var("SUMIVEIL_RENDERER") {
        if !r.trim().is_empty() {
            v.push(("環境変数 SUMIVEIL_RENDERER", r));
        }
    }
    v.extend(more);
    v
}

fn write(reason: Reason, config_path: &Path, loaded: Option<&LoadedConfig>, config_error: Option<&str>, error: Option<String>, more: Vec<(&'static str, String)>) -> std::io::Result<PathBuf> {
    let info = Info { reason, program: "sumiveil-gui", config_path, loaded, config_error, extra: extra(more), error };
    diag::write_report(&info)
}

/// 設定 →「情報」の「診断レポートを作成」。
pub fn create_manual(config_path: &Path, loaded: &LoadedConfig, config_error: Option<&str>) -> std::io::Result<PathBuf> {
    write(Reason::Manual, config_path, Some(loaded), config_error, None, vec![])
}

/// 診断レポートの保存先フォルダ。
pub fn folder(config_path: &Path) -> PathBuf {
    diag::report_dir(config_path)
}

/// 起動時の設定を読み直してレポートを作る (内部エラー・起動失敗のとき。アプリの状態に頼らない)。
fn write_from_scratch(reason: Reason, error: String) -> Option<PathBuf> {
    let path = config::resolve_config_path(None);
    let (loaded, err) = match config::load(&path, None) {
        Ok(l) => (Some(l), None),
        Err(e) => (None, Some(e.to_string())),
    };
    write(reason, &path, loaded.as_ref(), err.as_deref(), Some(error), vec![]).ok()
}

fn notify(title: &str, path: Option<&Path>) {
    let body = match path {
        Some(p) => format!(
            "{title}\n\n診断レポートを作成しました (自動では送信されません):\n{}\n\n問い合わせの際は、内容を確認してからこのファイルを担当者に送ってください。\n\nA diagnostic report was saved (it is never sent automatically).",
            p.display()
        ),
        None => format!("{title}\n\n診断レポートを保存できませんでした。"),
    };
    crate::win::error_box("Sumiveil", &body);
    // OK を押したら、エクスプローラーでファイルを選んだ状態で開く (送るファイルを探しやすく)
    if let Some(p) = path {
        crate::win::reveal_in_explorer(p);
    }
}

/// 内部エラー (パニック) のときに診断レポートを作り、知らせる。
pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        static BUSY: AtomicBool = AtomicBool::new(false);
        if !BUSY.swap(true, Ordering::SeqCst) {
            let msg = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "(不明)".into());
            let loc = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
            let thread = std::thread::current().name().unwrap_or("(名前なし)").to_string();
            let bt = std::backtrace::Backtrace::force_capture();
            let error = format!("スレッド: {thread}\n場所: {loc}\n内容: {msg}\n\nバックトレース:\n{bt}");
            let path = write_from_scratch(Reason::Crash, error);
            notify("Sumiveil で内部エラーが発生しました。/ An internal error occurred.", path.as_deref());
            BUSY.store(false, Ordering::SeqCst);
        }
        default(info);
    }));
}

/// 描画方式をすべて試しても起動できなかったときに診断レポートを作り、知らせる。
pub fn report_startup_failure(errors: &[String]) {
    let error = format!("試した描画方式と結果:\n{}", errors.join("\n"));
    let path = write_from_scratch(Reason::StartupFailure, error);
    notify("Sumiveil の画面を表示できませんでした。/ Sumiveil could not open its window.", path.as_deref());
}
