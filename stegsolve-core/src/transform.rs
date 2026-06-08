//! 单图变换：位平面、通道掩码、反色、随机颜色映射、灰度高亮。
//!
//! 对照原版 `Transform.java` 修复：
//! - **T1**：`transmask`（Full alpha/red/green/blue）此前把 alpha 硬编码成 255，
//!   导致 "Full alpha" 永远是纯红、丢失真实透明度。现改为读取真实 alpha。
//! - **T2**：补充索引/调色板式随机映射（distinct-color 随机置换），可揭示
//!   调色板隐写；真彩色图仍用数值公式映射。
//! - **T3**：随机颜色映射公式与原版对齐（系数乘两次、保留通道位置，不再红蓝错位）。

use image::{DynamicImage, Rgba, RgbaImage};
use std::collections::HashMap;

use crate::rng::Rng;

/// 支持的最大变换编号（与原版一致：0..=41）。
pub const MAX_TRANS: i32 = 41;

/// 单图变换器。
pub struct Transform {
    original_image: RgbaImage,
    transformed_image: RgbaImage,
    trans_num: i32,
    /// 随机映射用的滚动 seed，每次重算随机映射时推进，模拟原版"每次都不同"。
    rand_seed: u64,
}

impl Transform {
    /// 由 `DynamicImage` 创建（统一转换为 RGBA8）。
    pub fn new(img: DynamicImage) -> Self {
        Self::from_rgba(img.to_rgba8())
    }

    /// 由 `RgbaImage` 创建。
    pub fn from_rgba(rgba: RgbaImage) -> Self {
        Self {
            original_image: rgba.clone(),
            transformed_image: rgba,
            trans_num: 0,
            rand_seed: 0x1234_5678_9ABC_DEF0,
        }
    }

    /// 当前变换后的图像。
    pub fn get_image(&self) -> &RgbaImage {
        &self.transformed_image
    }

    /// 原始（未变换）图像。数据提取应基于原图，而非当前显示的变换视图。
    pub fn original_image(&self) -> &RgbaImage {
        &self.original_image
    }

    /// 当前变换编号。
    pub fn trans_num(&self) -> i32 {
        self.trans_num
    }

    /// 当前变换的中文/英文描述（与原版一致）。
    pub fn get_text(&self) -> String {
        match self.trans_num {
            0 => "正常图像".to_string(),
            1 => "颜色反转 (Xor)".to_string(),
            2..=9 => format!("Alpha plane {}", 9 - self.trans_num),
            10..=17 => format!("Red plane {}", 17 - self.trans_num),
            18..=25 => format!("Green plane {}", 25 - self.trans_num),
            26..=33 => format!("Blue plane {}", 33 - self.trans_num),
            34 => "Full alpha".to_string(),
            35 => "Full red".to_string(),
            36 => "Full green".to_string(),
            37 => "Full blue".to_string(),
            38 => "Random colour map 1".to_string(),
            39 => "Random colour map 2".to_string(),
            40 => "Random colour map 3".to_string(),
            41 => "灰度".to_string(),
            _ => String::new(),
        }
    }

    /// 上一个变换（带回绕）。
    pub fn back(&mut self) {
        self.trans_num -= 1;
        if self.trans_num < 0 {
            self.trans_num = MAX_TRANS;
        }
        self.calc_trans();
    }

    /// 下一个变换（带回绕）。
    pub fn forward(&mut self) {
        self.trans_num += 1;
        if self.trans_num > MAX_TRANS {
            self.trans_num = 0;
        }
        self.calc_trans();
    }

    /// 直接跳转到指定变换。
    pub fn set_trans(&mut self, n: i32) {
        self.trans_num = n.clamp(0, MAX_TRANS);
        self.calc_trans();
    }

    /// 把 [R,G,B,A] 像素打包成 Java `getRGB` 风格的 ARGB u32。
    #[inline]
    fn argb(p: &Rgba<u8>) -> u32 {
        ((p[3] as u32) << 24) | ((p[0] as u32) << 16) | ((p[1] as u32) << 8) | (p[2] as u32)
    }

    /// 把一个 24 位 RGB 值写成不透明像素。
    #[inline]
    fn rgb_pixel(col: u32) -> Rgba<u8> {
        Rgba([(col >> 16) as u8, (col >> 8) as u8, (col & 0xff) as u8, 255])
    }

