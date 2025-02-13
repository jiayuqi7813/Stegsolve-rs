use eframe::egui;
use image::{GenericImageView, RgbaImage};
use rfd::FileDialog;
use std::cell::RefCell;
use std::rc::Rc;

pub struct StereoTransform {
    original_image: RgbaImage,
    transform: RgbaImage,
    trans_num: i32,
}

impl StereoTransform {
    pub fn new(img: RgbaImage) -> Self {
        let mut st = Self {
            original_image: img.clone(),
            transform: RgbaImage::new(img.width(), img.height()),
            trans_num: 0,
        };
        st.calc_trans();
        st
    }

    fn calc_trans(&mut self) {
        let width = self.original_image.width() as i32;
        let height = self.original_image.height() as i32;

        self.transform = RgbaImage::new(width as u32, height as u32); // Recreate the transform image

        for i in 0..width {
            for j in 0..height {
                let fcol = self.original_image.get_pixel(i as u32, j as u32);
                let offset = ((i + self.trans_num).rem_euclid(width)) as u32;
                let ocol = self.original_image.get_pixel(offset, j as u32);

                let new_pixel =
                    image::Rgba([fcol[0] ^ ocol[0], fcol[1] ^ ocol[1], fcol[2] ^ ocol[2], 255]);

                self.transform.put_pixel(i as u32, j as u32, new_pixel);
            }
        }
    }

    pub fn back(&mut self) {
        self.trans_num -= 1;
        if self.trans_num < 0 {
            self.trans_num = self.original_image.width() as i32 - 1;
        }
        println!("Back pressed: trans_num = {}", self.trans_num);
        self.calc_trans();
    }

    pub fn forward(&mut self) {
        self.trans_num += 1;
        if self.trans_num >= self.original_image.width() as i32 {
            self.trans_num = 0;
        }
        println!("Forward pressed: trans_num = {}", self.trans_num);
        self.calc_trans();
    }

    pub fn get_text(&self) -> String {
        format!("偏移量: {}", self.trans_num)
    }

    pub fn get_image(&self) -> &RgbaImage {
        &self.transform
    }
}

pub struct Stereo {
    transform: Rc<RefCell<StereoTransform>>, // RefCell stores a mutable reference to the StereoTransform
    texture: Option<egui::TextureHandle>,
}

impl Stereo {
    pub fn new(img: RgbaImage) -> Self {
        Self {
            transform: Rc::new(RefCell::new(StereoTransform::new(img))),
            texture: None,
        }
    }

    fn update_texture(&mut self, ui: &mut egui::Ui) {
        let transform_borrow = self.transform.borrow();
        let image = transform_borrow.get_image();

        let size = [image.width() as usize, image.height() as usize];
        let pixels: Vec<egui::Color32> = image
            .as_raw()
            .chunks_exact(4)
            .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
            .collect();

        let texture_id = "stereo-image"; // Define texture_id outside if let

        let texture = self.texture.get_or_insert_with(|| {
            ui.ctx().load_texture(
                texture_id,
                egui::ColorImage {
                    size,
                    pixels: pixels.clone(),
                },
                egui::TextureOptions::default(),
            )
        });

        texture.set(
            egui::ColorImage { size, pixels },
            egui::TextureOptions::default(),
        );
    }

    pub fn update(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.vertical(|ui| {

            let text = {
                let transform_borrow = self.transform.borrow();
                transform_borrow.get_text()
            };
            ui.label(text);

            self.update_texture(ui);
            if let Some(texture) = &self.texture {
                let size = texture.size_vec2();
                ui.image(texture);
            }

            ui.horizontal(|ui| {
                let transform_rc = self.transform.clone();
                if ui.button("◀").clicked() {
                    transform_rc.borrow_mut().back();
                    self.update_texture(ui); // Update texture after back
                }
                if ui.button("▶").clicked() {
                    transform_rc.borrow_mut().forward(); // Call forward on the correct transform
                    self.update_texture(ui); // Update texture after forward
                }
                if ui.button("保存").clicked() {
                    if let Some(path) = FileDialog::new()
                        .add_filter("图片", &["png", "jpg", "jpeg", "bmp"])
                        .set_file_name("solved.png")
                        .save_file()
                    {
                        let borrowed_transform = transform_rc.borrow();
                        let image = borrowed_transform.get_image();
                        save_rgba_image(image, path);
                    }
                }
            });
        });
    }
}

pub fn save_rgba_image(img: &image::RgbaImage, path: std::path::PathBuf) {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "png".to_string());

    let format = match ext.as_str() {
        "png" => image::ImageFormat::Png,
        "jpg" | "jpeg" => image::ImageFormat::Jpeg,
        "bmp" => image::ImageFormat::Bmp,
        _ => image::ImageFormat::Png,
    };

    if let Err(e) = img.save_with_format(&path, format) {
        eprintln!("保存失败: {}", e);
    }
}
