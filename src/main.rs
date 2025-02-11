#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#![allow(rustdoc::missing_crate_level_docs)] // it's an example

mod transform;
use eframe::egui;
use egui::*;
use rfd;
use image::DynamicImage;


use transform::Transform;

#[derive(Default)]
struct MyApp {
    transform: Option<Transform>,
    current_file_path: Option<String>,
    zoom_level: f32, // 改为f32，因为egui使用f32
    texture: Option<egui::TextureHandle>,
}



fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0]),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "StegSolve (Rust + Egui)",
        options,
        Box::new(|cc| {
            // 设置字体
            let mut fonts = egui::FontDefinitions::default();
            let fonts_data: &'static [u8] = include_bytes!("../font/MiSans-Normal.ttf");
            
            fonts.font_data.insert(
                "misans".to_owned(),
                std::sync::Arc::new(egui::FontData::from_static(fonts_data)),
            );
            
            fonts.families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "misans".to_owned());

            cc.egui_ctx.set_fonts(fonts);
            
            Ok(Box::<MyApp>::default())
        }),
    ) {
        eprintln!("Error: {}", e);
    }
}


impl eframe::App for MyApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // 界面布局开始
        CentralPanel::default().show(ctx, |ui| {
            // 头部标题栏
            ui.horizontal(|ui| {
                ui.heading("StegSolve (Rust + Egui)");
                if ui.button("文件").clicked() {
                    // TODO: 打开文件
                }
                if ui.button("分析").clicked() {
                    // TODO: 打开分析菜单
                }
                if ui.button("帮助").clicked() {
                    // TODO: 显示帮助信息
                }
            });

            // 图像显示区域
            if let Some(transform) = &self.transform {
                if self.texture.is_none() {
                    let rgba_image = transform.get_image();
                    let size = [rgba_image.width() as usize, rgba_image.height() as usize];
                    let image_data = egui::ColorImage::from_rgba_unmultiplied(
                        size,
                        rgba_image.as_raw(),
                    );
                    self.texture = Some(ctx.load_texture(
                        "image", 
                        image_data,
                        TextureOptions::default() // 添加纹理选项参数
                    ));
                }

                if let Some(texture) = &self.texture {
                    let size = texture.size_vec2() * self.zoom_level;
                    ui.image((texture.id(), size));
                }
            }

            // 缩放控制
            ui.add(Slider::new(&mut self.zoom_level, 0.1..=5.0).text("缩放"));

            // 前后按钮
            ui.horizontal(|ui| {
                if ui.button("<").clicked() {
                    if let Some(transform) = &mut self.transform {
                        transform.back();
                        self.texture = None; // 强制刷新纹理
                    }
                }
                if ui.button(">").clicked() {
                    if let Some(transform) = &mut self.transform {
                        transform.forward();
                        self.texture = None; // 强制刷新纹理
                    }
                }
            });

            // 文件操作按钮
            ui.horizontal(|ui| {
                if ui.button("打开").clicked() {
                    // 简化文件打开逻辑
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        match image::open(&path) {
                            Ok(img) => {
                                self.transform = Some(Transform::new(img));
                                self.current_file_path = Some(path.to_string_lossy().to_string());
                                self.texture = None; // 重置纹理
                            }
                            Err(e) => {
                                eprintln!("打开图片失败: {:?}", e);
                            }
                        }
                    }
                }
                if ui.button("另存为").clicked() {
                    if let Some(transform) = &self.transform {
                        if let Some(path) = rfd::FileDialog::new().save_file() {
                            let img = transform.get_image();
                            img.save(path).unwrap();
                        }
                    }
                }
            });
        });
    }
}

fn file_open_dialog() -> Option<String> {
    // TODO: 显示文件对话框并返回文件路径
    None
}

fn file_save_dialog() -> Option<String> {
    // TODO: 显示保存文件对话框并返回保存路径
    None
}

fn save_image(img: &image::RgbaImage, path: String) {
    // 保存图像到文件
    img.save(path).unwrap();
}