    /// 反色：等价于 Java `col ^ 0xffffff`（逐通道 255 - v）。
    fn inversion(&mut self) {
        let img = &self.original_image;
        let mut out = RgbaImage::new(img.width(), img.height());
        for (x, y, p) in img.enumerate_pixels() {
            out.put_pixel(x, y, Rgba([255 - p[0], 255 - p[1], 255 - p[2], 255]));
        }
        self.transformed_image = out;
    }

    /// 位平面：取 ARGB 第 `bit` 位，置位则白、否则黑（读取真实 alpha）。
    fn transform_bit(&mut self, bit: u32) {
        let img = &self.original_image;
        let mut out = RgbaImage::new(img.width(), img.height());
        for (x, y, p) in img.enumerate_pixels() {
            let fcol = Self::argb(p);
            let col = if (fcol >> bit) & 1 == 1 { 0xffffff } else { 0 };
            out.put_pixel(x, y, Self::rgb_pixel(col));
        }
        self.transformed_image = out;
    }

    /// 通道掩码（Full alpha/red/green/blue）。
    ///
    /// FIX T1：使用真实 alpha。逻辑严格对齐 Java `transmask`：
    /// `col = fcol & mask; if col > 0xffffff { col >>= 8 }`（原版 `col<0` 的情况在
    /// 无符号下等价于 `col > 0xffffff`）。
    fn transform_mask(&mut self, mask: u32) {
        let img = &self.original_image;
        let mut out = RgbaImage::new(img.width(), img.height());
        for (x, y, p) in img.enumerate_pixels() {
            let fcol = Self::argb(p);
            let mut col = fcol & mask;
            if col > 0xffffff {
                col >>= 8;
            }
            out.put_pixel(x, y, Self::rgb_pixel(col));
        }
        self.transformed_image = out;
    }

    /// 灰度高亮：r==g==b 显示白色，否则黑色。
    fn graybits(&mut self) {
        let img = &self.original_image;
        let mut out = RgbaImage::new(img.width(), img.height());
        for (x, y, p) in img.enumerate_pixels() {
            let v = if p[0] == p[1] && p[1] == p[2] { 0xffffff } else { 0 };
            out.put_pixel(x, y, Self::rgb_pixel(v));
        }
        self.transformed_image = out;
    }

    /// 根据当前编号重算。随机映射会根据图像是否"调色板式"自动选择策略。
    fn calc_trans(&mut self) {
        match self.trans_num {
            0 => self.transformed_image = self.original_image.clone(),
            1 => self.inversion(),
            2..=9 => self.transform_bit((33 - self.trans_num) as u32), // 31..24
            10..=17 => self.transform_bit((33 - self.trans_num) as u32), // 23..16
            18..=25 => self.transform_bit((33 - self.trans_num) as u32), // 15..8
            26..=33 => self.transform_bit((33 - self.trans_num) as u32), // 7..0
            34 => self.transform_mask(0xff000000),
            35 => self.transform_mask(0x00ff0000),
            36 => self.transform_mask(0x0000ff00),
            37 => self.transform_mask(0x000000ff),
            38..=40 => {
                self.rand_seed = self.rand_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let mut rng = Rng::new(self.rand_seed);
                self.transformed_image = random_map(&self.original_image, &mut rng);
            }
            41 => self.graybits(),
            _ => self.transformed_image = self.original_image.clone(),
        }
    }
}

/// 随机映射调度：调色板式图像（distinct color 数 ≤ 256）走调色板随机置换，
/// 否则走数值公式映射（对齐原版 `random_colormap`）。
pub fn random_map(img: &RgbaImage, rng: &mut Rng) -> RgbaImage {
    if let Some(palette_mapped) = random_palette_map(img, rng) {
        palette_mapped
    } else {
        random_colormap(img, rng)
    }
}

