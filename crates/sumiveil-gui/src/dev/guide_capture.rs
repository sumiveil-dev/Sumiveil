//! 利用ガイドの画像をアプリ自身が作る (cargo feature "guide-capture" のときだけ組み込む。配布版には入れない)。
//! 本体とは `super::wrap` (撮影の状態を持つ包み) と `guide_mark!` (部品の位置の記録) だけでつながる。
//!
//! - アプリの画面 (`--capture-shot <png> [--capture-steps <手順>]`):
//!   通常どおり起動して指定の画面を開き、アプリ内の疑似マウスイベントでホバー・クリックしてから、
//!   描画結果を PNG に保存して終了する。OS へのマウス・キー入力の送信や、画面の読み取りはしない。
//! - インストーラーの画面 (`--capture-setup <installer.exe> <出力フォルダ>`):
//!   自分で起動したインストーラーのウィンドウだけを PrintWindow で PNG にし、
//!   ボタンへの BM_CLICK メッセージで次の画面へ進める。インストールはせず、最後に終了させる。
//!
//! 手順の書式: `move:X,Y;click:@印の名前;wait:秒` (座標はウィンドウ内の論理座標。倍率 1.25 で 1280×800 の画面)。
//! `@名前` は、画面側が `guide_mark!` で記録した部品の位置 (中央) を指す。記録した位置は `<png>.json` にも書き出す
//! (画像のピクセル座標。後処理で注釈や切り抜きの位置合わせに使う)。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui;

/// 撮影時の画面の倍率とウィンドウの大きさ (論理ピクセル)。画像は 1600×1000 ピクセルになる。
const ZOOM_PPP: f32 = 1.25;
const WINDOW_SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);
/// 起動してから操作を始めるまでの待ち時間 (秒)
const SETTLE: f64 = 2.0;

#[derive(Clone, Debug)]
enum Target {
    Pos(egui::Pos2),
    Mark(String),
}

#[derive(Clone, Debug)]
enum Step {
    Move(Target),
    Click(Target),
    Wait(f64),
}

/// 画面側が記録した部品の位置 (名前 → 論理座標の矩形)。
#[derive(Clone, Default)]
struct Marks(BTreeMap<String, egui::Rect>);

fn marks_id() -> egui::Id {
    egui::Id::new("sumiveil-guide-marks")
}

/// 撮影で操作・切り抜きに使う部品の位置を記録する (`guide_mark!` から呼ばれる)。
pub fn mark(ctx: &egui::Context, name: impl Into<String>, rect: egui::Rect) {
    let name = name.into();
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Marks>(marks_id()).0.insert(name, rect);
    });
}

fn resolve(ctx: &egui::Context, t: &Target) -> Option<egui::Pos2> {
    match t {
        Target::Pos(p) => Some(*p),
        Target::Mark(name) => ctx.data(|d| d.get_temp::<Marks>(marks_id())).and_then(|m| m.0.get(name).map(|r| r.center())),
    }
}

pub struct Shot {
    out: PathBuf,
    steps: Vec<Step>,
    next: usize,
    start: Option<Instant>,
    wait_until: f64,
    release_at: Option<egui::Pos2>,
    /// 印が見つからない手順を待ち始めた時刻
    waiting_since: Option<f64>,
    stage: u8,
    requested: bool,
    saved: bool,
}

fn parse_target(s: &str) -> Option<Target> {
    if let Some(name) = s.trim().strip_prefix('@') {
        return Some(Target::Mark(name.to_string()));
    }
    let (x, y) = s.split_once(',')?;
    Some(Target::Pos(egui::pos2(x.trim().parse().ok()?, y.trim().parse().ok()?)))
}

/// 起動引数で指定された撮影 (出力先の PNG, 手順)。`take_args` が記録し、`Shot::from_args` が使う。
static SHOT_ARGS: std::sync::OnceLock<(PathBuf, String)> = std::sync::OnceLock::new();

