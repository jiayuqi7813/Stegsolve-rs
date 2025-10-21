#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#![allow(rustdoc::missing_crate_level_docs)] // it's an example

mod transform;
mod fileanalysis;
mod stereo;
mod extractanlysis;
mod framebrowser;
mod combine;
mod apng_decoder;

use eframe::egui;
use egui::*;
use rfd;
use stereo::Stereo;
use extractanlysis::ExtractDialog;
use fileanalysis::FileAnalysis;
use framebrowser::FrameBrowser;

use transform::Transform;
use combine::ImageCombiner;

#[derive(Default)]
struct StegApp {
    transform: Option<Transform>,
    current_file_path: Option<String>,
    zoom_level: f32,
    texture: Option<egui::TextureHandle>,
    scroll_pos: Vec2, // 新增滚动位置记录

    stereo: Option<Stereo>,
    extract_dialog: Option<ExtractDialog>,
    frame_browser: Option<FrameBrowser>,
    combine_dialog: Option<ImageCombiner>,



    current_channel_text: String,
    show_file_analysis: bool,
    show_extract_dialog: bool,
    show_stereo_dialog: bool,
    show_frame_browser: bool,
    show_combine_dialog: bool,
    show_about: bool,

}



fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0]),
        ..Default::default()
    };
    if let Err(e)=eframe::run_native(
        "StegSolve-rs",
        options,
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "misans".to_owned(),
                std::sync::Arc::new(egui::FontData::from_static(
                    include_bytes!("../font/MiSans-Normal.ttf")
                )),
            );
            fonts.families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "misans".to_owned());

            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::<StegApp>::default())
        }),
    ) {
        eprintln!("Error: {}", e);
    }

}


impl StegApp {
    // pub fn new(cc: &eframe::CreationContext<'_>) -> Box<Self> {
    //     // 设置字体
    //     let mut fonts = egui::FontDefinitions::default();
    //     fonts.font_data.insert(
    //         "misans".to_owned(),
    //         std::sync::Arc::new(egui::FontData::from_static(
    //             include_bytes!("../font/MiSans-Normal.ttf")
    //         )),
    //     );
    //     fonts.families
    //         .entry(egui::FontFamily::Proportional)
    //         .or_default()
    //         .insert(0, "misans".to_owned());

    //     cc.egui_ctx.set_fonts(fonts);
    //     Box::new(Self::default())
    // }

    fn open_image(&mut self, path: &std::path::Path) {
        match image::open(path) {
            Ok(img) => {
                self.transform = Some(Transform::new(img));
                self.current_file_path = Some(path.to_string_lossy().to_string());
                self.texture = None;
                self.zoom_level = 1.0;
                self.scroll_pos = Vec2::ZERO;
                if let Some(t) = &self.transform {
                    self.stereo = Some(Stereo::new(t.get_image().clone()));
                }
                self.frame_browser = Some(framebrowser::FrameBrowser::new());
                if let Some(browser) = &mut self.frame_browser {
                    let _ = browser.load_frames(&self.current_file_path.as_ref().unwrap());
                }
                self.combine_dialog = Some(ImageCombiner::new(self.transform.as_ref().unwrap().get_image().clone()));
                
            }
            Err(e) => eprintln!("打开图片失败: {:?}", e),
        }
    }
}


