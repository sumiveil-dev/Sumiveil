//! Sumiveil GUI (Windows ネイティブ)。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod diag;
mod batch_page;
mod fonts;
#[cfg(feature = "guide-capture")]
mod guide_capture;
mod icon;
mod icons;
mod mask_page;
mod props_prompt;
mod settings_page;
mod shortcuts;
mod theme;
mod tray;
mod widgets;
mod win;
mod worker;

use std::path::PathBuf;

/// 利用ガイドの撮影で操作する部品の位置を記録する (guide-capture のビルドだけ。配布版では何もしない)。
#[cfg(feature = "guide-capture")]
#[macro_export]
macro_rules! guide_mark {
    ($ctx:expr, $name:expr, $rect:expr) => {
        $crate::guide_capture::mark($ctx, $name, $rect)
    };
}
#[cfg(not(feature = "guide-capture"))]
#[macro_export]
macro_rules! guide_mark {
    ($ctx:expr, $name:expr, $rect:expr) => {
        let _ = (&$ctx, &$rect);
    };
}

use eframe::egui;

fn main() -> eframe::Result<()> {
    diag::install_panic_hook();
    // 診断レポートの動作確認用: 環境変数 SUMIVEIL_DEBUG_PANIC があれば、わざと内部エラーを起こす
    if std::env::var_os("SUMIVEIL_DEBUG_PANIC").is_some() {
        panic!("SUMIVEIL_DEBUG_PANIC による動作確認 (伏せ字の確認: `山田太郎 090-1234-5678`)");
    }
    let mut file: Option<PathBuf> = None;
    let mut start_in_tray = false;
    let mut page: Option<String> = None;
    let mut find: Option<String> = None;
    #[cfg(feature = "guide-capture")]
    let mut capture: Option<(PathBuf, String)> = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(a) = args.next() {
        match a.to_str() {
            Some("--tray") => start_in_tray = true,
            Some("--page") => page = args.next().and_then(|p| p.into_string().ok()),
            // 検索バーにこの文字列を入れて開く (一括処理の画面なら横断検索を始める)
            Some("--find") => find = args.next().and_then(|p| p.into_string().ok()),
            // 利用ガイドの撮影用 (guide_capture.rs)
            #[cfg(feature = "guide-capture")]
            Some("--capture-shot") => capture = args.next().map(|p| (PathBuf::from(p), capture.take().map(|c| c.1).unwrap_or_default())),
            #[cfg(feature = "guide-capture")]
            Some("--capture-steps") => {
                let steps = args.next().and_then(|s| s.into_string().ok()).unwrap_or_default();
                capture = Some((capture.take().map(|c| c.0).unwrap_or_default(), steps));
            }
            #[cfg(feature = "guide-capture")]
            Some("--capture-setup") => {
                let installer = PathBuf::from(args.next().unwrap_or_default());
                let out = PathBuf::from(args.next().unwrap_or_default());
                std::process::exit(guide_capture::capture_setup(&installer, &out));
            }
            #[cfg(feature = "guide-capture")]
            Some("--capture-upgrade") => {
                let installer = PathBuf::from(args.next().unwrap_or_default());
                let out = PathBuf::from(args.next().unwrap_or_default());
                std::process::exit(guide_capture::capture_upgrade(&installer, &out));
            }
            Some(s) if s.starts_with("--") => {}
            _ => file = Some(PathBuf::from(a)),
        }
    }
    // 引数なしの起動では多重起動を防ぐ (既存のウィンドウを前面に出す)
    if file.is_none() && !win::acquire_single_instance() {
        return Ok(());
    }
    win::set_app_id();

    // 設定の「起動時はトレイに格納」と描画方式
    let mut renderer_setting = String::from("auto");
    if let Ok(l) = sumiveil_core::config::load(&sumiveil_core::config::resolve_config_path(None), None) {
        if l.config.gui.start_in_tray && l.config.gui.tray_enabled {
            start_in_tray = true;
        }
        renderer_setting = l.config.gui.renderer.clone();
    }

    // 描画方式を順に試し、起動に失敗したら次へ切り替える。
    let startup = || app::Startup {
        file: file.clone(),
        start_in_tray,
        page: page.clone(),
        find: find.clone(),
        #[cfg(feature = "guide-capture")]
        capture: capture.clone(),
    };
    let mut last = Ok(());
    let mut errors = vec![];
    for mode in render_modes(&renderer_setting) {
        debug_log(&format!("starting renderer {mode:?}"));
        match run(mode, startup()) {
            Ok(()) => return Ok(()),
            Err(e) => {
                eprintln!("renderer {mode:?} failed: {e}");
                errors.push(format!("{mode:?}: {e}"));
                last = Err(e);
            }
        }
    }
    // どの描画方式でも画面を出せなかった: 診断レポートを作って知らせる (黙って終了しない)
    diag::report_startup_failure(&errors);
    last
}

