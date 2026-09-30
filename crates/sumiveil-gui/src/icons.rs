//! Sumiveil 独自のアイコン。外部のアイコンフォントや画像は使わず、20×20 の格子上の線・円弧・面だけで定義し、
//! 距離関数でアンチエイリアス付きのアルファマスクに描き出す (色はテクスチャの tint で付ける)。
//! 線は太さ 1.6・丸い端で統一する。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use eframe::egui::{self, Color32, ColorImage, Rect, TextureHandle, TextureOptions, Vec2};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Icon {
    Open,
    Paste,
    Copy,
    Save,
    Clear,
    Settings,
    Veil,
    Folder,
    Info,
    Play,
    Refresh,
    Sun,
    Moon,
    Auto,
    Menu,
    Report,
    Check,
    Warning,
    Add,
    Delete,
    Edit,
    Stop,
    List,
    ChevronDown,
    ChevronUp,
    Keyboard,
    More,
    Find,
    Up,
    Down,
    Close,
    Document,
    Link,
    Tag,
}

impl Icon {
    #[allow(dead_code)]
    pub const ALL: [Icon; 34] = [
        Icon::Open, Icon::Paste, Icon::Copy, Icon::Save, Icon::Clear, Icon::Settings, Icon::Veil, Icon::Folder, Icon::Info,
        Icon::Play, Icon::Refresh, Icon::Sun, Icon::Moon, Icon::Auto, Icon::Menu, Icon::Report, Icon::Check, Icon::Warning,
        Icon::Add, Icon::Delete, Icon::Edit, Icon::Stop, Icon::List, Icon::ChevronDown, Icon::ChevronUp, Icon::Keyboard,
        Icon::More, Icon::Find, Icon::Up, Icon::Down, Icon::Close, Icon::Document, Icon::Link, Icon::Tag,
    ];
}

/// 線の太さ (格子単位)
const W: f32 = 1.6;

/// 図形の部品 (座標は 20×20 の格子、y は下向き)。
#[derive(Clone, Debug)]
enum Prim {
    /// 折れ線 (線)
    Line(Vec<(f32, f32)>),
    /// 円 (線)
    Ring(f32, f32, f32),
    /// 円弧 (線)。角度は度で、x 軸の正方向から時計回り
    Arc(f32, f32, f32, f32, f32),
    /// 塗りつぶした円
    Dot(f32, f32, f32),
    /// 角丸の四角 (線)
    Frame(f32, f32, f32, f32, f32),
    /// 角丸の四角 (面)
    Block(f32, f32, f32, f32, f32),
    /// 多角形 (面)
    Fill(Vec<(f32, f32)>),
    /// 線分を芯にした長円の輪郭 (線)
    Capsule(f32, f32, f32, f32, f32),
    /// 円から別の円を切り抜いた三日月 (面)
    Crescent(f32, f32, f32, f32, f32, f32),
}

use Prim::*;