impl eframe::App for StegApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // 添加拖放支持
        if !ctx.input(|i| i.raw.dropped_files.is_empty()) {
            // 获取拖放的第一个文件
            if let Some(dropped_file) = ctx.input(|i| i.raw.dropped_files.first().cloned()) {
                // 如果文件路径可用
                if let Some(path) = dropped_file.path {
                    self.open_image(&path);
                }
            }
        }

        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            menu::bar(ui, |ui| {
                // 文件菜单
                ui.menu_button("文件", |ui| {
                    if ui.button("打开").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                            self.open_image(&path);
                        }
                        ui.close_menu();    
                    }
                    if ui.button("另存为").clicked() {
                        if let Some(transform) = &self.transform {
                            if let Some(path) = rfd::FileDialog::new().save_file() {
                                transform.get_image().save(path).unwrap();
                            }
                        }
                        ui.close_menu();
                    }

                });

                // 分析菜单
                ui.menu_button("分析", |ui| {
                    if ui.button("文件格式").clicked() {
                        self.show_file_analysis = true;
                        ui.close_menu();    
                    }

                    if ui.button("数据提取").clicked() {
                        self.show_extract_dialog = true;
                        // 初始化数据提取对话框（仅在首次点击时创建）
                        if self.extract_dialog.is_none() {
                            self.extract_dialog = Some(ExtractDialog::default());
                        }
                        ui.close_menu();
                    }
                    if ui.button("立体视图").clicked() {
                        self.show_stereo_dialog = true;
                        ui.close_menu();
                    }
                    if ui.button("帧浏览器").clicked() {
                        self.show_frame_browser = true;
                        ui.close_menu();
                    }
                    if ui.button("图像合成器").clicked() {
                        self.show_combine_dialog = true;
                        ui.close_menu();
                    }
                });

                // 帮助菜单
                ui.menu_button("帮助", |ui| {
                    if ui.button("关于").clicked() {
                        self.show_about = true;
                        ui.close_menu();
                    }
                });
            });
        });

        CentralPanel::default().show(ctx, |ui| {
            ScrollArea::both()
                .id_salt("image_scroll")
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
            if let Some(file_path) = &self.current_file_path {
                let viewport_id = ViewportId::from_hash_of("file_analysis");
                let viewport = ViewportBuilder::default()
                    .with_title("文件分析")
                    .with_resizable(true)
                    .with_inner_size([600.0, 400.0])
                    .with_decorations(true);
                
                let mut should_close = false;

                ctx.show_viewport_immediate(
                    viewport_id,
                    viewport,
                    |ctx, _class| {
                        CentralPanel::default().show(ctx, |ui| {
                            if ctx.input(|i| i.viewport().close_requested()) {
                                should_close = true;
                            }

                            let mut analysis = FileAnalysis::new(file_path);
                            analysis.ui(ui);
                        });

                        if should_close {
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                    },
                );

                if should_close {
                    self.show_file_analysis = false;
                }

            }
        }

        if self.show_extract_dialog {
            if let Some(transform) = &self.transform {
                let viewport_id = ViewportId::from_hash_of("extract_dialog");
                let viewport = ViewportBuilder::default()
                    .with_title("数据提取")
                    .with_resizable(true)
                    //自动调整大小
                    .with_inner_size([810.0, 430.0])
                    .with_decorations(true);
        
                // 临时变量跟踪关闭状态
                let mut should_close = false;
        
                ctx.show_viewport_immediate(
                    viewport_id,
                    viewport,
                    |ctx, _class| {
                        CentralPanel::default().show(ctx, |ui| {
                            // 检查视口关闭命令（来自系统按钮）
                            if ctx.input(|i| i.viewport().close_requested()) {
                                should_close = true;
                            }
        
                            // 正常绘制对话框内容
                            if let Some(dialog) = self.extract_dialog.as_mut() {
                                if dialog.ui(ui, transform.get_image()) {
                                    should_close = true;
                                }
                            }
                        });
                        // 如果检测到关闭命令，执行关闭操作
                        if should_close {
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                    },
                );

                // 同步关闭状态到主程序
                if should_close  {
                    self.show_extract_dialog = false;
                }
            }
        }



        if self.show_stereo_dialog {
            if let Some(_transform) = &self.transform {
                let viewport_id = ViewportId::from_hash_of("stereo_dialog");
                let viewport = ViewportBuilder::default()
                    .with_title("立体图分析")
                    .with_resizable(true)
                    .with_inner_size([800.0, 600.0])
                    .with_decorations(true);

                let mut should_close = false;

                ctx.show_viewport_immediate(
                    viewport_id,
                    viewport,
                    |ctx, _class| {
                        CentralPanel::default().show(ctx, |ui| {
                            if ctx.input(|i| i.viewport().close_requested()) {
                                should_close = true;
                            }

                            if let Some(stereo) = self.stereo.as_mut() {
                                stereo.update(ctx, ui);
                            }
                        });

                        if should_close {
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                    },
                );

                if should_close {
                    self.show_stereo_dialog = false;
                }
            }

        }

        if self.show_frame_browser {
            if let Some(browser) = &mut self.frame_browser {
                let viewport_id = ViewportId::from_hash_of("frame_browser");
                let viewport = ViewportBuilder::default()
                    .with_title("帧浏览器")
                    .with_resizable(true)
                    .with_inner_size([800.0, 600.0])
                    .with_decorations(true);

                let mut should_close = false;

                ctx.show_viewport_immediate(
                    viewport_id,
                    viewport,
                    |ctx, _class| {
                        CentralPanel::default().show(ctx, |ui| {
                            if ctx.input(|i| i.viewport().close_requested()) {
                                should_close = true;
                            }

                            browser.ui(ui);
                        });

                        if should_close {
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                    },
                );

                if should_close {
                    self.show_frame_browser = false;
                }
            }
        }

        if self.show_combine_dialog {
            let viewport_id = ViewportId::from_hash_of("combine_dialog");
            let viewport = ViewportBuilder::default()
                .with_title("图像合成器")
                .with_resizable(true)
                .with_inner_size([800.0, 600.0])
                .with_decorations(true);
        
            let mut should_close = false;
        
            ctx.show_viewport_immediate(
                viewport_id,
                viewport,
                |ctx, _class| {
                    CentralPanel::default().show(ctx, |ui| {
                        if ctx.input(|i| i.viewport().close_requested()) {
                            should_close = true;
                        }
        
                        if let Some(combiner) = &mut self.combine_dialog {
                            combiner.update(ui);
                        }
                    });
        
                    if should_close {
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                },
            );
        
            if should_close {
                // 在关闭窗口时重置状态
                if let Some(combiner) = &mut self.combine_dialog {
                    combiner.reset();
                }
                self.show_combine_dialog = false;
            }
        }

        if self.show_about {
            Window::new("关于")
                .open(&mut self.show_about)
                .movable(true)
                .show(ctx, |ui| {
                    ui.label("StegSolve (Rust + Egui)");
                    ui.label("版本: 0.2.0");
                    ui.label("作者: Sn1waR");
                });
        }



        TopBottomPanel::bottom("controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // 处理鼠标滚轮和上下键
                let mut zoom_delta = 0.0;
                
                if let Some(scroll_delta) = ctx.input(|i| {
                    if i.pointer.hover_pos().is_some() {
                        Some(i.raw_scroll_delta.y)
                    } else {
                        None
                    }
                }) {
                    zoom_delta = scroll_delta * 0.001;
                }

                // 处理上下键
                ctx.input(|i| {
                    if i.key_down(egui::Key::ArrowUp) {
                        zoom_delta += 0.02;
                    }
                    if i.key_down(egui::Key::ArrowDown) {
                        zoom_delta -= 0.02;
                    }
                });

                self.zoom_level = (self.zoom_level + zoom_delta).clamp(0.1, 5.0);

                // 缩放控制
                ui.add(Slider::new(&mut self.zoom_level, 0.1..=5.0).text("缩放"));

                // 导航按钮
                ui.separator();
                
                // 检查键盘输入
                let left_key_pressed = ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft));
                let right_key_pressed = ctx.input(|i| i.key_pressed(egui::Key::ArrowRight));
                
                let left_button = egui::Button::new("<")
                    .fill(if left_key_pressed { ui.style().visuals.selection.bg_fill } else { ui.style().visuals.widgets.inactive.bg_fill });
                let right_button = egui::Button::new(">")
                    .fill(if right_key_pressed { ui.style().visuals.selection.bg_fill } else { ui.style().visuals.widgets.inactive.bg_fill });
                
                let left_clicked = ui.add(left_button).clicked();
                let right_clicked = ui.add(right_button).clicked();
                
                if let Some(transform) = &self.transform {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(200.0);
                        ui.label(format!("通道: {}", transform.get_text()));
                    });
                }

                if left_clicked || left_key_pressed {
                    if let Some(transform) = &mut self.transform {
                        transform.back();
                        self.texture = None;
                        self.current_channel_text = transform.get_text();
                    }
                }
                
                if right_clicked || right_key_pressed {
                    if let Some(transform) = &mut self.transform {
                        transform.forward();
                        self.texture = None;
                        self.current_channel_text = transform.get_text();
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
