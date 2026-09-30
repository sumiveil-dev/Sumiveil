//! アプリアイコンをコードで描画する (画像ファイル不要)。
//! 和紙色の角丸の紙に文字行を並べ、中段の 1 行を墨の筆の帯で覆い、右下に朱の印を添えたデザイン。

/// RGBA (size x size) のピクセル列を返す。
pub fn render(size: u32) -> Vec<u8> {
    let s = size as f32;
    let mut px = vec![0u8; (size * size * 4) as usize];
    const SS: u32 = 4; // スーパーサンプリング
    for y in 0..size {
        for x in 0..size {
            let mut acc = [0f32; 4];
            for sy in 0..SS {
                for sx in 0..SS {
                    let u = (x as f32 + (sx as f32 + 0.5) / SS as f32) / s;
                    let v = (y as f32 + (sy as f32 + 0.5) / SS as f32) / s;
                    let c = sample(u, v);
                    // 乗算済みで加算
                    acc[0] += c[0] * c[3];
                    acc[1] += c[1] * c[3];
                    acc[2] += c[2] * c[3];
                    acc[3] += c[3];
                }
            }
            let n = (SS * SS) as f32;
            let a = acc[3] / n;
            let i = ((y * size + x) * 4) as usize;
            if a > 0.0 {
                px[i] = (acc[0] / acc[3] * 255.0).round() as u8;
                px[i + 1] = (acc[1] / acc[3] * 255.0).round() as u8;
                px[i + 2] = (acc[2] / acc[3] * 255.0).round() as u8;
            }
            px[i + 3] = (a * 255.0).round() as u8;
        }
    }
    px
}

fn rounded_rect(u: f32, v: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> bool {
    let cx = u.clamp(x0 + r, x1 - r);
    let cy = v.clamp(y0 + r, y1 - r);
    let dx = u - cx;
    let dy = v - cy;
    u >= x0 && u <= x1 && v >= y0 && v <= y1 && dx * dx + dy * dy <= r * r
}

fn rgb(v: u32) -> [f32; 3] {
    [((v >> 16) & 0xFF) as f32 / 255.0, ((v >> 8) & 0xFF) as f32 / 255.0, (v & 0xFF) as f32 / 255.0]
}

/// 点 (u, v) から線分 a-b への距離と、線分上の位置 t (0..1)。
fn seg(u: f32, v: f32, a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    let (px, py, bx, by) = (u - a.0, v - a.1, b.0 - a.0, b.1 - a.1);
    let t = ((px * bx + py * by) / (bx * bx + by * by)).clamp(0.0, 1.0);
    (((px - bx * t).powi(2) + (py - by * t).powi(2)).sqrt(), t)
}

/// 単位正方形上の色 (RGBA 0..1)。
fn sample(u: f32, v: f32) -> [f32; 4] {
    if !rounded_rect(u, v, 0.04, 0.04, 0.96, 0.96, 0.16) {
        return [0.0; 4];
    }
    let paper = rgb(0xF4F1EA);
    let edge = rgb(0xCFC6B4);
    let line_c = rgb(0x8A8276);
    let sumi = rgb(0x1E1C1A);
    let shu = rgb(0xB7412C);
    let with = |c: [f32; 3]| [c[0], c[1], c[2], 1.0];
    // 朱の印 (右下)
    if rounded_rect(u, v, 0.66, 0.66, 0.82, 0.82, 0.025) {
        return with(shu);
    }
    // 中段: 墨の筆の帯 (両端が細くなる、わずかに右上がり)
    let (d, t) = seg(u, v, (0.17, 0.53), (0.83, 0.47));
    let r = 0.092 * (0.62 + 0.38 * (std::f32::consts::PI * t).sin());
    if d <= r {
        return with(sumi);
    }
    // 上段・下段の文字行
    let line = |y: f32, x1: f32| rounded_rect(u, v, 0.20, y - 0.032, x1, y + 0.032, 0.032);
    if line(0.28, 0.76) || line(0.72, 0.56) {
        return with(line_c);
    }
    // 紙の縁 (暗い背景の上でも輪郭が分かるように)
    if !rounded_rect(u, v, 0.065, 0.065, 0.935, 0.935, 0.135) {
        return with(edge);
    }
    with(paper)
}

/// 複数サイズを含む .ico ファイルのバイト列 (32bit BMP 形式)。
#[allow(dead_code)]
pub fn ico_bytes(sizes: &[u32]) -> Vec<u8> {
    let mut images: Vec<Vec<u8>> = vec![];
    for &sz in sizes {
        let rgba = render(sz);
        let mut bmp = Vec::new();
        // BITMAPINFOHEADER (高さは XOR + AND マスクで 2 倍)
        bmp.extend_from_slice(&40u32.to_le_bytes());
        bmp.extend_from_slice(&(sz as i32).to_le_bytes());
        bmp.extend_from_slice(&((sz * 2) as i32).to_le_bytes());
        bmp.extend_from_slice(&1u16.to_le_bytes());
        bmp.extend_from_slice(&32u16.to_le_bytes());
        bmp.extend_from_slice(&0u32.to_le_bytes());
        bmp.extend_from_slice(&(sz * sz * 4).to_le_bytes());
        bmp.extend_from_slice(&[0u8; 16]);
        // ピクセル (下から上, BGRA)
        for y in (0..sz).rev() {
            for x in 0..sz {
                let i = ((y * sz + x) * 4) as usize;
                bmp.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
            }
        }
        // AND マスク (全て 0、行は 4 バイト境界)
        let row = sz.div_ceil(32) * 4;
        bmp.extend(std::iter::repeat_n(0u8, (row * sz) as usize));
        images.push(bmp);
    }
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (i, &sz) in sizes.iter().enumerate() {
        let b = if sz >= 256 { 0 } else { sz as u8 };
        out.extend_from_slice(&[b, b, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(images[i].len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += images[i].len() as u32;
    }
    for img in images {
        out.extend_from_slice(&img);
    }
    out
}