/// 診断用: 環境変数 SUMIVEIL_DEBUG_LOG のファイルに 1 行追記する。
pub fn debug_log(line: &str) {
    if let Some(path) = std::env::var_os("SUMIVEIL_DEBUG_LOG") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// 描画方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenderMode {
    /// OpenGL (軽量・高速起動。通常の PC の既定)
    OpenGl,
    /// Direct3D 12 (GPU)
    DirectX,
    /// Direct3D 12 のソフトウェア描画 (WARP)。GPU が使えない・不安定な環境でも確実に表示できる
    Software,
}

/// 試す順番。環境変数 SUMIVEIL_RENDERER > 設定 gui.renderer > 自動判定。
fn render_modes(setting: &str) -> Vec<RenderMode> {
    use RenderMode::*;
    let env = std::env::var("SUMIVEIL_RENDERER").unwrap_or_default();
    let pref = if env.trim().is_empty() { setting.to_ascii_lowercase() } else { env.to_ascii_lowercase() };
    match pref.trim() {
        "opengl" | "glow" => vec![OpenGl, Software],
        "directx" | "wgpu" | "dx12" => vec![DirectX, Software],
        "software" | "warp" | "cpu" => vec![Software],
        // 自動: 起動に失敗したら次の方式へ。環境による自動切り替えはしない
        // (Windows サンドボックスでは Direct3D 12 の初期化でサンドボックス自体が異常終了する例を確認したため)
        _ => vec![OpenGl, DirectX, Software],
    }
}

fn run(mode: RenderMode, startup: app::Startup) -> eframe::Result<()> {
    // トレイ起動時はウィンドウを表示しないまま作る (起動時のちらつき防止)
    let visible = !startup.start_in_tray;
    let icon = egui::IconData { rgba: icon::render(64), width: 64, height: 64 };
    // テキスト表示だけなので省電力 GPU (内蔵 GPU) を使い、メモリとバッテリー消費を抑える
    let mut wgpu_options = eframe::egui_wgpu::WgpuConfiguration::default();
    wgpu_options.surface.desired_maximum_frame_latency = Some(1);
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(c) = &mut wgpu_options.wgpu_setup {
        c.power_preference = eframe::wgpu::PowerPreference::LowPower;
        if mode == RenderMode::Software {
            // Windows 標準のソフトウェア描画 (Microsoft Basic Render Driver / WARP) を選ぶ
            c.native_adapter_selector = Some(std::sync::Arc::new(|adapters: &[eframe::wgpu::Adapter], _surface: Option<&eframe::wgpu::Surface<'_>>| {
                adapters
                    .iter()
                    .find(|a| a.get_info().device_type == eframe::wgpu::DeviceType::Cpu)
                    .cloned()
                    .ok_or_else(|| "no software (WARP) adapter".to_string())
            }));
        }
    }
    let renderer = if mode == RenderMode::OpenGl { eframe::Renderer::Glow } else { eframe::Renderer::Wgpu };
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(win::WINDOW_TITLE)
        .with_app_id("Sumiveil")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([760.0, 480.0])
        .with_icon(icon)
        .with_visible(visible)
        .with_drag_and_drop(true);
    if !visible {
        // eframe は最初の描画後に必ずウィンドウを表示するため、トレイ起動時は画面外に作ってから隠す
        viewport = viewport.with_position([-30000.0, -30000.0]);
    }
    let options = eframe::NativeOptions { wgpu_options, renderer, viewport, centered: visible, ..Default::default() };
    eframe::run_native(win::WINDOW_TITLE, options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, startup)))))
}
