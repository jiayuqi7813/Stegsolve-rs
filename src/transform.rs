use image::{DynamicImage, RgbaImage, Rgba};
use rand::Rng;

pub struct Transform {
    original_image: RgbaImage,   // 原始图像
    transformed_image: RgbaImage,// 变换后的图像
    trans_num: i32,              // 当前变换编号
    max_trans: i32,              // 最大变换编号
}

impl Transform {
    pub fn new(img: DynamicImage) -> Self {
        let rgba_img = img.to_rgba8(); // 将图像转换为 RgbaImage
        Self {
            original_image: rgba_img.clone(),
            transformed_image: rgba_img,
            trans_num: 0,
            max_trans: 41, // 最大变换编号
        }
    }

    // 获取当前变换后的图像
    pub fn get_image(&self) -> &RgbaImage {
        &self.transformed_image
    }

    // 获取当前变换的描述文本（与原版 StegSolve 对应）
    pub fn get_text(&self) -> String {
        match self.trans_num {
            0 => "正常图像".to_string(),
            1 => "颜色反转 (Xor)".to_string(),
            2..=9  => format!("Alpha plane {}", 9 - self.trans_num),
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
            _ => "".to_string(),
        }
    }

    // 切换到上一个变换
    pub fn back(&mut self) {
        self.trans_num -= 1;
        if self.trans_num < 0 {
            self.trans_num = self.max_trans;
        }
        self.calc_trans();
    }

    // 切换到下一个变换
    pub fn forward(&mut self) {
        self.trans_num += 1;
        if self.trans_num > self.max_trans {
            self.trans_num = 0;
        }
        self.calc_trans();
    }

    // 反转颜色 (类似 Java 里的 col ^ 0xffffff)
    fn inversion(&mut self) {
        let img = &self.original_image;
        let mut new_img = RgbaImage::new(img.width(), img.height());
        for (x, y, pixel) in img.enumerate_pixels() {
            // 直接对 RGB 做 255 - value; alpha 保持不变或设为255
            let new_pixel = Rgba([
                255 - pixel[0],
                255 - pixel[1],
                255 - pixel[2],
                255, // 与 Java TYPE_INT_RGB 一致，不用原 alpha
            ]);
            new_img.put_pixel(x, y, new_pixel);
        }
        self.transformed_image = new_img;
    }

