//! assets/sumiveil.ico を生成する: cargo run -p sumiveil-gui --example make-icon

#[path = "../src/icon.rs"]
mod icon;

fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    std::fs::create_dir_all(&root).unwrap();
    let ico = icon::ico_bytes(&[16, 20, 24, 32, 40, 48, 64, 128, 256]);
    std::fs::write(root.join("sumiveil.ico"), ico).unwrap();
    // インストーラーのウィザード用の小さな画像 (BMP)
    for (name, size) in [("wizard-small.bmp", 58u32), ("wizard-small-dark.bmp", 58u32)] {
        let rgba = icon::render(size);
        let dark = name.contains("dark");
        let bg: [u8; 3] = if dark { [0x20, 0x20, 0x20] } else { [0xFF, 0xFF, 0xFF] };
        let row = (size * 3).div_ceil(4) * 4;
        let mut bmp = Vec::new();
        let data_size = row * size;
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&(54 + data_size).to_le_bytes());
        bmp.extend_from_slice(&0u32.to_le_bytes());
        bmp.extend_from_slice(&54u32.to_le_bytes());
        bmp.extend_from_slice(&40u32.to_le_bytes());
        bmp.extend_from_slice(&(size as i32).to_le_bytes());
        bmp.extend_from_slice(&(size as i32).to_le_bytes());
        bmp.extend_from_slice(&1u16.to_le_bytes());
        bmp.extend_from_slice(&24u16.to_le_bytes());
        bmp.extend_from_slice(&[0u8; 24]);
        for y in (0..size).rev() {
            let mut line = Vec::new();
            for x in 0..size {
                let i = ((y * size + x) * 4) as usize;
                let a = rgba[i + 3] as u32;
                let blend = |c: u8, b: u8| ((c as u32 * a + b as u32 * (255 - a)) / 255) as u8;
                line.extend_from_slice(&[blend(rgba[i + 2], bg[2]), blend(rgba[i + 1], bg[1]), blend(rgba[i], bg[0])]);
            }
            line.resize(row as usize, 0);
            bmp.extend_from_slice(&line);
        }
        std::fs::write(root.join(name), bmp).unwrap();
    }
    // インストーラーのようこそ・完了画面の左の画像 (BMP)。和紙 (ダークは墨) の地に、アイコンと朱の細い線
    for (name, dark) in [("wizard-large.bmp", false), ("wizard-large-dark.bmp", true)] {
        let (w, h) = (328u32, 628u32);
        let bg: [u8; 3] = if dark { [0x17, 0x15, 0x13] } else { [0xEF, 0xEB, 0xE2] };
        let shu: [u8; 3] = if dark { [0xE0, 0x70, 0x5A] } else { [0xB7, 0x41, 0x2C] };
        let size = 160u32;
        let rgba = icon::render(size);
        let (ix, iy) = ((w - size) / 2, 190u32);
        let row = (w * 3).div_ceil(4) * 4;
        let mut bmp = Vec::new();
        let data_size = row * h;
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&(54 + data_size).to_le_bytes());
        bmp.extend_from_slice(&0u32.to_le_bytes());
        bmp.extend_from_slice(&54u32.to_le_bytes());
        bmp.extend_from_slice(&40u32.to_le_bytes());
        bmp.extend_from_slice(&(w as i32).to_le_bytes());
        bmp.extend_from_slice(&(h as i32).to_le_bytes());
        bmp.extend_from_slice(&1u16.to_le_bytes());
        bmp.extend_from_slice(&24u16.to_le_bytes());
        bmp.extend_from_slice(&[0u8; 24]);
        for y in (0..h).rev() {
            let mut line = Vec::new();
            for x in 0..w {
                let mut c = bg;
                // アイコンの下に朱の細い線
                if y >= iy + size + 36 && y < iy + size + 40 && x >= w / 2 - 28 && x < w / 2 + 28 {
                    c = shu;
                }
                if x >= ix && x < ix + size && y >= iy && y < iy + size {
                    let i = (((y - iy) * size + (x - ix)) * 4) as usize;
                    let a = rgba[i + 3] as u32;
                    for k in 0..3 {
                        c[k] = ((rgba[i + k] as u32 * a + c[k] as u32 * (255 - a)) / 255) as u8;
                    }
                }
                line.extend_from_slice(&[c[2], c[1], c[0]]);
            }
            line.resize(row as usize, 0);
            bmp.extend_from_slice(&line);
        }
        std::fs::write(root.join(name), bmp).unwrap();
    }
    println!("written to {}", root.display());
}