/// 撮影用の引数を取り除いた残りを返す。インストーラーの撮影なら、ここで終了する (GUI は起動しない)。
/// - `--capture-shot <png>` / `--capture-steps <手順>`: アプリの画面
/// - `--capture-setup <installer.exe> <出力フォルダ>` / `--capture-upgrade <installer.exe> <出力フォルダ>`: インストーラーの画面
pub fn take_args(args: Vec<std::ffi::OsString>) -> Vec<std::ffi::OsString> {
    fn value(it: &mut std::vec::IntoIter<std::ffi::OsString>) -> std::ffi::OsString {
        it.next().unwrap_or_default()
    }
    let mut rest = vec![];
    let (mut out, mut steps) = (None::<PathBuf>, String::new());
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.to_str() {
            Some("--capture-shot") => out = Some(PathBuf::from(value(&mut it))),
            Some("--capture-steps") => steps = value(&mut it).into_string().unwrap_or_default(),
            Some(k @ ("--capture-setup" | "--capture-upgrade")) => {
                let installer = PathBuf::from(value(&mut it));
                let dir = PathBuf::from(value(&mut it));
                let code = if k == "--capture-setup" { capture_setup(&installer, &dir) } else { capture_upgrade(&installer, &dir) };
                std::process::exit(code);
            }
            _ => rest.push(a),
        }
    }
    if let Some(out) = out {
        let _ = SHOT_ARGS.set((out, steps));
    }
    rest
}

impl Shot {
    /// 起動引数で撮影が指定されていれば作る。
    pub fn from_args() -> Option<Self> {
        SHOT_ARGS.get().map(|(out, steps)| Self::new(out.clone(), steps))
    }

    pub fn new(out: PathBuf, steps: &str) -> Self {
        let steps = steps
            .split(';')
            .filter_map(|s| {
                let (k, v) = s.trim().split_once(':')?;
                match k {
                    "move" => parse_target(v).map(Step::Move),
                    "click" => parse_target(v).map(Step::Click),
                    "wait" => v.trim().parse().ok().map(Step::Wait),
                    _ => None,
                }
            })
            .collect();
        Self { out, steps, next: 0, start: None, wait_until: 0.0, release_at: None, waiting_since: None, stage: 0, requested: false, saved: false }
    }

    fn elapsed(&mut self) -> f64 {
        self.start.get_or_insert_with(Instant::now).elapsed().as_secs_f64()
    }

    /// 毎フレームの入力に、手順どおりの疑似マウスイベントを足す。
    pub fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        ctx.request_repaint();
        // 倍率を固定してから、ウィンドウの大きさを合わせる (egui の座標で指定するため順に行う)
        match self.stage {
            0 => {
                let native = ctx.native_pixels_per_point().unwrap_or(1.0);
                ctx.set_zoom_factor(ZOOM_PPP / native);
                self.stage = 1;
                return;
            }
            1 => {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(WINDOW_SIZE));
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(0.0, 0.0)));
                self.stage = 2;
                return;
            }
            _ => {}
        }
        let now = self.elapsed();
        if now < SETTLE {
            return;
        }
        let modifiers = egui::Modifiers::default();
        if let Some(pos) = self.release_at.take() {
            raw.events.push(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers });
            self.wait_until = now + 0.6;
            return;
        }
        if now < self.wait_until {
            return;
        }
        if let Some(step) = self.steps.get(self.next).cloned() {
            let target = match &step {
                Step::Move(t) | Step::Click(t) => Some(t.clone()),
                Step::Wait(_) => None,
            };
            let pos = target.as_ref().and_then(|t| resolve(ctx, t));
            if target.is_some() && pos.is_none() {
                // 印がまだ記録されていない (画面の準備中): 5 秒まで待ち、それでも無ければ飛ばす
                let since = *self.waiting_since.get_or_insert(now);
                if now - since < 5.0 {
                    return;
                }
                eprintln!("guide-capture: 位置が見つからないため飛ばします: {step:?}");
            }
            self.waiting_since = None;
            self.next += 1;
            match (step, pos) {
                (Step::Move(_), Some(pos)) => {
                    raw.events.push(egui::Event::PointerMoved(pos));
                    self.wait_until = now + 0.5;
                }
                (Step::Click(_), Some(pos)) => {
                    raw.events.push(egui::Event::PointerMoved(pos));
                    raw.events.push(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers });
                    self.release_at = Some(pos);
                }
                (Step::Wait(sec), _) => self.wait_until = now + sec,
                // 位置が見つからなかった手順は飛ばす
                _ => {}
            }
            return;
        }
        if !self.requested && now >= self.wait_until + 0.8 {
            self.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
    }

    /// 描画結果が届いたら PNG に保存して終了する。
    pub fn logic(&mut self, ctx: &egui::Context) {
        if self.saved {
            return;
        }
        let image = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            let [w, h] = image.size;
            let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            if let Err(e) = write_png(&self.out, w as u32, h as u32, &rgba) {
                eprintln!("{}: {e}", self.out.display());
            }
            // 記録した部品の位置を、画像のピクセル座標で書き出す
            let ppp = ctx.pixels_per_point();
            let marks = ctx.data(|d| d.get_temp::<Marks>(marks_id())).unwrap_or_default();
            let json: BTreeMap<&String, [f32; 4]> =
                marks.0.iter().map(|(k, r)| (k, [r.min.x * ppp, r.min.y * ppp, r.max.x * ppp, r.max.y * ppp])).collect();
            let _ = std::fs::write(self.out.with_extension("json"), serde_json::to_string_pretty(&json).unwrap_or_default());
            self.saved = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn write_png(path: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgba).map_err(|e| e.to_string())
}

