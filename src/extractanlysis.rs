use eframe::egui;
use egui::{Align, Color32, Context, Layout, ScrollArea, Ui};
use image::RgbaImage;
use std::fs::File;
use std::io::Write;

// 用于文件对话框的库（需要在 Cargo.toml 中添加 rfd 依赖）
// rfd 文档：https://github.com/emilk/rfd

// ──────────────────────────────
// 定义提取选项的枚举

#[derive(PartialEq)]
pub enum ExtractDirection {
    Row,
    Column,
}

#[derive(PartialEq)]
pub enum BitOrder {
    MSBFirst,
    LSBFirst,
}

#[derive(PartialEq)]
pub enum RgbOrder {
    RGB,
    RBG,
    GRB,
    GBR,
    BRG,
    BGR,
}

// ──────────────────────────────
// 每个通道的位选择状态，数组顺序约定：索引0对应通道最高位（7），索引7对应最低位（0）
pub struct ChannelSelection {
    pub name: &'static str,
    pub bits: [bool; 8],
}

impl ChannelSelection {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            bits: [false; 8],
        }
    }
}

// ──────────────────────────────
// ExtractDialog 保存了所有的 UI 状态和提取数据
pub struct ExtractDialog {
    /// 是否显示此对话框，调用者可根据该值决定是否移除此对话框
    pub open: bool,
    /// 通道选择（Red, Green, Blue, Alpha）
    pub channel_selections: Vec<ChannelSelection>,
    /// 提取方向：按行 / 按列
    pub extract_direction: ExtractDirection,
    /// 位顺序：MSB优先 / LSB优先
    pub bit_order: BitOrder,
    /// RGB 通道的顺序
    pub rgb_order: RgbOrder,
    /// 预览中是否包含十六进制转储
    pub preview_hex_dump: bool,
    /// 预览文本（只读）
    pub preview_text: String,
    /// 提取后的二进制数据
    pub extract_data: Vec<u8>,
}

impl Default for ExtractDialog {
    fn default() -> Self {
        Self {
            open: true,
            channel_selections: vec![
                ChannelSelection::new("Red"),
                ChannelSelection::new("Green"),
                ChannelSelection::new("Blue"),
                ChannelSelection::new("Alpha"),
            ],
            extract_direction: ExtractDirection::Row,
            bit_order: BitOrder::MSBFirst,
            rgb_order: RgbOrder::RGB,
            preview_hex_dump: true,
            preview_text: String::new(),
            extract_data: Vec::new(),
        }
    }
}