fn shapes(icon: Icon) -> Vec<Prim> {
    match icon {
        Icon::Menu => vec![Line(vec![(4.0, 6.0), (16.0, 6.0)]), Line(vec![(4.0, 10.0), (16.0, 10.0)]), Line(vec![(4.0, 14.0), (16.0, 14.0)])],
        Icon::Folder => vec![Frame(3.0, 7.0, 17.0, 16.0, 1.6), Line(vec![(3.8, 7.0), (3.8, 5.4), (8.0, 5.4), (9.6, 7.0)])],
        Icon::Open => vec![
            Line(vec![(3.0, 11.5), (3.0, 16.0), (17.0, 16.0), (17.0, 11.5)]),
            Line(vec![(10.0, 13.0), (10.0, 3.8)]),
            Line(vec![(6.4, 7.4), (10.0, 3.8), (13.6, 7.4)]),
        ],
        Icon::Save => vec![
            Line(vec![(3.0, 11.5), (3.0, 16.0), (17.0, 16.0), (17.0, 11.5)]),
            Line(vec![(10.0, 3.6), (10.0, 12.4)]),
            Line(vec![(6.4, 8.8), (10.0, 12.4), (13.6, 8.8)]),
        ],
        Icon::Paste => vec![Frame(4.8, 4.2, 15.2, 17.2, 1.6), Block(7.4, 2.6, 12.6, 5.6, 1.0), Line(vec![(7.6, 9.2), (12.4, 9.2)]), Line(vec![(7.6, 12.6), (10.8, 12.6)])],
        Icon::Copy => vec![Frame(7.4, 7.0, 16.4, 17.0, 1.6), Line(vec![(4.0, 13.0), (4.0, 4.6), (4.6, 4.0), (12.6, 4.0)])],
        Icon::Clear => vec![
            Line(vec![(3.6, 5.6), (16.4, 5.6)]),
            Line(vec![(3.6, 10.0), (9.4, 10.0)]),
            Line(vec![(3.6, 14.4), (7.8, 14.4)]),
            Line(vec![(11.6, 10.6), (16.4, 15.4)]),
            Line(vec![(16.4, 10.6), (11.6, 15.4)]),
        ],
        Icon::Settings => vec![
            Line(vec![(3.2, 5.6), (16.8, 5.6)]),
            Line(vec![(3.2, 10.0), (16.8, 10.0)]),
            Line(vec![(3.2, 14.4), (16.8, 14.4)]),
            Dot(7.0, 5.6, 2.3),
            Dot(13.2, 10.0, 2.3),
            Dot(9.0, 14.4, 2.3),
        ],
        // 墨の帯で 1 行を覆った紙 (Sumiveil の象徴)
        Icon::Veil => vec![Line(vec![(4.0, 4.6), (14.6, 4.6)]), Block(2.8, 7.8, 17.2, 12.2, 1.2), Line(vec![(4.0, 15.4), (11.4, 15.4)])],
        Icon::Info => vec![Ring(10.0, 10.0, 7.2), Dot(10.0, 6.6, 1.15), Line(vec![(10.0, 9.4), (10.0, 13.8)])],
        Icon::Play => vec![Fill(vec![(6.6, 4.4), (15.8, 10.0), (6.6, 15.6)])],
        Icon::Refresh => vec![Arc(10.0, 10.0, 6.0, 0.0, 280.0), Line(vec![(10.4, 1.9), (12.2, 4.3), (9.7, 6.0)])],
        Icon::Sun => {
            let mut v = vec![Ring(10.0, 10.0, 3.4)];
            for k in 0..8 {
                let a = (k as f32) * std::f32::consts::FRAC_PI_4;
                let (s, c) = a.sin_cos();
                v.push(Line(vec![(10.0 + c * 5.8, 10.0 + s * 5.8), (10.0 + c * 7.6, 10.0 + s * 7.6)]));
            }
            v
        }
        Icon::Moon => vec![Crescent(10.0, 10.0, 7.0, 13.6, 6.8, 5.8)],
        Icon::Auto => vec![Ring(10.0, 10.0, 7.0), Fill(half_disc(10.0, 10.0, 7.0))],
        Icon::Report => vec![
            Line(vec![(5.0, 3.2), (11.8, 3.2), (15.0, 6.4), (15.0, 16.8), (5.0, 16.8), (5.0, 3.2)]),
            Line(vec![(11.8, 3.2), (11.8, 6.4), (15.0, 6.4)]),
            Line(vec![(7.8, 14.0), (7.8, 11.6)]),
            Line(vec![(10.0, 14.0), (10.0, 9.4)]),
            Line(vec![(12.2, 14.0), (12.2, 10.6)]),
        ],
        Icon::Document => vec![
            Line(vec![(5.0, 3.2), (11.8, 3.2), (15.0, 6.4), (15.0, 16.8), (5.0, 16.8), (5.0, 3.2)]),
            Line(vec![(11.8, 3.2), (11.8, 6.4), (15.0, 6.4)]),
            Line(vec![(7.6, 10.2), (12.4, 10.2)]),
            Line(vec![(7.6, 13.4), (11.2, 13.4)]),
        ],
        Icon::Check => vec![Line(vec![(4.4, 10.6), (8.4, 14.4), (15.6, 5.8)])],
        Icon::Warning => vec![Line(vec![(10.0, 3.4), (17.4, 16.4), (2.6, 16.4), (10.0, 3.4)]), Line(vec![(10.0, 8.2), (10.0, 11.6)]), Dot(10.0, 14.0, 1.1)],
        Icon::Add => vec![Line(vec![(10.0, 4.0), (10.0, 16.0)]), Line(vec![(4.0, 10.0), (16.0, 10.0)])],
        Icon::Delete => vec![
            Line(vec![(3.6, 5.4), (16.4, 5.4)]),
            Line(vec![(7.8, 5.4), (8.4, 3.2), (11.6, 3.2), (12.2, 5.4)]),
            Line(vec![(5.4, 5.4), (6.4, 16.6), (13.6, 16.6), (14.6, 5.4)]),
            Line(vec![(8.6, 8.6), (8.8, 13.6)]),
            Line(vec![(11.4, 8.6), (11.2, 13.6)]),
        ],
        Icon::Edit => vec![
            Line(vec![(4.0, 16.0), (4.8, 12.4), (13.0, 4.2), (15.8, 7.0), (7.6, 15.2), (4.0, 16.0)]),
            Line(vec![(11.2, 6.0), (14.0, 8.8)]),
        ],
        Icon::Stop => vec![Block(5.0, 5.0, 15.0, 15.0, 2.0)],
        Icon::List => vec![
            Dot(4.6, 5.6, 1.2),
            Dot(4.6, 10.0, 1.2),
            Dot(4.6, 14.4, 1.2),
            Line(vec![(8.0, 5.6), (16.4, 5.6)]),
            Line(vec![(8.0, 10.0), (16.4, 10.0)]),
            Line(vec![(8.0, 14.4), (14.0, 14.4)]),
        ],
        Icon::ChevronDown => vec![Line(vec![(5.0, 7.8), (10.0, 12.8), (15.0, 7.8)])],
        Icon::ChevronUp => vec![Line(vec![(5.0, 12.2), (10.0, 7.2), (15.0, 12.2)])],
        Icon::Keyboard => {
            let mut v = vec![Frame(2.4, 5.0, 17.6, 15.0, 2.0), Line(vec![(6.8, 12.2), (13.2, 12.2)])];
            for x in [5.6, 8.6, 11.4, 14.4] {
                v.push(Dot(x, 8.4, 0.95));
            }
            v
        }
        Icon::More => vec![Dot(4.6, 10.0, 1.5), Dot(10.0, 10.0, 1.5), Dot(15.4, 10.0, 1.5)],
        Icon::Find => vec![Ring(8.6, 8.6, 5.0), Line(vec![(12.4, 12.4), (16.4, 16.4)])],
        Icon::Up => vec![Line(vec![(10.0, 16.0), (10.0, 4.4)]), Line(vec![(5.6, 8.8), (10.0, 4.4), (14.4, 8.8)])],
        Icon::Down => vec![Line(vec![(10.0, 4.0), (10.0, 15.6)]), Line(vec![(5.6, 11.2), (10.0, 15.6), (14.4, 11.2)])],
        Icon::Close => vec![Line(vec![(5.2, 5.2), (14.8, 14.8)]), Line(vec![(14.8, 5.2), (5.2, 14.8)])],
        Icon::Link => vec![Capsule(5.4, 14.6, 8.4, 11.6, 2.8), Capsule(11.6, 8.4, 14.6, 5.4, 2.8), Line(vec![(8.2, 11.8), (11.8, 8.2)])],
        Icon::Tag => vec![Line(vec![(3.2, 3.2), (10.2, 3.2), (16.8, 9.8), (9.8, 16.8), (3.2, 10.2), (3.2, 3.2)]), Dot(7.0, 7.0, 1.3)],
    }
}

