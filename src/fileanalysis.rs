use crc32fast::Hasher;
use eframe::egui::{Align, CentralPanel, Frame, Layout, ScrollArea, TopBottomPanel, Ui};
use rfd::FileDialog;
use std::fs::File;
use std::io::Read;
pub struct FileAnalysis {
    report: Vec<String>,
    scroll_to_bottom: bool,
}



impl FileAnalysis {
    pub fn new(file_path: &str) -> Self {
        Self {
            report: analyse_file_format(file_path),
            scroll_to_bottom: false,
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        // 底部按钮面板（始终固定显示）
        TopBottomPanel::bottom("bottom_panel")
            .show(ui.ctx(), |ui| {
                Frame::NONE
                    .fill(ui.style().visuals.window_fill)
                    .show(ui, |ui| {
                        ui.with_layout(
                            Layout::left_to_right(Align::Center)
                                .with_cross_justify(true),
                            |ui| {
                                if ui.button("复制到剪贴板").clicked() {
                                    ui.ctx().copy_text(self.report.join("\n"));
                                }
                                if ui.button("导出报告").clicked() {
                                    if let Some(path) = FileDialog::new()
                                        .add_filter("文本文件", &["txt"])
                                        .save_file()
                                    {
                                        if let Err(e) = std::fs::write(&path, self.report.join("\n")) {
                                            eprintln!("保存文件失败: {}", e);
                                        }
                                    }
                                }
                            }
                        );
                    });
            });

        // 中央内容区域（可滚动）
        CentralPanel::default().show(ui.ctx(), |ui| {
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(self.scroll_to_bottom)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    
                    // 显示报告内容
                    for line in &self.report {
                        ui.label(line);
                    }

                    // 自动滚动处理
                    if self.scroll_to_bottom {
                        ui.scroll_to_cursor(Some(Align::BOTTOM));
                        self.scroll_to_bottom = false;
                    }
                });
        });
    }
}


// 工具函数
fn uf(data: &[u8], offset: usize) -> u8 {
    if offset >= data.len() {
        0
    } else {
        data[offset]
    }
}

fn get_word_le(data: &[u8], offset: usize) -> u16 {
    if offset + 1 >= data.len() {
        0
    } else {
        u16::from_le_bytes([data[offset], data[offset + 1]])
    }
}

fn get_dword_le(data: &[u8], offset: usize) -> u32 {
    if offset + 3 >= data.len() {
        0
    } else {
        u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ])
    }
}

fn get_dword_be(data: &[u8], offset: usize) -> u32 {
    if offset + 3 >= data.len() {
        0
    } else {
        u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ])
    }
}

// 十六进制转储
fn hex_dump(data: &[u8], from: usize, to: usize, report: &mut Vec<String>) {
    if from >= data.len() {
        return;
    }

    report.push("十六进制:".to_string());
    for i in (from..=to.min(data.len() - 1)).step_by(16) {
        let mut line = String::new();
        for j in 0..16 {
            if i + j <= to && i + j < data.len() {
                line.push_str(&format!("{:02X} ", data[i + j]));
                if j == 7 {
                    line.push(' ');
                }
            }
        }
        report.push(line);
    }

    report.push("ASCII:".to_string());
    for i in (from..=to.min(data.len() - 1)).step_by(16) {
        let mut line = String::new();
        for j in 0..16 {
            if i + j <= to && i + j < data.len() {
                let c = data[i + j] as char;
                if c.is_ascii_graphic() {
                    line.push(c);
                } else {
                    line.push('.');
                }
                if j == 7 {
                    line.push(' ');
                }
            }
        }
        report.push(line);
    }
}

/// 分析 BMP 文件

