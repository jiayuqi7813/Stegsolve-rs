//! 双图合成。
//!
//! 对照原版 `CombineTransform.java` 修复：
//! - **C1（高危）**：原 Rust 只实现了 0–4 五种模式，SUB / SUB(分通道) / MUL /
//!   MUL(分通道) / Lightest / Darkest 六种全部退化成"显示图1"。本实现按原版
//!   `comb()` 补齐全部 13 种模式。
//! - **C2**：ADD（模式 3）改为与原版一致的"整型打包相加"（通道间进位会串色），
//!   与 ADD(分通道) 形成区分；后者为逐通道 mod 256。

use image::{Rgba, RgbaImage};

/// 13 种合成模式（编号与原版一致 0..=12）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombineMode {
    Xor,
    Or,
    And,
    Add,
    AddSeparate,
    Sub,
    SubSeparate,
    Mul,
    MulSeparate,
    Lightest,
    Darkest,
    HorizontalInterlace,
    VerticalInterlace,
}

pub const NUM_MODES: i32 = 13;

impl CombineMode {
    pub fn from_index(i: i32) -> CombineMode {
        match i.rem_euclid(NUM_MODES) {
            0 => CombineMode::Xor,
            1 => CombineMode::Or,
            2 => CombineMode::And,
            3 => CombineMode::Add,
            4 => CombineMode::AddSeparate,
            5 => CombineMode::Sub,
            6 => CombineMode::SubSeparate,
            7 => CombineMode::Mul,
            8 => CombineMode::MulSeparate,
            9 => CombineMode::Lightest,
            10 => CombineMode::Darkest,
            11 => CombineMode::HorizontalInterlace,
            _ => CombineMode::VerticalInterlace,
        }
    }

    pub fn index(self) -> i32 {
        match self {
            CombineMode::Xor => 0,
            CombineMode::Or => 1,
            CombineMode::And => 2,
            CombineMode::Add => 3,
            CombineMode::AddSeparate => 4,
            CombineMode::Sub => 5,
            CombineMode::SubSeparate => 6,
            CombineMode::Mul => 7,
            CombineMode::MulSeparate => 8,
            CombineMode::Lightest => 9,
            CombineMode::Darkest => 10,
            CombineMode::HorizontalInterlace => 11,
            CombineMode::VerticalInterlace => 12,
        }
    }

    pub fn text(self) -> &'static str {
        match self {
            CombineMode::Xor => "XOR",
            CombineMode::Or => "OR",
            CombineMode::And => "AND",
            CombineMode::Add => "ADD",
            CombineMode::AddSeparate => "ADD (R,G,B separate)",
            CombineMode::Sub => "SUB",
            CombineMode::SubSeparate => "SUB (R,G,B separate)",
            CombineMode::Mul => "MUL",
            CombineMode::MulSeparate => "MUL (R,G,B separate)",
            CombineMode::Lightest => "Lightest (R,G,B separate)",
            CombineMode::Darkest => "Darkest (R,G,B separate)",
            CombineMode::HorizontalInterlace => "Horizontal Interlace",
            CombineMode::VerticalInterlace => "Vertical Interlace",
        }
    }
}

#[inline]
fn argb(p: &Rgba<u8>) -> u32 {
    ((p[3] as u32) << 24) | ((p[0] as u32) << 16) | ((p[1] as u32) << 8) | (p[2] as u32)
}

#[inline]
fn rgb_pixel(col: u32) -> Rgba<u8> {
    Rgba([(col >> 16) as u8, (col >> 8) as u8, (col & 0xff) as u8, 255])
}

