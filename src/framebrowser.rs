use eframe::egui;
use egui::{ColorImage, TextureHandle, Ui};
use image::codecs::gif::GifDecoder;
use image::AnimationDecoder;
use image::{ImageError, ImageFormat, RgbaImage};
use std::path::Path;
use std::io::BufReader;
use crate::apng_decoder::{check_apng, ApngDecoder};

/// 帧浏览器：用于浏览、切换和保存图片帧
pub struct FrameBrowser {
    frames: Vec<RgbaImage>,
    textures: Vec<Option<TextureHandle>>,
    current_frame: usize,
}

impl FrameBrowser {
    /// 创建一个新的帧浏览器实例
    pub fn new() -> Self {
        Self {
            frames: Vec::new(),
            textures: Vec::new(),
            current_frame: 0,
        }
    }

    /// 从指定路径加载图像帧（支持GIF、APNG、WebP和静态图片）
    pub fn load_frames<P: AsRef<Path>>(&mut self, path: P) -> Result<(), ImageError> {
        // 清空现有帧
        self.frames.clear();
        self.textures.clear();
        self.current_frame = 0;

        let path = path.as_ref();
        
        // 首先检查是否为APNG格式（避免卡死）
        if let Ok(apng_info) = check_apng(path) {
            if apng_info.is_apng {
                println!("检测到APNG格式，帧数: {}", apng_info.frame_count);
                
                // 使用专门的APNG解码器
                match ApngDecoder::from_path(path) {
                    Ok(decoder) => {
                        let frames = decoder.into_frames();
                        println!("成功解码 {} 帧", frames.len());
                        for frame in frames {
                            self.frames.push(frame);
                            self.textures.push(None);
                        }
                        return Ok(());
                    }
                    Err(e) => {
                        eprintln!("APNG解码失败: {}, 尝试作为静态PNG加载", e);
                        // 失败时回退到加载第一帧
                        let img = image::open(path)?.to_rgba8();
                        self.frames.push(img);
                        self.textures.push(None);
                        return Ok(());
                    }
                }
            }
        }
        
        // 非APNG格式，使用常规方式加载
        let file = std::fs::File::open(path)?;
        let buf_reader = std::io::BufReader::new(file);
        let reader = image::ImageReader::new(buf_reader).with_guessed_format()?;
        
        if let Some(format) = reader.format() {
            match format {
                ImageFormat::Gif => {
                    // GIF动画处理
                    let file = std::fs::File::open(path)?;
                    let buffered = BufReader::new(file);
                    let decoder = GifDecoder::new(buffered)?;
                    let frames = decoder.into_frames().collect_frames()?;
                    
                    println!("GIF帧数: {}", frames.len());
                    for frame in frames {
                        self.frames.push(frame.into_buffer());
                        self.textures.push(None);
                    }
                    return Ok(());
                }
                ImageFormat::WebP => {
                    // WebP处理（动画支持有限）
                    let img = reader.decode()?.to_rgba8();
                    self.frames.push(img);
                    self.textures.push(None);
                    return Ok(());
                }
                _ => {
                    // 其他格式作为静态图像加载
                    let img = reader.decode()?.to_rgba8();
                    self.frames.push(img);
                    self.textures.push(None);
                    return Ok(());
                }
            }
        } else {
            // 无法判断格式时，尝试按静态图像加载
            let img = reader.decode()?.to_rgba8();
            self.frames.push(img);
            self.textures.push(None);
            return Ok(());
        }
    }


    /// 将 RgbaImage 转换为 egui 所需的 ColorImage
    fn image_to_color_image(img: &RgbaImage) -> ColorImage {
        let width = img.width() as usize;
        let height = img.height() as usize;
        ColorImage::from_rgba_unmultiplied([width, height], img.as_raw())
    }

    /// 在传入的 UI 中绘制帧浏览器界面
    pub fn ui(&mut self, ui: &mut Ui) {
        // 检查键盘输入
        let left_pressed = ui.ctx().input(|i| i.key_pressed(egui::Key::ArrowLeft));
        let right_pressed = ui.ctx().input(|i| i.key_pressed(egui::Key::ArrowRight));

        if !self.frames.is_empty() {
            if left_pressed {
                if self.current_frame == 0 {
                    self.current_frame = self.frames.len() - 1;
                } else {
                    self.current_frame -= 1;
                }
            }
            if right_pressed {
                self.current_frame = (self.current_frame + 1) % self.frames.len();
            }
        }

        ui.vertical(|ui| {
            // 如果没有加载帧，则提示
            if self.frames.is_empty() {
                ui.label("No frames loaded");
            } else {
                ui.label(format!("Frame: {} of {}", self.current_frame + 1, self.frames.len()));
                // 使用 ScrollArea 显示图片
                egui::ScrollArea::both().show(ui, |ui| {
                    let idx = self.current_frame;
                    // 若纹理尚未加载，则转换并缓存
                    if self.textures[idx].is_none() {
                        let color_img = Self::image_to_color_image(&self.frames[idx]);
                        let texture = ui.ctx().load_texture(
                            format!("frame_{}", idx),
                            color_img,
                            Default::default(),
                        );
                        self.textures[idx] = Some(texture);
                    }
                    if let Some(texture) = &self.textures[idx] {
                        // let image_size = texture.size_vec2();
                        ui.image(texture);
                    }
                });
                // 底部按钮区域
                ui.horizontal(|ui| {
                    let left_button = egui::Button::new("<")
                        .fill(if left_pressed { ui.style().visuals.selection.bg_fill } else { ui.style().visuals.widgets.inactive.bg_fill });
                    if ui.add(left_button).clicked() {
                        if self.current_frame == 0 {
                            self.current_frame = self.frames.len() - 1;
                        } else {
                            self.current_frame -= 1;
                        }
                    }

                    let right_button = egui::Button::new(">")
                        .fill(if right_pressed { ui.style().visuals.selection.bg_fill } else { ui.style().visuals.widgets.inactive.bg_fill });
                    if ui.add(right_button).clicked() {
                        self.current_frame = (self.current_frame + 1) % self.frames.len();
                    }
                    if ui.button("Save").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_file_name(&format!("frame{}.png", self.current_frame + 1))
                            .save_file()
                        {
                            if let Err(e) = self.frames[self.current_frame].save(&path) {
                                eprintln!("保存帧失败: {:?}", e);
                            }
                        }
                    }
                });
            }
        });
    }
}
