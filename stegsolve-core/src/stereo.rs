//! 立体图求解：把图像与其自身水平偏移版本逐像素 XOR（仅 RGB），用于揭示
//! 视差隐写 / 立体图。逻辑与原版 `StereoTransform.java` 一致。
//!
//! 修复：移除了原 `back()/forward()` 里每次都触发的 `println!` 调试输出。

use image::{Rgba, RgbaImage};

pub struct StereoTransform {
    original_image: RgbaImage,
    transform: RgbaImage,
    trans_num: i32,
}

impl StereoTransform {
    pub fn new(img: RgbaImage) -> Self {
        let mut st = Self {
            transform: RgbaImage::new(img.width(), img.height()),
            original_image: img,
            trans_num: 0,
        };
        st.calc_trans();
        st
    }

    pub fn get_image(&self) -> &RgbaImage {
        &self.transform
    }

    pub fn trans_num(&self) -> i32 {
        self.trans_num
    }

    pub fn get_text(&self) -> String {
        format!("偏移量: {}", self.trans_num)
    }

    fn calc_trans(&mut self) {
        let width = self.original_image.width();
        let height = self.original_image.height();
        let mut out = RgbaImage::new(width, height);
        if width == 0 {
            self.transform = out;
            return;
        }
        for j in 0..height {
            for i in 0..width {
                let f = self.original_image.get_pixel(i, j);
                let off = (i + self.trans_num.rem_euclid(width as i32) as u32) % width;
                let o = self.original_image.get_pixel(off, j);
                out.put_pixel(
                    i,
                    j,
                    Rgba([f[0] ^ o[0], f[1] ^ o[1], f[2] ^ o[2], 255]),
                );
            }
        }
        self.transform = out;
    }

    pub fn back(&mut self) {
        self.trans_num -= 1;
        if self.trans_num < 0 {
            self.trans_num = self.original_image.width() as i32 - 1;
        }
        self.calc_trans();
    }

    pub fn forward(&mut self) {
        self.trans_num += 1;
        if self.trans_num >= self.original_image.width() as i32 {
            self.trans_num = 0;
        }
        self.calc_trans();
    }

    pub fn set_offset(&mut self, n: i32) {
        let w = self.original_image.width() as i32;
        self.trans_num = if w > 0 { n.rem_euclid(w) } else { 0 };
        self.calc_trans();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn offset_zero_is_self_xor_zero() {
        // 偏移 0 时每个像素与自身 XOR = 0（RGB 全黑，alpha 255）
        let mut img = RgbaImage::new(3, 1);
        img.put_pixel(0, 0, Rgba([0x12, 0x34, 0x56, 255]));
        img.put_pixel(1, 0, Rgba([0xAB, 0xCD, 0xEF, 255]));
        img.put_pixel(2, 0, Rgba([0x00, 0xFF, 0x80, 255]));
        let st = StereoTransform::new(img);
        for p in st.get_image().pixels() {
            assert_eq!(*p, Rgba([0, 0, 0, 255]));
        }
    }

    #[test]
    fn offset_one_xors_with_neighbor() {
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([0xF0, 0x0F, 0xAA, 255]));
        img.put_pixel(1, 0, Rgba([0x0F, 0xF0, 0x55, 255]));
        let mut st = StereoTransform::new(img);
        st.set_offset(1);
        // 像素0 与 像素1 XOR
        assert_eq!(*st.get_image().get_pixel(0, 0), Rgba([0xFF, 0xFF, 0xFF, 255]));
        // 像素1 与 像素0（回绕）XOR
        assert_eq!(*st.get_image().get_pixel(1, 0), Rgba([0xFF, 0xFF, 0xFF, 255]));
    }

    #[test]
    fn back_forward_wraps_on_width() {
        let img = RgbaImage::new(4, 1);
        let mut st = StereoTransform::new(img);
        st.back();
        assert_eq!(st.trans_num(), 3);
        st.forward();
        assert_eq!(st.trans_num(), 0);
        st.forward();
        assert_eq!(st.trans_num(), 1);
    }
}
