use eframe::egui::{self, TextureOptions, TextureHandle};
use egui::{Context, Slider, Color32, Vec2, Window};
use image::{RgbaImage, ImageBuffer};
use std::cell::RefCell;
use std::rc::Rc;
use rfd;

/// 常量，用于选择合成模式
const NUM_TRANSFORMS: i32 = 13;

pub struct ImageCombiner {
    img1: RgbaImage,
    img2: Rc<RefCell<Option<RgbaImage>>>,
    transform_num: Rc<RefCell<i32>>,
    texture: Option<TextureHandle>, // 存储合成图像的纹理
}

impl ImageCombiner {
    pub fn new(img1: RgbaImage) -> Self {
        Self {
            img1,
            img2: Rc::new(RefCell::new(None)),
            transform_num: Rc::new(RefCell::new(0)),
            texture: None,
        }
    }

    pub fn update(&mut self, ui: &mut egui::Ui) { // 修改参数类型为 Ui
        ui.vertical(|ui| {
            ui.label("请选择第二张图片");
    
            if let Some(texture) = &self.texture {
                ui.image(texture);
            }
    
            ui.separator();
    
            // 显示当前合成模式的文本
            ui.label(format!("当前合成模式: {}", self.get_transform_text()));
    
            ui.separator();
    
            // Transform Selector
            ui.label("选择合成模式");
            let mut current_transform = *self.transform_num.borrow();
            if ui.add(Slider::new(&mut current_transform, 0..=NUM_TRANSFORMS - 1).text("变换模式")).changed() {
                *self.transform_num.borrow_mut() = current_transform;
            }
            ui.separator();
    
            // Buttons
            ui.horizontal(|ui| {
                if ui.button("<").clicked() {
                    self.backward(ui.ctx());
                }
    
                if ui.button("打开图片").clicked() {
                    self.open_second_image(ui.ctx());
                }
    
                if ui.button("保存").clicked() {
                    self.save_image(ui.ctx());
                }
    
                if ui.button(">").clicked() {
                    self.forward(ui.ctx());
                }
            });
        });
    }

    fn backward(&mut self, ctx: &Context) {
        if self.img2.borrow().is_none() { return; }
        {
            let mut num = self.transform_num.borrow_mut();
            *num = if *num <= 0 {
                NUM_TRANSFORMS - 1
            } else {
                *num - 1
            };
        }
        self.update_image_with_context(ctx);
    }

    fn forward(&mut self, ctx: &Context) {
        if self.img2.borrow().is_none() { return; }
        {
            let mut num = self.transform_num.borrow_mut();
            *num = (*num + 1) % NUM_TRANSFORMS;
        }
        self.update_image_with_context(ctx);
    }

    fn open_second_image(&mut self, ctx: &Context) {
        let dialog = rfd::FileDialog::new().pick_file();
        if let Some(path) = dialog {
            if let Ok(img) = image::open(path) {
                *self.img2.borrow_mut() = Some(img.to_rgba8());
                self.update_image_with_context(ctx);
            } else {
                println!("无法打开图片");
            }
        }
    }

    fn save_image(&self, ctx: &Context) {
        if self.img2.borrow().is_none() { return; }

        let dialog = rfd::FileDialog::new().save_file();
        if let Some(path) = dialog {
            if let Some(combined) = self.get_combined_image() {
                if let Err(e) = combined.save(path) {
                    println!("保存失败: {}", e);
                }
            }
        }
    }

    fn get_combined_image(&self) -> Option<RgbaImage> {
        let img2 = self.img2.borrow();
        let img2 = img2.as_ref()?;

        let transform_num = *self.transform_num.borrow();
        
        match transform_num {
            11 => self.horizontal_interlace(img2),
            12 => self.vertical_interlace(img2),
            _ => self.combine_pixels(img2, transform_num),
        }
    }

    fn combine_pixels(&self, img2: &RgbaImage, transform_num: i32) -> Option<RgbaImage> {
        let width = self.img1.width().max(img2.width());
        let height = self.img1.height().max(img2.height());
        
        let mut result = ImageBuffer::new(width, height);

        for y in 0..height {
            for x in 0..width {
                let p1 = if x < self.img1.width() && y < self.img1.height() {
                    self.img1.get_pixel(x, y)
                } else {
                    &image::Rgba([0, 0, 0, 0])
                };

                let p2 = if x < img2.width() && y < img2.height() {
                    img2.get_pixel(x, y)
                } else {
                    &image::Rgba([0, 0, 0, 0])
                };

                let combined = match transform_num  {
                    0 => [p1[0]^p2[0], p1[1]^p2[1], p1[2]^p2[2], 255], // XOR
                    1 => [p1[0]|p2[0], p1[1]|p2[1], p1[2]|p2[2], 255], // OR
                    2 => [p1[0]&p2[0], p1[1]&p2[1], p1[2]&p2[2], 255], // AND
                    3 => [ // ADD
                        p1[0].saturating_add(p2[0]),
                        p1[1].saturating_add(p2[1]),
                        p1[2].saturating_add(p2[2]),
                        255
                    ],
                    4 => [ // ADD separate
                        ((p1[0] as u16 + p2[0] as u16) % 256) as u8,
                        ((p1[1] as u16 + p2[1] as u16) % 256) as u8,
                        ((p1[2] as u16 + p2[2] as u16) % 256) as u8,
                        255
                    ],
                    _ => [p1[0], p1[1], p1[2], 255]
                };

                result.put_pixel(x, y, image::Rgba(combined));
            }
        }

        Some(result)
    }

    fn horizontal_interlace(&self, img2: &RgbaImage) -> Option<RgbaImage> {
        let width = self.img1.width().min(img2.width());
        let height = self.img1.height().min(img2.height());
        
        let mut result = ImageBuffer::new(width, height * 2);

        for y in 0..height {
            for x in 0..width {
                let p1 = self.img1.get_pixel(x, y);
                let p2 = img2.get_pixel(x, y);
                
                result.put_pixel(x, y*2, *p1);
                result.put_pixel(x, y*2+1, *p2);
            }
        }

        Some(result)
    }

    fn vertical_interlace(&self, img2: &RgbaImage) -> Option<RgbaImage> {
        let width = self.img1.width().min(img2.width());
        let height = self.img1.height().min(img2.height());
        
        let mut result = ImageBuffer::new(width * 2, height);

        for y in 0..height {
            for x in 0..width {
                let p1 = self.img1.get_pixel(x, y);
                let p2 = img2.get_pixel(x, y);
                
                result.put_pixel(x*2, y, *p1);
                result.put_pixel(x*2+1, y, *p2);
            }
        }

        Some(result)
    }

    fn update_image_with_context(&mut self, ctx: &Context) {
        if let Some(combined) = self.get_combined_image() {
            let size = [combined.width() as usize, combined.height() as usize];
            let image_data = egui::ColorImage::from_rgba_unmultiplied(size, combined.as_raw());
            self.texture = Some(ctx.load_texture(
                "combined_image",
                image_data,
                TextureOptions::default(),
            ));
        }
    }

    fn get_transform_text(&self) -> String {
        match *self.transform_num.borrow() {
            0 => "XOR",
            1 => "OR",
            2 => "AND",
            3 => "ADD",
            4 => "ADD (R,G,B separate)",
            5 => "SUB",
            6 => "SUB (R,G,B separate)", 
            7 => "MUL",
            8 => "MUL (R,G,B separate)",
            9 => "Lightest (R,G,B separate)",
            10 => "Darkest (R,G,B separate)",
            11 => "Horizontal Interlace",
            12 => "Vertical Interlace",
            _ => "???"
        }.to_string()
    }
}