    // ===== 关键修改：transform_bit 按照 Java 的 ARGB 做位平面提取 =====
    fn transform_bit(&mut self, bit: i32) {
        let img = &self.original_image;
        let (width, height) = (img.width(), img.height());
        let mut new_img = RgbaImage::new(width, height);
    
        for (x, y, pixel) in img.enumerate_pixels() {
            // **务必确认 pixel[..] 的含义是真实 RGBA，别拿反顺序**
            let fcol = ((pixel[3] as u32) << 24)  // A
                     | ((pixel[0] as u32) << 16)  // R
                     | ((pixel[1] as u32) << 8)   // G
                     |  (pixel[2] as u32);        // B
    
            // Java 相当于: if(((fcol >>> bit) & 1) > 0) col=0xffffff else 0
            let col = if ((fcol >> bit) & 1) == 1 {
                0xffffff
            } else {
                0x000000
            };
    
            // 写回时，只要保留 RGB，A=255 (StegSolve 的 TYPE_INT_RGB 常用做法)
            let r = (col >> 16) as u8;
            let g = (col >> 8)  as u8;
            let b = (col & 0xff) as u8;
            new_img.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    
        self.transformed_image = new_img;
    }

    // ===== 关键修改：transform_mask 与原版 Java transmask(int mask) 对齐 =====
    fn transform_mask(&mut self, mask: u32) {
        let img = &self.original_image;
        let mut new_img = RgbaImage::new(img.width(), img.height());
    
        for (x, y, pixel) in img.enumerate_pixels() {
            let fcol = (255u32 << 24)         // A=255
                     | ((pixel[0] as u32) << 16)
                     | ((pixel[1] as u32) << 8)
                     |  (pixel[2] as u32);
    
            let mut col = fcol & mask;
            if col > 0xffffff {
                col >>= 8;  // 与 Java 的 col >>> 8 对齐
            }
    
            let r = (col >> 16) as u8;
            let g = (col >> 8)  as u8;
            let b = (col & 0xff) as u8;
            new_img.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    
        self.transformed_image = new_img;
    }

    fn random_colormap(&mut self) {
        let img = &self.original_image;
        let mut new_img = RgbaImage::new(img.width(), img.height());
        let mut rng = rand::thread_rng();

        // 生成随机系数和偏移量
        let bm = rng.gen_range(0..256) as u32;
        let ba = rng.gen_range(0..256) as u32;
        let bx = rng.gen_range(0..256) as u32;
        let gm = rng.gen_range(0..256) as u32;
        let ga = rng.gen_range(0..256) as u32;
        let gx = rng.gen_range(0..256) as u32;
        let rm = rng.gen_range(0..256) as u32;
        let ra = rng.gen_range(0..256) as u32;
        let rx = rng.gen_range(0..256) as u32;

        for (x, y, pixel) in img.enumerate_pixels() {
            let b = ((pixel[0] as u32 * bm) ^ bx) + ba;
            let g = ((pixel[1] as u32 * gm) ^ gx) + ga;
            let r = ((pixel[2] as u32 * rm) ^ rx) + ra;

            // 确保颜色值在 0-255 范围内
            let b = (b & 0xff) as u8;
            let g = (g & 0xff) as u8;
            let r = (r & 0xff) as u8;

            new_img.put_pixel(x, y, Rgba([r, g, b, 255]));
        }

        self.transformed_image = new_img;
    }

    // 随机颜色映射（适用于 IndexColorModel）
    fn random_indexmap(&mut self) {
        // 由于 Rust 的 image crate 不支持直接操作调色板，
        // 我们可以假设图像是 RGBA 格式，并直接应用随机颜色映射。
        // 如果需要支持调色板图像，可以考虑使用其他库或手动处理调色板。
        self.random_colormap();
    }

    // 根据图像类型选择随机映射方式
    fn random_map(&mut self) {
        // 假设图像是 RGBA 格式（ComponentColorModel）
        self.random_colormap();
    }

    // 灰度高亮 (r = g = b时显示白色，否则黑色)
    fn gray_bits(&mut self) {
        let img = &self.original_image;
        let mut new_img = RgbaImage::new(img.width(), img.height());
        for (x, y, pixel) in img.enumerate_pixels() {
            if pixel[0] == pixel[1] && pixel[0] == pixel[2] {
                new_img.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            } else {
                new_img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        self.transformed_image = new_img;
    }

    // 根据当前 trans_num 计算变换结果
    fn calc_trans(&mut self) {
        match self.trans_num {
            0 => {
                self.transformed_image = self.original_image.clone();
            }
            1 => {
                self.inversion();
            }
            2 => {
                // Alpha plane 7
                self.transform_bit(31);
            }
            3 => {
                // Alpha plane 6
                self.transform_bit(30);
            }
            4 => {
                // Alpha plane 5
                self.transform_bit(29);
            }  
            5 => {
                // Alpha plane 4
                self.transform_bit(28);
            }  
            6 => {
                // Alpha plane 3
                self.transform_bit(27);
            }
            7 => {
                // Alpha plane 2
                self.transform_bit(26);
            }
            8 => {
                // Alpha plane 1
                self.transform_bit(25);
            }   
            9 => {
                // Alpha plane 0
                self.transform_bit(24);
            }
            10 => {
                // Red plane 7
                self.transform_bit(23);
            }
            11 => {
                // Red plane 6
                self.transform_bit(22);
            }
            12 => {
                // Red plane 5
                self.transform_bit(21);
            }
            13 => {
                // Red plane 4
                self.transform_bit(20);
            }
            14 => {
                // Red plane 3
                self.transform_bit(19);
            }
            15 => {
                // Red plane 2
                self.transform_bit(18);
            }
            16 => {
                // Red plane 1
                self.transform_bit(17);
            }
            17 => {
                // Red plane 0
                self.transform_bit(16);
            }
            18 => {
                // Green plane 7
                self.transform_bit(15);
            }
            19 => {
                // Green plane 6
                self.transform_bit(14);
            }
            20 => {
                // Green plane 5
                self.transform_bit(13);
            }
            21 => {
                // Green plane 4
                self.transform_bit(12);
            }
            22 => {
                // Green plane 3
                self.transform_bit(11);
            }
            23 => {
                // Green plane 2
                self.transform_bit(10);
            }
            24 => {
                // Green plane 1
                self.transform_bit(9);
            }
            25 => {
                // Green plane 0
                self.transform_bit(8);
            }
            26 => {
                // Blue plane 7
                self.transform_bit(7);
            }
            27 => {
                // Blue plane 6
                self.transform_bit(6);
            }
            28 => {
                // Blue plane 5
                self.transform_bit(5);
            }
            29 => {
                // Blue plane 4
                self.transform_bit(4);
            }
            30 => {
                // Blue plane 3
                self.transform_bit(3);
            }
            31 => {
                // Blue plane 2
                self.transform_bit(2);
            }
            32 => {
                // Blue plane 1
                self.transform_bit(1);
            }
            33 => {
                // Blue plane 0
                self.transform_bit(0);
            }
            34 => {
                // Full alpha
                self.transform_mask(0xff000000);
            }
            35 => {
                // Full red
                self.transform_mask(0x00ff0000);
            }
            36 => {
                // Full green
                self.transform_mask(0x0000ff00);
            }
            37 => {
                // Full blue
                self.transform_mask(0x000000ff);
            }
            38..=40 => {
                self.random_map();
            }
            41 => {
                self.gray_bits();
            }
            _ => {
                self.transformed_image = self.original_image.clone();
            }
            
        }
    }
}