/// 数值公式随机映射（FIX T3：对齐原版，系数乘两次、保留通道位置）。
pub fn random_colormap(img: &RgbaImage, rng: &mut Rng) -> RgbaImage {
    let (bm, ba, bx) = (rng.byte(), rng.byte(), rng.byte());
    let (gm, ga, gx) = (rng.byte(), rng.byte(), rng.byte());
    let (rm, ra, rx) = (rng.byte(), rng.byte(), rng.byte());

    let mut out = RgbaImage::new(img.width(), img.height());
    for (x, y, p) in img.enumerate_pixels() {
        let red = p[0] as u32;
        let green = p[1] as u32;
        let blue = p[2] as u32;

        let mut b = blue.wrapping_mul(bm);
        b = (b.wrapping_mul(bm) ^ bx).wrapping_add(ba);
        let mut g = green.wrapping_mul(gm);
        g = (g.wrapping_mul(gm) ^ gx).wrapping_add(ga);
        let mut r = red.wrapping_mul(rm);
        r = (r.wrapping_mul(rm) ^ rx).wrapping_add(ra);

        let col = (r << 16).wrapping_add(g << 8).wrapping_add(b);
        out.put_pixel(x, y, Transform::rgb_pixel(col & 0xffffff));
    }
    out
}

/// 调色板式随机映射（FIX T2）：当图像 distinct color 数 ≤ 256 时，为每种不同颜色
/// 分配一个随机新颜色（一致映射）。可揭示"调色板顺序即信息"的隐写。
/// 若颜色数超过 256，返回 `None` 交回数值公式映射。
pub fn random_palette_map(img: &RgbaImage, rng: &mut Rng) -> Option<RgbaImage> {
    let mut map: HashMap<[u8; 3], [u8; 3]> = HashMap::new();
    for p in img.pixels() {
        let key = [p[0], p[1], p[2]];
        if !map.contains_key(&key) {
            if map.len() >= 256 {
                return None;
            }
            let new = [rng.byte() as u8, rng.byte() as u8, rng.byte() as u8];
            map.insert(key, new);
        }
    }

    let mut out = RgbaImage::new(img.width(), img.height());
    for (x, y, p) in img.enumerate_pixels() {
        let nc = map[&[p[0], p[1], p[2]]];
        out.put_pixel(x, y, Rgba([nc[0], nc[1], nc[2], 255]));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn make_img() -> RgbaImage {
        // 4 个像素，含一个半透明像素，便于测试 alpha 相关变换
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(0, 0, Rgba([0xAB, 0xCD, 0xEF, 0x10]));
        img.put_pixel(1, 0, Rgba([0x00, 0xFF, 0x00, 0xFF]));
        img.put_pixel(0, 1, Rgba([0x80, 0x80, 0x80, 0x80]));
        img.put_pixel(1, 1, Rgba([0x12, 0x34, 0x56, 0x00]));
        img
    }

    #[test]
    fn trans_text_matches_original() {
        let mut t = Transform::from_rgba(make_img());
        assert_eq!(t.get_text(), "正常图像");
        t.set_trans(2);
        assert_eq!(t.get_text(), "Alpha plane 7");
        t.set_trans(9);
        assert_eq!(t.get_text(), "Alpha plane 0");
        t.set_trans(10);
        assert_eq!(t.get_text(), "Red plane 7");
        t.set_trans(33);
        assert_eq!(t.get_text(), "Blue plane 0");
        t.set_trans(34);
        assert_eq!(t.get_text(), "Full alpha");
        t.set_trans(41);
        assert_eq!(t.get_text(), "灰度");
    }

    #[test]
    fn back_forward_wraps() {
        let mut t = Transform::from_rgba(make_img());
        t.back();
        assert_eq!(t.trans_num(), MAX_TRANS);
        t.forward();
        assert_eq!(t.trans_num(), 0);
    }

    #[test]
    fn inversion_is_255_minus() {
        let mut t = Transform::from_rgba(make_img());
        t.set_trans(1);
        let out = t.get_image();
        let p = out.get_pixel(0, 0);
        assert_eq!(*p, Rgba([255 - 0xAB, 255 - 0xCD, 255 - 0xEF, 255]));
    }

    #[test]
    fn blue_plane_0_lsb() {
        // Blue plane 0 = trans 33 = bit 0 of ARGB = LSB of blue channel
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([0, 0, 0x01, 255])); // blue LSB = 1 -> white
        img.put_pixel(1, 0, Rgba([0, 0, 0x02, 255])); // blue LSB = 0 -> black
        let mut t = Transform::from_rgba(img);
        t.set_trans(33);
        assert_eq!(*t.get_image().get_pixel(0, 0), Rgba([255, 255, 255, 255]));
        assert_eq!(*t.get_image().get_pixel(1, 0), Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn alpha_plane_uses_real_alpha() {
        // Alpha plane 0 = trans 9 = bit 24 = LSB of alpha
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([0, 0, 0, 0x01])); // alpha LSB = 1 -> white
        img.put_pixel(1, 0, Rgba([0, 0, 0, 0x02])); // alpha LSB = 0 -> black
        let mut t = Transform::from_rgba(img);
        t.set_trans(9);
        assert_eq!(*t.get_image().get_pixel(0, 0), Rgba([255, 255, 255, 255]));
        assert_eq!(*t.get_image().get_pixel(1, 0), Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn full_alpha_shows_real_alpha_not_solid_red() {
        // FIX T1 回归测试：Full alpha 必须随真实 alpha 变化，而不是恒为纯红。
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([10, 20, 30, 0x00])); // alpha 0
        img.put_pixel(1, 0, Rgba([10, 20, 30, 0xFF])); // alpha 255
        let mut t = Transform::from_rgba(img);
        t.set_trans(34); // Full alpha, mask 0xff000000
        let p0 = *t.get_image().get_pixel(0, 0);
        let p1 = *t.get_image().get_pixel(1, 0);
        // alpha=0 -> col=0 -> 黑；alpha=255 -> 0xff000000>>8=0x00ff0000 -> 红
        assert_eq!(p0, Rgba([0, 0, 0, 255]));
        assert_eq!(p1, Rgba([0xff, 0, 0, 255]));
        assert_ne!(p0, p1, "Full alpha 必须区分不同 alpha（修复前恒为纯红）");
    }

    #[test]
    fn full_red_shows_red_gradient() {
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([0x00, 0x55, 0x99, 255]));
        img.put_pixel(1, 0, Rgba([0xFF, 0x55, 0x99, 255]));
        let mut t = Transform::from_rgba(img);
        t.set_trans(35); // Full red, mask 0x00ff0000
        assert_eq!(*t.get_image().get_pixel(0, 0), Rgba([0x00, 0, 0, 255]));
        assert_eq!(*t.get_image().get_pixel(1, 0), Rgba([0xFF, 0, 0, 255]));
    }

    #[test]
    fn graybits_highlights_equal_channels() {
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([0x42, 0x42, 0x42, 255])); // r=g=b -> white
        img.put_pixel(1, 0, Rgba([0x42, 0x43, 0x42, 255])); // not equal -> black
        let mut t = Transform::from_rgba(img);
        t.set_trans(41);
        assert_eq!(*t.get_image().get_pixel(0, 0), Rgba([255, 255, 255, 255]));
        assert_eq!(*t.get_image().get_pixel(1, 0), Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn random_colormap_is_deterministic_and_keeps_channels() {
        let img = make_img();
        let mut r1 = Rng::new(99);
        let mut r2 = Rng::new(99);
        let a = random_colormap(&img, &mut r1);
        let b = random_colormap(&img, &mut r2);
        assert_eq!(a, b, "同 seed 必须得到相同结果");
        // 所有像素 alpha 应为 255
        for p in a.pixels() {
            assert_eq!(p[3], 255);
        }
    }

    #[test]
    fn palette_map_used_for_low_color_images() {
        // make_img 只有 4 种颜色 -> 走调色板置换；相同颜色映射到相同新颜色
        let mut img = RgbaImage::new(3, 1);
        img.put_pixel(0, 0, Rgba([1, 2, 3, 255]));
        img.put_pixel(1, 0, Rgba([1, 2, 3, 255])); // 与第0个同色
        img.put_pixel(2, 0, Rgba([9, 9, 9, 255]));
        let mut rng = Rng::new(5);
        let out = random_palette_map(&img, &mut rng).expect("应走调色板映射");
        assert_eq!(out.get_pixel(0, 0), out.get_pixel(1, 0), "同色必须映射为同色");
        assert_ne!(out.get_pixel(0, 0), out.get_pixel(2, 0), "不同色应分配不同随机色（极小概率碰撞除外）");
    }
}