// 改进BMP分析
fn analyse_bmp(data: &[u8], report: &mut Vec<String>) {
    if data.len() < 54 {
        report.push("文件太短，无法解析BMP头".to_string());
        return;
    }

    let file_size = get_dword_le(data, 2);
    let data_offset = get_dword_le(data, 10);
    let header_size = get_dword_le(data, 14);
    let width = get_dword_le(data, 18);
    let height = get_dword_le(data, 22);
    let planes = get_word_le(data, 26);
    let bit_count = get_word_le(data, 28);
    let compression = get_dword_le(data, 30);

    report.push("文件头信息:".to_string());
    report.push(format!("文件大小: {:X} ({}) 字节", file_size, file_size));
    report.push(format!("数据偏移: {:X} 字节", data_offset));
    report.push(format!("信息头大小: {:X} 字节", header_size));
    report.push(format!("宽度: {} 像素", width));
    report.push(format!("高度: {} 像素", height));
    report.push(format!("色彩平面数: {}", planes));
    report.push(format!("位深度: {} 位", bit_count));

    // 压缩方式
    let compression_type = match compression {
        0 => "无压缩",
        1 => "RLE 8位压缩",
        2 => "RLE 4位压缩",
        3 => "Bitfields",
        _ => "未知压缩方式",
    };
    report.push(format!("压缩方式: {} ({})", compression, compression_type));

    // 检查颜色表
    if bit_count <= 8 {
        let color_count = if get_dword_le(data, 46) == 0 {
            1 << bit_count
        } else {
            get_dword_le(data, 46)
        };

        report.push(format!("\n颜色表 ({} 个颜色):", color_count));
        let color_table_offset = 14 + header_size as usize;

        for i in 0..color_count as usize {
            let offset = color_table_offset + i * 4;
            if offset + 4 <= data.len() {
                report.push(format!(
                    "颜色 {}: B={:02X} G={:02X} R={:02X} A={:02X}",
                    i,
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3]
                ));
            }
        }
    }

    // 检查数据偏移
    if data_offset as usize > 54 {
        report.push("\n头部与数据之间的间隙:".to_string());
        hex_dump(data, 54, data_offset as usize - 1, report);
    }
}
/// 分析 PNG 文件
fn analyse_png(data: &[u8], report: &mut Vec<String>) {
    if data.len() < 8 || &data[0..8] != b"\x89PNG\r\n\x1a\n" {
        report.push("无效的 PNG 文件头".to_string());
        return;
    }

    report.push("文件头: 有效的PNG文件".to_string());
    let mut pos = 8;

    while pos + 12 <= data.len() {
        let length = get_dword_be(data, pos) as usize;
        let chunk_type = &data[pos + 4..pos + 8];
        let chunk_name = std::str::from_utf8(chunk_type).unwrap_or("未知");

        report.push(format!("\n块类型: {}", chunk_name));
        report.push(format!("数据长度: {} 字节", length));

        // CRC32校验
        let mut hasher = Hasher::new();
        hasher.update(&data[pos + 4..pos + 8 + length]);
        let calculated_crc = hasher.finalize();
        let file_crc = get_dword_be(data, pos + 8 + length);

        report.push(format!("CRC32: {:08X}", file_crc));
        if calculated_crc != file_crc {
            report.push(format!("计算得到的CRC32: {:08X} (不匹配)", calculated_crc));
        }

        // 特殊块分析
        match chunk_name {
            "IHDR" => {
                if length >= 13 {
                    let width = get_dword_be(data, pos + 8);
                    let height = get_dword_be(data, pos + 12);
                    let bit_depth = data[pos + 16];
                    let color_type = data[pos + 17];

                    report.push(format!("宽度: {}", width));
                    report.push(format!("高度: {}", height));
                    report.push(format!("位深度: {}", bit_depth));
                    report.push(format!("颜色类型: {}", color_type));
                }
            }
            "IDAT" => {
                report.push("图像数据块".to_string());
            }
            "IEND" => {
                report.push("文件结束标记".to_string());
                break;
            }
            _ => {
                if length > 0 {
                    report.push("数据内容:".to_string());
                    hex_dump(data, pos + 8, pos + 8 + length - 1, report);
                }
            }
        }

        pos += 12 + length;
    }
}

// 改进GIF分析
fn analyse_gif(data: &[u8], report: &mut Vec<String>) {
    if data.len() < 13 {
        report.push("文件太短，无法解析GIF头".to_string());
        return;
    }

    let version = std::str::from_utf8(&data[3..6]).unwrap_or("未知");
    report.push(format!("GIF版本: {}", version));

    let width = get_word_le(data, 6);
    let height = get_word_le(data, 8);
    report.push(format!("宽度: {} 像素", width));
    report.push(format!("高度: {} 像素", height));

    let flags = data[10];
    let global_color_table = (flags & 0x80) != 0;
    let color_resolution = ((flags >> 4) & 0x07) + 1;
    let sort_flag = (flags & 0x08) != 0;
    let size_of_global_color_table = if global_color_table {
        1 << ((flags & 0x07) + 1)
    } else {
        0
    };

    report.push(format!(
        "全局颜色表: {}",
        if global_color_table { "是" } else { "否" }
    ));
    report.push(format!("颜色分辨率: {}", color_resolution));
    report.push(format!("排序标志: {}", if sort_flag { "是" } else { "否" }));
    report.push(format!("全局颜色表大小: {}", size_of_global_color_table));

    let mut pos = 13;

    // 解析全局颜色表
    if global_color_table {
        report.push("\n全局颜色表:".to_string());
        for i in 0..size_of_global_color_table {
            if pos + 3 > data.len() {
                break;
            }
            report.push(format!(
                "颜色 {}: R={:02X} G={:02X} B={:02X}",
                i,
                data[pos],
                data[pos + 1],
                data[pos + 2]
            ));
            pos += 3;
        }
    }

    // 解析数据块
    while pos < data.len() {
        match data[pos] {
            0x2C => {
                if pos + 10 <= data.len() {
                    report.push("\n图像描述符:".to_string());
                    let left = get_word_le(data, pos + 1);
                    let top = get_word_le(data, pos + 3);
                    let width = get_word_le(data, pos + 5);
                    let height = get_word_le(data, pos + 7);

                    report.push(format!("左边界: {}", left));
                    report.push(format!("上边界: {}", top));
                    report.push(format!("宽度: {}", width));
                    report.push(format!("高度: {}", height));
                    report.push(format!("标志位: {:02X}", data[pos + 9]));
                }
                pos += 10;
            }
            0x21 => {
                if pos + 2 > data.len() {
                    break;
                }
                match data[pos + 1] {
                    0xF9 => {
                        report.push("\n图形控制扩展:".to_string());
                        if pos + 8 <= data.len() {
                            let block_size = data[pos + 2];
                            let flags = data[pos + 3];
                            let delay = get_word_le(data, pos + 4);
                            report.push(format!("块大小: {}", block_size));
                            report.push(format!("标志位: {:02X}", flags));
                            report.push(format!("延迟时间: {}", delay));
                        }
                        pos += 8;
                    }
                    0xFE => {
                        report.push("\n注释扩展:".to_string());
                        pos += 2;
                        while pos < data.len() && data[pos] != 0 {
                            let size = data[pos] as usize;
                            pos += 1;
                            if pos + size <= data.len() {
                                if let Ok(comment) = std::str::from_utf8(&data[pos..pos + size]) {
                                    report.push(format!("注释: {}", comment));
                                }
                                pos += size;
                            } else {
                                break;
                            }
                        }
                        pos += 1;
                    }
                    _ => {
                        report.push(format!("\n未知扩展块: {:02X}", data[pos + 1]));
                        pos += 2;
                    }
                }
            }
            0x3B => {
                report.push("\n文件结束标记".to_string());
                break;
            }
            _ => pos += 1,
        }
    }
}