/// 円の左半分の多角形 (上から下へ円周をたどる)。
fn half_disc(cx: f32, cy: f32, r: f32) -> Vec<(f32, f32)> {
    (0..=24)
        .map(|i| {
            let a = std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 24.0;
            let (s, c) = a.sin_cos();
            (cx + c * r, cy - s * r)
        })
        .collect()
}

fn seg_dist(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (px, py) = (p.0 - a.0, p.1 - a.1);
    let (bx, by) = (b.0 - a.0, b.1 - a.1);
    let len2 = bx * bx + by * by;
    let t = if len2 > 0.0 { ((px * bx + py * by) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let (dx, dy) = (px - bx * t, py - by * t);
    (dx * dx + dy * dy).sqrt()
}

fn rrect_sdf(p: (f32, f32), x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> f32 {
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (hx, hy) = ((x1 - x0) / 2.0 - r, (y1 - y0) / 2.0 - r);
    let qx = (p.0 - cx).abs() - hx;
    let qy = (p.1 - cy).abs() - hy;
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - r
}

fn poly_sdf(p: (f32, f32), pts: &[(f32, f32)]) -> f32 {
    let mut d = f32::MAX;
    let mut inside = false;
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        d = d.min(seg_dist(p, a, b));
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
            inside = !inside;
        }
    }
    if inside { -d } else { d }
}

/// 点 p から図形までの符号付き距離 (負は内側)。線の図形は太さを含めた距離。
fn prim_sdf(prim: &Prim, p: (f32, f32)) -> f32 {
    let half = W / 2.0;
    let dist = |q: (f32, f32)| ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt();
    match prim {
        Line(pts) => pts.windows(2).map(|w| seg_dist(p, w[0], w[1])).fold(f32::MAX, f32::min) - half,
        Ring(cx, cy, r) => (dist((*cx, *cy)) - r).abs() - half,
        Arc(cx, cy, r, a0, a1) => {
            let mut ang = (p.1 - cy).atan2(p.0 - cx).to_degrees();
            while ang < *a0 {
                ang += 360.0;
            }
            if ang <= *a1 {
                (dist((*cx, *cy)) - r).abs() - half
            } else {
                let end = |a: f32| {
                    let (s, c) = a.to_radians().sin_cos();
                    (cx + c * r, cy + s * r)
                };
                dist(end(*a0)).min(dist(end(*a1))) - half
            }
        }
        Dot(cx, cy, r) => dist((*cx, *cy)) - r,
        Frame(x0, y0, x1, y1, r) => rrect_sdf(p, *x0, *y0, *x1, *y1, *r).abs() - half,
        Block(x0, y0, x1, y1, r) => rrect_sdf(p, *x0, *y0, *x1, *y1, *r),
        // 面の角が線の図形より細く見えないよう、少しだけ太らせる
        Fill(pts) => poly_sdf(p, pts) - 0.35,
        Capsule(ax, ay, bx, by, r) => (seg_dist(p, (*ax, *ay), (*bx, *by)) - r).abs() - half,
        Crescent(cx, cy, r, ox, oy, r2) => (dist((*cx, *cy)) - r).max(-(dist((*ox, *oy)) - r2)),
    }
}

/// アイコンを size×size のアルファ値 (0..=255) に描き出す。
pub fn rasterize(icon: Icon, size: u32) -> Vec<u8> {
    let prims = shapes(icon);
    let scale = 20.0 / size as f32;
    let mut out = vec![0u8; (size * size) as usize];
    for y in 0..size {
        for x in 0..size {
            let p = ((x as f32 + 0.5) * scale, (y as f32 + 0.5) * scale);
            let d = prims.iter().map(|pr| prim_sdf(pr, p)).fold(f32::MAX, f32::min);
            // 距離をピクセル単位に直し、境界の 1px でなめらかに変化させる
            let a = (0.5 - d / scale).clamp(0.0, 1.0);
            out[(y * size + x) as usize] = (a * 255.0).round() as u8;
        }
    }
    out
}

const TEX_SIZE: u32 = 64;

fn cache() -> &'static Mutex<HashMap<Icon, TextureHandle>> {
    static C: OnceLock<Mutex<HashMap<Icon, TextureHandle>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

fn texture(ctx: &egui::Context, icon: Icon) -> TextureHandle {
    let mut map = cache().lock().unwrap_or_else(|e| e.into_inner());
    map.entry(icon)
        .or_insert_with(|| {
            let pixels = rasterize(icon, TEX_SIZE).into_iter().map(Color32::from_white_alpha).collect();
            let img = ColorImage::new([TEX_SIZE as usize, TEX_SIZE as usize], pixels);
            let opts = TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear));
            ctx.load_texture(format!("sumiveil-icon-{icon:?}"), img, opts)
        })
        .clone()
}

/// ボタンやラベルに入れる画像 (size は論理ピクセル)。
pub fn image(ctx: &egui::Context, icon: Icon, size: f32, color: Color32) -> egui::Image<'static> {
    let tex = texture(ctx, icon);
    egui::Image::new(egui::load::SizedTexture::new(tex.id(), Vec2::splat(size))).tint(color)
}

/// center を中心に描く。
pub fn paint(painter: &egui::Painter, icon: Icon, center: egui::Pos2, size: f32, color: Color32) {
    let tex = texture(painter.ctx(), icon);
    let rect = Rect::from_center_size(center, Vec2::splat(size));
    painter.image(tex.id(), rect, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), color);
}