impl ExtractDialog {
    /// 在 egui 的 UI 内绘制对话框，image 为待提取数据的图像
    pub fn ui(&mut self, ui: &mut Ui, image: &RgbaImage) {
        // 外层采用垂直布局
        ui.vertical(|ui| {
            // ── 预览设置 ─────────────────────────────
            ui.group(|ui| {
                ui.label("预览设置");
                ui.horizontal(|ui| {
                    ui.label("在预览中包含十六进制转储");
                    ui.checkbox(&mut self.preview_hex_dump, "");
                });
            });

            ui.separator();

            // ── 选项区域：左侧为位平面选择，右侧为提取选项 ─────────────────────────────
            ui.horizontal(|ui| {
                // 位平面选择·
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label("位平面选择");
                        ui.add_space(4.0); // 标题和选项之间添加间距

                        egui::Grid::new("channel_selection_grid")
                            .spacing([8.0, 4.0]) // 设置水平和垂直间距
                            .show(ui, |ui| {
                                for channel in self.channel_selections.iter_mut() {
                                    // 通道名称（固定宽度）
                                    ui.add_sized([30.0, 20.0], egui::Label::new(channel.name));

                                    // "全选"复选框（固定宽度）
                                    let all_selected = channel.bits.iter().all(|&b| b);
                                    let mut all_sel = all_selected;
                                    if ui
                                        .add_sized(
                                            [40.0, 20.0],
                                            egui::Checkbox::new(&mut all_sel, "全选"),
                                        )
                                        .changed()
                                    {
                                        for b in channel.bits.iter_mut() {
                                            *b = all_sel;
                                        }
                                    }

                                    // 显示位复选框（从高位到低位，固定宽度）
                                    for (i, bit) in channel.bits.iter_mut().enumerate() {
                                        ui.add_sized(
                                            [24.0, 20.0],
                                            egui::Checkbox::new(bit, (7 - i).to_string()),
                                        );
                                    }

                                    ui.end_row(); // 结束当前行，开始新行
                                }
                            });
                        });
                });

                // 提取选项
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label("提取选项");
                        ui.add_space(4.0); // 只保留一个小间距

                        egui::Grid::new("extract_options_grid")
                            .spacing([8.0, 2.0]) // 减小垂直间距
                            .show(ui, |ui| {
                                // 提取方向：按行 / 按列
                                ui.label("提取方向:");
                                ui.radio_value(
                                    &mut self.extract_direction,
                                    ExtractDirection::Row,
                                    "按行",
                                );
                                ui.radio_value(
                                    &mut self.extract_direction,
                                    ExtractDirection::Column,
                                    "按列",
                                );
                                ui.end_row();

                                // 位顺序：MSB优先 / LSB优先
                                ui.label("位顺序:");
                                ui.radio_value(&mut self.bit_order, BitOrder::MSBFirst, "MSB优先");
                                ui.radio_value(&mut self.bit_order, BitOrder::LSBFirst, "LSB优先");
                                ui.end_row();

                                // RGB 顺序（分成两行显示）
                                ui.label("RGB顺序:");
                                egui::Grid::new("rgb_order_grid").show(ui, |ui| {
                                    ui.radio_value(&mut self.rgb_order, RgbOrder::RGB, "RGB");
                                    ui.radio_value(&mut self.rgb_order, RgbOrder::RBG, "RBG");
                                    ui.radio_value(&mut self.rgb_order, RgbOrder::GRB, "GRB");
                                    ui.end_row();
                                    ui.radio_value(&mut self.rgb_order, RgbOrder::GBR, "GBR");
                                    ui.radio_value(&mut self.rgb_order, RgbOrder::BRG, "BRG");
                                    ui.radio_value(&mut self.rgb_order, RgbOrder::BGR, "BGR");
                                });
                                ui.end_row();
                            });
                    });
                });
            });

            ui.separator();

            // ── 预览区域 ─────────────────────────────
            ui.group(|ui| {
                ui.label("预览");
                ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.preview_text)
                            .font(egui::TextStyle::Monospace)
                            .code_editor()
                            .desired_rows(10)
                            .lock_focus(true)
                            .desired_width(f32::INFINITY),
                    );
                });
            });

            ui.separator();

            // ── 按钮区域 ─────────────────────────────
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // “取消”按钮：关闭对话框
                    if ui.button("取消").clicked() {
                        self.open = false;
                    }
                    // “保存二进制”按钮：先生成提取数据，再保存二进制文件
                    if ui.button("保存二进制").clicked() {
                        self.generate_extract(image);
                        self.save_binary();
                    }
                    // “保存文本”按钮：生成预览后保存预览文本
                    if ui.button("保存文本").clicked() {
                        self.generate_extract(image);
                        self.generate_preview();
                        self.save_preview();
                    }
                    // “预览”按钮：生成预览数据并显示到预览区域
                    if ui.button("预览").clicked() {
                        self.generate_extract(image);
                        self.generate_preview();
                    }
                });
            });
        });
    }

    // ─────────────────────────────
    // 内部方法：根据通道选择生成掩码，返回 (mask, maskbits)
    fn get_mask(&self) -> (u32, u32) {
        let mut mask = 0u32;
        let mut maskbits = 0u32;
        // 按通道顺序（Red, Green, Blue, Alpha），每个通道内按顺序（数组索引0对应位7）
        for (channel_index, channel) in self.channel_selections.iter().enumerate() {
            for (bit_index, &selected) in channel.bits.iter().enumerate() {
                let flat_index = channel_index * 8 + bit_index; // 0..32
                if selected {
                    let shift = 31 - flat_index;
                    mask |= 1 << shift;
                    maskbits += 1;
                }
            }
        }
        (mask, maskbits)
    }

    // 获取提取顺序选项：返回 (row_first, lsb_first, rgb_order)
    fn get_bit_order_options(&self) -> (bool, bool, u8) {
        let row_first = self.extract_direction == ExtractDirection::Row;
        let lsb_first = self.bit_order == BitOrder::LSBFirst;
        let rgb_order = match self.rgb_order {
            RgbOrder::RGB => 1,
            RgbOrder::RBG => 2,
            RgbOrder::GRB => 3,
            RgbOrder::GBR => 4,
            RgbOrder::BRG => 5,
            RgbOrder::BGR => 6,
        };
        (row_first, lsb_first, rgb_order)
    }

    // 向提取缓冲区添加一个位
    fn add_bit(extract: &mut Vec<u8>, bit_pos: &mut u8, byte_pos: &mut usize, num: u8) {
        if num != 0 {
            if let Some(byte) = extract.get_mut(*byte_pos) {
                *byte += *bit_pos;
            }
        }
        *bit_pos >>= 1;
        if *bit_pos >= 1 {
            return;
        }
        *bit_pos = 128;
        *byte_pos += 1;
        if *byte_pos < extract.len() {
            extract[*byte_pos] = 0;
        }
    }

    // 提取 8 位
    fn extract_8bits(
        extract: &mut Vec<u8>,
        bit_pos: &mut u8,
        byte_pos: &mut usize,
        next_byte: u32,
        mut current_mask: u32,
        mask: u32,
        lsb_first: bool,
    ) {
        for _ in 0..8 {
            if mask & current_mask != 0 {
                let bit_val = if next_byte & current_mask != 0 { 1 } else { 0 };
                Self::add_bit(extract, bit_pos, byte_pos, bit_val);
            }
            if lsb_first {
                current_mask <<= 1;
            } else {
                current_mask >>= 1;
            }
        }
    }

    // 根据当前选项，从单个像素中提取位
    fn extract_bits(
        extract: &mut Vec<u8>,
        bit_pos: &mut u8,
        byte_pos: &mut usize,
        next_byte: u32,
        mask: u32,
        lsb_first: bool,
        rgb_order: u8,
    ) {
        if lsb_first {
            // LSB 优先：Alpha 通道（从最低位开始）
            Self::extract_8bits(
                extract,
                bit_pos,
                byte_pos,
                next_byte,
                1 << 0,
                mask,
                lsb_first,
            );
            // RGB 通道，按选定顺序
            let channels = match rgb_order {
                1 => [1 << 8, 1 << 16, 1 << 24], // RGB
                2 => [1 << 8, 1 << 24, 1 << 16], // RBG
                3 => [1 << 16, 1 << 8, 1 << 24], // GRB
                4 => [1 << 16, 1 << 24, 1 << 8], // GBR
                5 => [1 << 24, 1 << 8, 1 << 16], // BRG
                _ => [1 << 24, 1 << 16, 1 << 8], // BGR
            };
            for &shift in channels.iter() {
                Self::extract_8bits(
                    extract, bit_pos, byte_pos, next_byte, shift, mask, lsb_first,
                );
            }
        } else {
            // MSB 优先：Alpha 通道（从最高位开始）
            Self::extract_8bits(
                extract,
                bit_pos,
                byte_pos,
                next_byte,
                1 << 7,
                mask,
                lsb_first,
            );
            let channels = match rgb_order {
                1 => [1 << 31, 1 << 23, 1 << 15], // RGB
                2 => [1 << 31, 1 << 15, 1 << 23], // RBG
                3 => [1 << 23, 1 << 31, 1 << 15], // GRB
                4 => [1 << 23, 1 << 15, 1 << 31], // GBR
                5 => [1 << 15, 1 << 31, 1 << 23], // BRG
                _ => [1 << 15, 1 << 23, 1 << 31], // BGR
            };
            for &shift in channels.iter() {
                Self::extract_8bits(
                    extract, bit_pos, byte_pos, next_byte, shift, mask, lsb_first,
                );
            }
        }
    }

    /// 根据当前设置和图像生成提取数据
    pub fn generate_extract(&mut self, image: &RgbaImage) {
        let (mask, maskbits) = self.get_mask();
        let (row_first, lsb_first, rgb_order) = self.get_bit_order_options();

        let total_bits = (image.width() * image.height()) as u32 * maskbits;
        let len = ((total_bits + 7) / 8) as usize;
        self.extract_data = vec![0u8; len];
        let mut bit_pos = 128u8;
        let mut byte_pos = 0usize;

        if row_first {
            for y in 0..image.height() {
                for x in 0..image.width() {
                    let pixel = image.get_pixel(x, y);
                    // 将 [r, g, b, a] 按大端顺序转换为 u32
                    let rgba = u32::from_be_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]);
                    Self::extract_bits(
                        &mut self.extract_data,
                        &mut bit_pos,
                        &mut byte_pos,
                        rgba,
                        mask,
                        lsb_first,
                        rgb_order,
                    );
                }
            }
        } else {
            for x in 0..image.width() {
                for y in 0..image.height() {
                    let pixel = image.get_pixel(x, y);
                    let rgba = u32::from_be_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]);
                    Self::extract_bits(
                        &mut self.extract_data,
                        &mut bit_pos,
                        &mut byte_pos,
                        rgba,
                        mask,
                        lsb_first,
                        rgb_order,
                    );
                }
            }
        }
    }

    /// 生成预览文本，并更新内部的 preview_text 字段
    pub fn generate_preview(&mut self) {
        let extract = &self.extract_data;
        let mut preview = String::new();
        let hex_dump = self.preview_hex_dump;
        // 每 16 字节一行
        for chunk_start in (0..extract.len()).step_by(16) {
            if hex_dump {
                for j in 0..16 {
                    if chunk_start + j < extract.len() {
                        preview.push_str(&format!("{:02x}", extract[chunk_start + j]));
                        if j == 7 {
                            preview.push(' ');
                        }
                    }
                }
                preview.push_str("  ");
            }
            for j in 0..16 {
                if chunk_start + j < extract.len() {
                    let c = extract[chunk_start + j] as char;
                    if c.is_ascii_graphic() || c.is_ascii_whitespace() {
                        preview.push(c);
                    } else {
                        preview.push('.');
                    }
                    if j == 7 {
                        preview.push(' ');
                    }
                }
            }
            preview.push('\n');
        }
        self.preview_text = preview;
    }

    /// 调用文件对话框保存预览文本（保存为文本文件）
    pub fn save_preview(&self) {
        if let Some(path) = rfd::FileDialog::new().set_title("保存预览文本").save_file() {
            if let Ok(mut file) = File::create(path) {
                if let Err(e) = file.write_all(self.preview_text.as_bytes()) {
                    eprintln!("保存文件失败: {}", e);
                }
            }
        }
    }

    /// 调用文件对话框保存提取数据（保存为二进制文件）
    pub fn save_binary(&self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("保存二进制数据")
            .save_file()
        {
            if let Ok(mut file) = File::create(path) {
                if let Err(e) = file.write_all(&self.extract_data) {
                    eprintln!("保存文件失败: {}", e);
                }
            }
        }
    }
}