fn analyse_jpg(data: &[u8], report: &mut Vec<String>) {
    let mut pos = 0;

    // 检查文件是否以 SOI (Start of Image) 开头
    if data.len() < 2 || data[0] != 0xFF || data[1] != 0xD8 {
        report.push("JPEG 文件不包含有效的 SOI 标记".to_string());
        return;
    }

    report.push("图像的开头 (SOI)".to_string());
    pos += 2;

    // 解析段
    while pos + 4 <= data.len() {
        if data[pos] != 0xFF {
            report.push(format!("无效的标记位置: {}", pos));
            break;
        }
        let marker = data[pos + 1];
        let length = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;

        report.push(format!("段标记: {:02X}", marker));
        report.push(format!("段长度: {} 字节", length));

        // 处理常见段
        match marker {
            0xC0..=0xC3 => {
                report.push("帧段 (Start of Frame)".to_string());
                if pos + length <= data.len() {
                    let height = u16::from_be_bytes([data[pos + 5], data[pos + 6]]);
                    let width = u16::from_be_bytes([data[pos + 7], data[pos + 8]]);
                    report.push(format!("宽度: {} 像素", width));
                    report.push(format!("高度: {} 像素", height));
                }
            }
            0xDA => {
                report.push("扫描数据段 (Start of Scan)".to_string());
            }
            0xD9 => {
                report.push("图像的结尾 (EOI)".to_string());
                break;
            }
            _ => {
                report.push("其他段".to_string());
            }
        }

        pos += length + 2;
    }

    if pos < data.len() {
        report.push(format!("文件末尾的附加字节数: {}", data.len() - pos));
    }
}

/// 分析文件格式
pub fn analyse_file_format(file_path: &str) -> Vec<String> {
    let mut report = vec!["文件格式报告".to_string()];

    // 读取文件
    if let Ok(mut file) = File::open(file_path) {
        let mut data = Vec::new();
        if file.read_to_end(&mut data).is_ok() {
            report.push(format!("文件: {}", file_path));
            report.push(format!("文件大小: {} 字节", data.len()));

            // 简单的文件格式检查
            if data.len() >= 2 && data[0] == b'B' && data[1] == b'M' {
                report.push("文件格式: BMP".to_string());
                analyse_bmp(&data, &mut report);
            } else if data.len() >= 4
                && data[0] == 0x89
                && data[1] == 0x50
                && data[2] == 0x4E
                && data[3] == 0x47
            {
                report.push("文件格式: PNG".to_string());
                analyse_png(&data, &mut report);
            } else if data.len() >= 6 && data[0] == b'G' && data[1] == b'I' && data[2] == b'F' {
                report.push("文件格式: GIF".to_string());
                analyse_gif(&data, &mut report);
            } else if data.len() >= 2 && data[0] == 0xFF && data[1] == 0xD8 {
                report.push("文件格式: JPEG".to_string());
                analyse_jpg(&data, &mut report);
            } else {
                report.push("文件格式未知".to_string());
            }
        } else {
            report.push("读取文件失败".to_string());
        }
    } else {
        report.push("无法打开文件".to_string());
    }

    report
}