/// 按模式合成两个像素（ARGB u32 输入，返回 24 位 RGB）。逐行对齐 Java `comb()`。
fn comb(c1: u32, c2: u32, mode: CombineMode) -> u32 {
    match mode {
        CombineMode::Xor => (c1 ^ c2) & 0xff_ffff,
        CombineMode::Or => (c1 | c2) & 0xff_ffff,
        CombineMode::And => (c1 & c2) & 0xff_ffff,
        CombineMode::Add => c1.wrapping_add(c2) & 0xff_ffff,
        CombineMode::AddSeparate => {
            let r = (c1 & 0xff0000).wrapping_add(c2 & 0xff0000) & 0xff0000;
            let g = (c1 & 0xff00).wrapping_add(c2 & 0xff00) & 0xff00;
            let b = (c1 & 0xff).wrapping_add(c2 & 0xff) & 0xff;
            r | g | b
        }
        CombineMode::Sub => c1.wrapping_sub(c2) & 0xff_ffff,
        CombineMode::SubSeparate => {
            let r = (c1 & 0xff0000).wrapping_sub(c2 & 0xff0000) & 0xff0000;
            let g = (c1 & 0xff00).wrapping_sub(c2 & 0xff00) & 0xff00;
            let b = (c1 & 0xff).wrapping_sub(c2 & 0xff) & 0xff;
            r | g | b
        }
        CombineMode::Mul => c1.wrapping_mul(c2) & 0xff_ffff,
        CombineMode::MulSeparate => {
            let r = (((c1 & 0xff0000) >> 16).wrapping_mul((c2 & 0xff0000) >> 16) & 0xff) << 16;
            let g = (((c1 & 0xff00) >> 8).wrapping_mul((c2 & 0xff00) >> 8) & 0xff) << 8;
            let b = (c1 & 0xff).wrapping_mul(c2 & 0xff) & 0xff;
            r | g | b
        }
        CombineMode::Lightest => {
            let r = (c1 & 0xff0000).max(c2 & 0xff0000);
            let g = (c1 & 0xff00).max(c2 & 0xff00);
            let b = (c1 & 0xff).max(c2 & 0xff);
            r | g | b
        }
        CombineMode::Darkest => {
            let r = (c1 & 0xff0000).min(c2 & 0xff0000);
            let g = (c1 & 0xff00).min(c2 & 0xff00);
            let b = (c1 & 0xff).min(c2 & 0xff);
            r | g | b
        }
        // 交错模式不走 comb（尺寸会变），单独处理
        CombineMode::HorizontalInterlace | CombineMode::VerticalInterlace => 0,
    }
}

/// 合成两图，返回结果图像。
pub fn combine(img1: &RgbaImage, img2: &RgbaImage, mode: CombineMode) -> RgbaImage {
    match mode {
        CombineMode::HorizontalInterlace => horizontal_interlace(img1, img2),
        CombineMode::VerticalInterlace => vertical_interlace(img1, img2),
        _ => {
            let width = img1.width().max(img2.width());
            let height = img1.height().max(img2.height());
            let mut out = RgbaImage::new(width, height);
            for y in 0..height {
                for x in 0..width {
                    let c1 = if x < img1.width() && y < img1.height() {
                        argb(img1.get_pixel(x, y))
                    } else {
                        0
                    };
                    let c2 = if x < img2.width() && y < img2.height() {
                        argb(img2.get_pixel(x, y))
                    } else {
                        0
                    };
                    out.put_pixel(x, y, rgb_pixel(comb(c1, c2, mode)));
                }
            }
            out
        }
    }
}

fn horizontal_interlace(img1: &RgbaImage, img2: &RgbaImage) -> RgbaImage {
    let width = img1.width().min(img2.width());
    let height = img1.height().min(img2.height());
    let mut out = RgbaImage::new(width, height * 2);
    for y in 0..height {
        for x in 0..width {
            out.put_pixel(x, y * 2, *img1.get_pixel(x, y));
            out.put_pixel(x, y * 2 + 1, *img2.get_pixel(x, y));
        }
    }
    out
}

fn vertical_interlace(img1: &RgbaImage, img2: &RgbaImage) -> RgbaImage {
    let width = img1.width().min(img2.width());
    let height = img1.height().min(img2.height());
    let mut out = RgbaImage::new(width * 2, height);
    for y in 0..height {
        for x in 0..width {
            out.put_pixel(x * 2, y, *img1.get_pixel(x, y));
            out.put_pixel(x * 2 + 1, y, *img2.get_pixel(x, y));
        }
    }
    out
}

/// 有状态的合成器：持有两图并按模式编号循环。
pub struct CombineTransform {
    img1: RgbaImage,
    img2: RgbaImage,
    mode: CombineMode,
    result: RgbaImage,
}

impl CombineTransform {
    pub fn new(img1: RgbaImage, img2: RgbaImage) -> Self {
        let result = combine(&img1, &img2, CombineMode::Xor);
        Self { img1, img2, mode: CombineMode::Xor, result }
    }

    pub fn get_image(&self) -> &RgbaImage {
        &self.result
    }

    pub fn mode(&self) -> CombineMode {
        self.mode
    }

