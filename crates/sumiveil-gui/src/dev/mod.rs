//! 開発・検証用の仕組み。配布版 (feature 無しのビルド) では、ここの処理は何もしない。
//! - feature "guide-capture": 利用ガイドの画像をアプリ自身が作る (guide_capture.rs)
//! - feature "dev-hooks": 環境変数で動作確認用の動きをさせる
//!   - `SUMIVEIL_DEBUG_PANIC`: 起動直後にわざと内部エラーを起こす (診断レポートの確認用)
//!   - `SUMIVEIL_DEBUG_REPAINT=<ファイル>`: 再描画の理由をファイルに記録する (アイドル時の CPU 使用率の調査用)
//!
//! 本体 (`App`) には手を入れず、`wrap` で包んで eframe に渡す。

use std::ffi::OsString;

#[cfg(feature = "guide-capture")]
pub mod guide_capture;

/// 撮影で操作・切り抜きに使う部品の位置を記録する。配布版では何もしない。
#[cfg(feature = "guide-capture")]
#[macro_export]
macro_rules! guide_mark {
    ($ctx:expr, $name:expr, $rect:expr) => {
        $crate::dev::guide_capture::mark($ctx, $name, $rect)
    };
}
#[cfg(not(feature = "guide-capture"))]
#[macro_export]
macro_rules! guide_mark {
    ($ctx:expr, $name:expr, $rect:expr) => {
        let _ = (&$ctx, &$rect);
    };
}

/// 起動の最初に呼ぶ。開発用の引数を取り除いた残りを返す (インストーラーの撮影なら、ここで終了する)。
pub fn take_args(args: Vec<OsString>) -> Vec<OsString> {
    #[cfg(feature = "dev-hooks")]
    if std::env::var_os("SUMIVEIL_DEBUG_PANIC").is_some() {
        panic!("SUMIVEIL_DEBUG_PANIC による動作確認 (伏せ字の確認: `山田太郎 090-1234-5678`)");
    }
    #[cfg(feature = "guide-capture")]
    let args = guide_capture::take_args(args);
    args
}

/// eframe に渡すアプリ。開発用の機能を組み込んだビルドなら、本体を包む。
pub fn wrap(app: crate::app::App) -> Box<dyn eframe::App> {
    #[cfg(any(feature = "guide-capture", feature = "dev-hooks"))]
    return Box::new(DevApp::new(app));
    #[cfg(not(any(feature = "guide-capture", feature = "dev-hooks")))]
    Box::new(app)
}

#[cfg(any(feature = "guide-capture", feature = "dev-hooks"))]
struct DevApp {
    inner: crate::app::App,
    #[cfg(feature = "guide-capture")]
    shot: Option<guide_capture::Shot>,
    #[cfg(feature = "dev-hooks")]
    repaint_log: Option<OsString>,
}

#[cfg(any(feature = "guide-capture", feature = "dev-hooks"))]
impl DevApp {
    fn new(inner: crate::app::App) -> Self {
        Self {
            inner,
            #[cfg(feature = "guide-capture")]
            shot: guide_capture::Shot::from_args(),
            #[cfg(feature = "dev-hooks")]
            repaint_log: std::env::var_os("SUMIVEIL_DEBUG_REPAINT"),
        }
    }
}

/// 本体のメソッドへそのまま受け渡し、前後に開発用の処理をはさむ。
#[cfg(any(feature = "guide-capture", feature = "dev-hooks"))]
impl eframe::App for DevApp {
    fn raw_input_hook(&mut self, ctx: &eframe::egui::Context, raw_input: &mut eframe::egui::RawInput) {
        #[cfg(feature = "guide-capture")]
        if let Some(s) = self.shot.as_mut() {
            s.raw_input_hook(ctx, raw_input);
        }
        self.inner.raw_input_hook(ctx, raw_input);
    }

    fn logic(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        #[cfg(feature = "guide-capture")]
        if let Some(s) = self.shot.as_mut() {
            s.logic(ctx);
        }
        self.inner.logic(ctx, frame);
    }

    fn ui(&mut self, ui: &mut eframe::egui::Ui, frame: &mut eframe::Frame) {
        self.inner.ui(ui, frame);
        #[cfg(feature = "dev-hooks")]
        if let Some(path) = &self.repaint_log {
            let causes = ui.ctx().repaint_causes();
            if !causes.is_empty() {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                    let _ = writeln!(f, "{causes:?}");
                }
            }
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.inner.save(storage);
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        self.inner.on_exit(gl);
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        self.inner.auto_save_interval()
    }

    fn clear_color(&self, visuals: &eframe::egui::Visuals) -> [f32; 4] {
        self.inner.clear_color(visuals)
    }

    fn persist_egui_memory(&self) -> bool {
        self.inner.persist_egui_memory()
    }
}
