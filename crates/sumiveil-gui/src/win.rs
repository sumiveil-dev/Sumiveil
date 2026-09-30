//! Windows 固有の処理 (アクセントカラー、タイトルバーのダークモード、ウィンドウ表示、多重起動防止、トースト通知)。

use eframe::egui::Color32;
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Shell::{DefSubclassProc, SetCurrentProcessExplicitAppUserModelID, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, IsIconic, IsWindowVisible, SetForegroundWindow, ShowWindow, SWP_NOSIZE, SW_HIDE, SW_RESTORE, SW_SHOW, WINDOWPOS, WM_WINDOWPOSCHANGING,
};

pub const AUMID: &str = "Sumiveil.Sumiveil";
pub const WINDOW_TITLE: &str = "Sumiveil";

fn hwnd(h: isize) -> HWND {
    HWND(h as *mut core::ffi::c_void)
}

/// ウィンドウの幅・高さの上限 (物理ピクセル)。
/// DirectX (wgpu) は 8192 を超える描画面を作れずにアプリが終了し、OpenGL も 16384 を超えると描画が崩れるため。
const MAX_WINDOW_PX: i32 = 8000;

unsafe extern "system" fn size_limit_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _id: usize, _data: usize) -> LRESULT {
    if msg == WM_WINDOWPOSCHANGING {
        // Safety: WM_WINDOWPOSCHANGING の lParam は WINDOWPOS へのポインタ (書き換えると実際のサイズに反映される)
        if let Some(pos) = unsafe { (lp.0 as *mut WINDOWPOS).as_mut() } {
            if (pos.flags & SWP_NOSIZE).0 == 0 {
                pos.cx = pos.cx.min(MAX_WINDOW_PX);
                pos.cy = pos.cy.min(MAX_WINDOW_PX);
            }
        }
    }
    unsafe { DefSubclassProc(h, msg, wp, lp) }
}

/// エラーのメッセージボックス (診断レポートの案内に使う)。
pub fn error_box(title: &str, body: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK, MB_SETFOREGROUND};
    let (t, b) = (HSTRING::from(title), HSTRING::from(body));
    unsafe {
        MessageBoxW(None, PCWSTR(b.as_ptr()), PCWSTR(t.as_ptr()), MB_OK | MB_ICONERROR | MB_SETFOREGROUND);
    }
}

/// ウィンドウが上限より大きくならないようにする。マウスでの操作だけでなく、ほかのアプリ
/// (ウィンドウ整理ツールなど) からのサイズ変更も含めて、変更される直前に大きさを抑える。
/// 複数のモニターにまたがって広げたときなどに備える (1 台のモニターでは 8K でも届かない)。
pub fn limit_window_size(h: isize) {
    if h == 0 {
        return;
    }
    unsafe {
        let _ = SetWindowSubclass(hwnd(h), Some(size_limit_proc), 1, 0);
    }
}

/// Windows のアクセントカラー (HKCU\Software\Microsoft\Windows\DWM\AccentColor, ABGR)。
pub fn accent_color() -> Option<Color32> {
    let mut data: u32 = 0;
    let mut size: u32 = 4;
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\DWM"),
            w!("AccentColor"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };
    if r.is_ok() {
        Some(Color32::from_rgb(data as u8, (data >> 8) as u8, (data >> 16) as u8))
    } else {
        None
    }
}

/// タイトルバーをアプリのテーマ (ライト / ダーク) に合わせる。色そのものは Windows の標準のまま。
pub fn set_dark_caption(h: isize, dark: bool) {
    if h == 0 {
        return;
    }
    unsafe {
        let dark_flag: i32 = dark as i32;
        let _ = DwmSetWindowAttribute(hwnd(h), DWMWA_USE_IMMERSIVE_DARK_MODE, &dark_flag as *const i32 as _, 4);
    }
}

pub fn show_window(h: isize) {
    if h == 0 {
        return;
    }
    unsafe {
        let w = hwnd(h);
        if IsIconic(w).as_bool() {
            let _ = ShowWindow(w, SW_RESTORE);
        } else {
            let _ = ShowWindow(w, SW_SHOW);
        }
        let _ = SetForegroundWindow(w);
    }
}

pub fn hide_window(h: isize) {
    if h != 0 {
        unsafe {
            let _ = ShowWindow(hwnd(h), SW_HIDE);
        }
    }
}

pub fn is_visible(h: isize) -> bool {
    h != 0 && unsafe { IsWindowVisible(hwnd(h)).as_bool() }
}

/// 多重起動の防止。既に起動していればそのウィンドウを前面に出して false を返す。
pub fn acquire_single_instance() -> bool {
    unsafe {
        let _handle = CreateMutexW(None, true, w!("Local\\Sumiveil.SingleInstance"));
        if GetLastError() == ERROR_ALREADY_EXISTS {
            if let Ok(w) = FindWindowW(PCWSTR::null(), &HSTRING::from(WINDOW_TITLE)) {
                show_window(w.0 as isize);
            }
            return false;
        }
        // ミューテックスはプロセス終了まで保持する
        std::mem::forget(_handle);
    }
    true
}

pub fn set_app_id() {
    unsafe {
        let _ = SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(AUMID));
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// トースト通知 (インストーラーで作成したショートカットの AppUserModelID を使う)。失敗しても無視してよい。
pub fn toast(title: &str, body: &str) -> windows::core::Result<()> {
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
    let doc = XmlDocument::new()?;
    doc.LoadXml(&HSTRING::from(format!(
        "<toast duration=\"short\"><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual><audio silent=\"true\"/></toast>",
        xml_escape(title),
        xml_escape(body)
    )))?;
    let toast = ToastNotification::CreateToastNotification(&doc)?;
    ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(AUMID))?.Show(&toast)
}

/// フォルダやファイルをエクスプローラーで開く。
pub fn open_in_explorer(path: &std::path::Path) {
    let _ = std::process::Command::new("explorer.exe").arg(path).spawn();
}

/// ファイルの場所をエクスプローラーで選択状態で開く。
pub fn reveal_in_explorer(path: &std::path::Path) {
    let _ = std::process::Command::new("explorer.exe").arg(format!("/select,{}", path.display())).spawn();
}