// ───────────────────────── インストーラーの画面 ─────────────────────────

mod setup {
    use super::write_png;
    use std::path::Path;
    use std::time::{Duration, Instant};
    use windows::core::BOOL;
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT, WPARAM};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS,
    };
    use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, EnumWindows, GetClassNameW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, SendMessageW, BM_CLICK,
    };

    fn text(hwnd: HWND) -> String {
        let mut buf = [0u16; 512];
        let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    fn class_name(hwnd: HWND) -> String {
        let mut buf = [0u16; 256];
        let n = unsafe { GetClassNameW(hwnd, &mut buf) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    /// 表示中のボタン (ラジオボタンを含む) で、表示文字 (& を除く) が prefix で始まるもの。
    fn find_button(hwnd: HWND, prefix: &str) -> Option<HWND> {
        children(hwnd).into_iter().find(|&c| {
            let visible = unsafe { IsWindowVisible(c).as_bool() };
            visible && class_name(c).contains("Button") && text(c).replace('&', "").starts_with(prefix)
        })
    }

    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let v = unsafe { &mut *(lparam.0 as *mut Vec<HWND>) };
        v.push(hwnd);
        BOOL(1)
    }

    fn top_windows() -> Vec<HWND> {
        let mut v: Vec<HWND> = vec![];
        unsafe {
            let _ = EnumWindows(Some(collect), LPARAM(&mut v as *mut _ as isize));
        }
        v.into_iter().filter(|&h| unsafe { IsWindowVisible(h).as_bool() }).collect()
    }

    fn children(hwnd: HWND) -> Vec<HWND> {
        let mut v: Vec<HWND> = vec![];
        unsafe {
            let _ = EnumChildWindows(Some(hwnd), Some(collect), LPARAM(&mut v as *mut _ as isize));
        }
        v
    }

    /// タイトルが条件に合う表示中のウィンドウを待つ。
    fn wait_window(pred: impl Fn(&str) -> bool, timeout: Duration) -> Result<HWND, String> {
        let t0 = Instant::now();
        while t0.elapsed() < timeout {
            if let Some(h) = top_windows().into_iter().find(|&h| pred(&text(h))) {
                std::thread::sleep(Duration::from_millis(900));
                return Ok(h);
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        Err("インストーラーのウィンドウが見つかりません".into())
    }

    /// 表示文字 (& を除く) が prefix で始まるボタンに BM_CLICK を送る。
    fn click(hwnd: HWND, prefix: &str) -> Result<(), String> {
        let b = find_button(hwnd, prefix).ok_or_else(|| format!("ボタン「{prefix}」が見つかりません"))?;
        unsafe {
            SendMessageW(b, BM_CLICK, Some(WPARAM(0)), Some(LPARAM(0)));
        }
        std::thread::sleep(Duration::from_millis(1200));
        Ok(())
    }

    /// ウィンドウを PNG に保存する (影を除いた見た目の枠で切り抜く)。
    fn shot(hwnd: HWND, path: &Path) -> Result<(), String> {
        unsafe {
            let mut r = RECT::default();
            GetWindowRect(hwnd, &mut r).map_err(|e| e.to_string())?;
            let (w, h) = (r.right - r.left, r.bottom - r.top);
            let screen = GetDC(None);
            let mem = CreateCompatibleDC(Some(screen));
            let bmp = CreateCompatibleBitmap(screen, w, h);
            let old = SelectObject(mem, bmp.into());
            // 2 = PW_RENDERFULLCONTENT (ダーク表示などの描画も含める)
            let _ = PrintWindow(hwnd, mem, PRINT_WINDOW_FLAGS(2));
            let mut bi = BITMAPINFO::default();
            bi.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            };
            let mut bgra = vec![0u8; (w * h * 4) as usize];
            GetDIBits(mem, bmp, 0, h as u32, Some(bgra.as_mut_ptr() as *mut _), &mut bi, DIB_RGB_COLORS);
            SelectObject(mem, old);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(mem);
            ReleaseDC(None, screen);
            // 見た目の枠 (Windows 11 の影の分を除く)
            let mut f = RECT::default();
            let (mut x0, mut y0, mut x1, mut y1) = (0, 0, w, h);
            if DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, &mut f as *mut _ as *mut _, std::mem::size_of::<RECT>() as u32).is_ok() {
                x0 = (f.left - r.left).clamp(0, w);
                y0 = (f.top - r.top).clamp(0, h);
                x1 = (f.right - r.left).clamp(x0, w);
                y1 = (f.bottom - r.top).clamp(y0, h);
            }
            let (cw, ch) = (x1 - x0, y1 - y0);
            let mut rgba = Vec::with_capacity((cw * ch * 4) as usize);
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = ((y * w + x) * 4) as usize;
                    rgba.extend_from_slice(&[bgra[i + 2], bgra[i + 1], bgra[i], 255]);
                }
            }
            write_png(path, cw as u32, ch as u32, &rgba)
        }
    }

    /// ウィンドウの持ち主のプロセスを終了する (インストールを始める前に止めるため)。
    fn terminate_owner(hwnd: HWND) {
        unsafe {
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if let Ok(h) = OpenProcess(PROCESS_TERMINATE, false, pid) {
                let _ = TerminateProcess(h, 1);
                let _ = CloseHandle(h);
            }
        }
        std::thread::sleep(Duration::from_millis(1500));
    }

    pub fn run(installer: &Path, out: &Path) -> Result<(), String> {
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
        let wait = Duration::from_secs(60);
        // 1. インストールモードの選択 (撮ったら終了する)
        let mut child = std::process::Command::new(installer).arg("/LANG=ja").spawn().map_err(|e| e.to_string())?;
        let h = wait_window(|t| t.contains("インストールモード"), wait)?;
        shot(h, &out.join("setup-mode.png"))?;
        terminate_owner(h);
        let _ = child.wait();
        // 2. 現在のユーザー用で起動し直し、使用許諾 → インストール先 → コンポーネント → 追加タスク
        let mut child = std::process::Command::new(installer).args(["/LANG=ja", "/CURRENTUSER"]).spawn().map_err(|e| e.to_string())?;
        let h = wait_window(|t| t.starts_with("Sumiveil") && t.contains("セットアップ"), wait)?;
        let result = (|| {
            click(h, "同意する")?;
            click(h, "次へ")?; // 使用許諾
            click(h, "次へ")?; // インストール先
            shot(h, &out.join("setup-components.png"))?;
            click(h, "次へ")?;
            shot(h, &out.join("setup-tasks.png"))
        })();
        terminate_owner(h);
        let _ = child.wait();
        result
    }

    /// 動作確認用: インストール済みの環境で新しいインストーラーを実行し、上書き更新の各画面を撮りながら最後まで進める。
    pub fn run_upgrade(installer: &Path, out: &Path) -> Result<(), String> {
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
        let mut child = std::process::Command::new(installer).arg("/LANG=ja").spawn().map_err(|e| e.to_string())?;
        let h = wait_window(|t| t.starts_with("Sumiveil") && t.contains("セットアップ"), Duration::from_secs(60))?;
        // 準備完了の画面 (「インストール」ボタン) まで、各画面を撮りながら進める
        for i in 1..=6 {
            shot(h, &out.join(format!("upgrade-{i}.png")))?;
            if click(h, "インストール").is_ok() {
                break;
            }
            click(h, "次へ")?;
        }
        // 完了の画面を待って撮る
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(120) {
            if find_button(h, "完了").is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        std::thread::sleep(Duration::from_millis(800));
        shot(h, &out.join("upgrade-finished.png"))?;
        click(h, "完了")?;
        let _ = child.wait();
        Ok(())
    }
}

/// `--capture-upgrade` の処理 (動作確認用。GUI は起動しない)。
fn capture_upgrade(installer: &Path, out: &Path) -> i32 {
    match setup::run_upgrade(installer, out) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("capture-upgrade: {e}");
            1
        }
    }
}

/// `--capture-setup` の処理 (GUI は起動しない)。
fn capture_setup(installer: &Path, out: &Path) -> i32 {
    match setup::run(installer, out) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("capture-setup: {e}");
            std::thread::sleep(Duration::from_millis(10));
            1
        }
    }
}
