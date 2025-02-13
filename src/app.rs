use eframe::egui;
use egui::*;




#[derive(Default)]
pub struct StegApp {
    transform: Option<Transform>,
    current_file_path: Option<String>,
    zoom_level: f32,
    texture: Option<egui::TextureHandle>,
    scroll_pos: Vec2, // 新增滚动位置记录
    current_channel_text: String,

}

impl StegApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Box<Self> {
        // 设置字体
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "misans".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(
                include_bytes!("../assets/fonts/MiSans-Normal.ttf")
            )),
        );
        fonts.families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "misans".to_owned());

        cc.egui_ctx.set_fonts(fonts);
        Box::new(Self::default())
    }

    fn open_image(&mut self, path: &std::path::Path) {
        match image::open(path) {
            Ok(img) => {
                self.transform = Some(Transform::new(img));
                self.current_file_path = Some(path.to_string_lossy().to_string());
                self.texture = None;
                self.zoom_level = 1.0;
                self.scroll_pos = Vec2::ZERO;
            }
            Err(e) => eprintln!("打开图片失败: {:?}", e),
        }
    }
}


impl eframe::App for StegApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            menu::bar(ui, |ui| {
                // 文件菜单
                ui.menu_button("文件", |ui| {
                    if ui.button("打开").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                            self.open_image(&path);
                        }
                    }
                    if ui.button("另存为").clicked() {
                        if let Some(transform) = &self.transform {
                            if let Some(path) = rfd::FileDialog::new().save_file() {
                                transform.get_image().save(path).unwrap();
                            }
                        }
                    }
                });

                // 分析菜单
                ui.menu_button("分析", |ui| {
                    if ui.button("文件格式").clicked() {
                        self.show_file_analysis = true;
                    }
                    if ui.button("数据提取").clicked() {
                        self.show_extract_dialog = true;
                    }
                    if ui.button("立体视图").clicked() {
                        self.show_stereo_dialog = true;
                    }
                    if ui.button("帧浏览器").clicked() {
                        self.show_frame_browser = true;
                    }
                    if ui.button("图像合成器").clicked() {
                        self.show_combine_dialog = true;
                    }
                });

                // 帮助菜单
                ui.menu_button("帮助", |ui| {
                    if ui.button("关于").clicked() {
                        self.show_about = true;
                    }
                });
            });
        });

        CentralPanel::default().show(ctx, |ui| {
            ScrollArea::both()
                .id_source("image_scroll")
                .scroll_offset(self.scroll_pos)
                .show(ui, |ui| {
                    if let Some(transform) = &self.transform {
                        if self.texture.is_none() {
                            let rgba_image = transform.get_image();
                            let size = [rgba_image.width() as usize, rgba_image.height() as usize];
                            let image_data = ColorImage::from_rgba_unmultiplied(
                                size,
                                rgba_image.as_raw(),
                            );
                            self.texture = Some(ui.ctx().load_texture(
                                "image",
                                image_data,
                                TextureOptions::default()
                            ));
                        }

                        if let Some(texture) = &self.texture {
                            let desired_size = texture.size_vec2() * self.zoom_level;
                            let (rect, response) = ui.allocate_exact_size(
                                desired_size,
                                Sense::drag(),
                            );
                            
                            // 处理拖拽滚动
                            if response.dragged() {
                                let delta = response.drag_delta();
                                self.scroll_pos -= delta;
                            }
                            
                            // 居中显示图片
                            let painter = ui.painter_at(rect);
                            painter.image(
                                texture.id(),
                                rect,
                                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }
                    }
                });
        });

        if self.show_file_analysis {
            Window::new("文件格式")
                .open(&mut self.show_file_analysis)
                .show(ctx, |ui| {
                    ui.label("文件格式分析");
                });
        }


        if self.show_extract_dialog {
            Window::new("数据提取")
                .open(&mut self.show_extract_dialog)
                .show(ctx, |ui| {
                    ui.label("数据提取");
                });
        }

        if self.show_stereo_dialog {
            Window::new("立体视图")
                .open(&mut self.show_stereo_dialog)
                .show(ctx, |ui| {
                    ui.label("立体视图");
                });
        }

        if self.show_frame_browser {
            Window::new("帧浏览器")
                .open(&mut self.show_frame_browser)
                .show(ctx, |ui| {
                    ui.label("帧浏览器");
                });
        }

        if self.show_combine_dialog {
            Window::new("图像合成器")
                .open(&mut self.show_combine_dialog)
                .show(ctx, |ui| {
                    ui.label("图像合成器");
                });
        }

        if self.show_about {
            Window::new("关于")
                .open(&mut self.show_about)
                .show(ctx, |ui| {
                    ui.label("StegSolve (Rust + Egui)");
                    ui.label("版本: 0.1.0");
                    ui.label("作者: 你的名字");
                });
        }



        TopBottomPanel::bottom("controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // 缩放控制
                ui.add(Slider::new(&mut self.zoom_level, 0.1..=5.0).text("缩放"));

                // 导航按钮
                ui.separator();
                if ui.button("<").clicked() {
                    if let Some(transform) = &mut self.transform {
                        transform.back();
                        self.texture = None;
                        self.current_channel_text = transform.get_text();
                    }
                }

                if let Some(transform) = &self.transform {
                    ui.label(format!("通道: {}", transform.get_text()));
                }


                if ui.button(">").clicked() {
                    if let Some(transform) = &mut self.transform {
                        transform.forward();
                        self.texture = None;
                        self.current_channel_text = transform.get_text(); // 更新通道描述
                    }
                }

                // 文件操作
                ui.separator();
                if ui.button("打开").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        self.open_image(&path);
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