    pub fn get_text(&self) -> &'static str {
        self.mode.text()
    }

    fn recompute(&mut self) {
        self.result = combine(&self.img1, &self.img2, self.mode);
    }

    pub fn set_mode(&mut self, mode: CombineMode) {
        self.mode = mode;
        self.recompute();
    }

    pub fn set_index(&mut self, i: i32) {
        self.set_mode(CombineMode::from_index(i));
    }

    pub fn forward(&mut self) {
        self.set_index(self.mode.index() + 1);
    }

    pub fn back(&mut self) {
        self.set_index(self.mode.index() - 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn solid(w: u32, h: u32, c: [u8; 4]) -> RgbaImage {
        let mut img = RgbaImage::new(w, h);
        for p in img.pixels_mut() {
            *p = Rgba(c);
        }
        img
    }

    #[test]
    fn xor_mode() {
        let a = solid(1, 1, [0xF0, 0x0F, 0xAA, 255]);
        let b = solid(1, 1, [0x0F, 0xF0, 0x55, 255]);
        let out = combine(&a, &b, CombineMode::Xor);
        assert_eq!(*out.get_pixel(0, 0), Rgba([0xFF, 0xFF, 0xFF, 255]));
    }

    #[test]
    fn and_or_modes() {
        let a = solid(1, 1, [0b1100, 0, 0xFF, 255]);
        let b = solid(1, 1, [0b1010, 0, 0x0F, 255]);
        let and = combine(&a, &b, CombineMode::And);
        let or = combine(&a, &b, CombineMode::Or);
        assert_eq!(and.get_pixel(0, 0)[0], 0b1000);
        assert_eq!(or.get_pixel(0, 0)[0], 0b1110);
        assert_eq!(and.get_pixel(0, 0)[2], 0x0F);
        assert_eq!(or.get_pixel(0, 0)[2], 0xFF);
    }

    // ---- C1 回归：以下模式修复前全部退化为"显示图1" ----

    #[test]
    fn sub_separate_is_implemented() {
        let a = solid(1, 1, [200, 100, 50, 255]);
        let b = solid(1, 1, [50, 30, 10, 255]);
        let out = combine(&a, &b, CombineMode::SubSeparate);
        assert_eq!(*out.get_pixel(0, 0), Rgba([150, 70, 40, 255]));
        // 必须不等于图1
        assert_ne!(*out.get_pixel(0, 0), *a.get_pixel(0, 0));
    }

    #[test]
    fn mul_separate_is_implemented() {
        let a = solid(1, 1, [2, 3, 4, 255]);
        let b = solid(1, 1, [3, 4, 5, 255]);
        let out = combine(&a, &b, CombineMode::MulSeparate);
        // 逐通道乘 mod 256：6,12,20
        assert_eq!(*out.get_pixel(0, 0), Rgba([6, 12, 20, 255]));
    }

    #[test]
    fn lightest_and_darkest() {
        let a = solid(1, 1, [200, 10, 90, 255]);
        let b = solid(1, 1, [50, 240, 90, 255]);
        let light = combine(&a, &b, CombineMode::Lightest);
        let dark = combine(&a, &b, CombineMode::Darkest);
        assert_eq!(*light.get_pixel(0, 0), Rgba([200, 240, 90, 255]));
        assert_eq!(*dark.get_pixel(0, 0), Rgba([50, 10, 90, 255]));
    }

    #[test]
    fn add_separate_wraps_per_channel() {
        let a = solid(1, 1, [200, 0, 0, 255]);
        let b = solid(1, 1, [100, 0, 0, 255]);
        // 200+100 = 300 -> mod 256 = 44
        let out = combine(&a, &b, CombineMode::AddSeparate);
        assert_eq!(out.get_pixel(0, 0)[0], 44);
    }

    #[test]
    fn all_13_modes_run_without_panic() {
        let a = solid(2, 2, [10, 20, 30, 255]);
        let b = solid(2, 2, [40, 50, 60, 255]);
        for i in 0..NUM_MODES {
            let m = CombineMode::from_index(i);
            let _ = combine(&a, &b, m);
        }
    }

    #[test]
    fn interlace_dimensions() {
        let a = solid(4, 3, [1, 1, 1, 255]);
        let b = solid(4, 3, [2, 2, 2, 255]);
        let h = combine(&a, &b, CombineMode::HorizontalInterlace);
        assert_eq!((h.width(), h.height()), (4, 6));
        let v = combine(&a, &b, CombineMode::VerticalInterlace);
        assert_eq!((v.width(), v.height()), (8, 3));
        // 交错行/列应分别来自两图
        assert_eq!(*h.get_pixel(0, 0), Rgba([1, 1, 1, 255]));
        assert_eq!(*h.get_pixel(0, 1), Rgba([2, 2, 2, 255]));
    }

    #[test]
    fn stateful_cycle_wraps() {
        let a = solid(1, 1, [1, 2, 3, 255]);
        let b = solid(1, 1, [4, 5, 6, 255]);
        let mut c = CombineTransform::new(a, b);
        assert_eq!(c.mode(), CombineMode::Xor);
        c.back();
        assert_eq!(c.mode(), CombineMode::VerticalInterlace);
        c.forward();
        assert_eq!(c.mode(), CombineMode::Xor);
    }
}
