use glib::MainContext;
use gtk::{prelude::*};
use gtk::{self, ApplicationWindow, Button, CheckButton, Label, TextView, ScrolledWindow};
use gtk::{Box, Frame, Orientation};
use gtk::FileDialog;
use std::cell::RefCell;
use std::fs::File;
use std::io::Write;
use std::rc::Rc;
use image::RgbaImage;

pub struct ExtractDialog {
    window: ApplicationWindow,
    bit_checkboxes: Vec<CheckButton>,
    preview_text: TextView,
    extract_data: RefCell<Vec<u8>>,
    // 单选按钮组
    extract_by_row: CheckButton,
    extract_by_col: CheckButton,
    msb_first: CheckButton, 
    lsb_first: CheckButton,
    rgb_order_buttons: Vec<CheckButton>,

    hex_dump_enabled: CheckButton,
}

impl ExtractDialog {
    pub fn new(parent: &ApplicationWindow, image: &RgbaImage) -> Self {
        // 创建主窗口
        let window = ApplicationWindow::builder()
            .transient_for(parent)
            .modal(true)
            .title("数据提取")
            .default_width(800)
            .default_height(600)
            .build();
    
        // 主布局
        let main_box = Box::new(Orientation::Vertical, 5);
        main_box.set_margin_top(10);
        main_box.set_margin_bottom(10);
        main_box.set_margin_start(10);
        main_box.set_margin_end(10);
    
        // === 选项面板 ===
        let options_box = Box::new(Orientation::Horizontal, 10);
        options_box.set_hexpand(true);
    
        // -- 位平面选择框架
        let bit_planes_frame = Frame::builder()
            .label("位平面选择")
            .margin_start(5)
            .margin_end(5)
            .build();
    
        // 使用 Grid 布局
        let bit_planes_grid = gtk::Grid::new();
        bit_planes_grid.set_row_spacing(5);
        bit_planes_grid.set_column_spacing(5);
        bit_planes_grid.set_margin_top(5);
        bit_planes_grid.set_margin_bottom(5);
    
        // RGBA通道复选框
        let mut bit_checkboxes = Vec::new();
        for (row, channel) in ["Red", "Green", "Blue", "Alpha"].iter().enumerate() {
            // 添加通道标签
            let label = Label::new(Some(channel));
            bit_planes_grid.attach(&label, 0, row as i32, 1, 1);
    
            // 添加全选按钮
            let all_btn = CheckButton::with_label("全选");
            bit_planes_grid.attach(&all_btn, 1, row as i32, 1, 1);
    
            // 添加位复选框
            let mut channel_boxes = Vec::new();
            for i in (0..8).rev() {
                let cb = CheckButton::with_label(&i.to_string());
                bit_planes_grid.attach(&cb, 2 + (7 - i) as i32, row as i32, 1, 1);
                channel_boxes.push(cb.clone());
            }
    
            // 全选按钮行为
            let boxes_clone = channel_boxes.clone();
            all_btn.connect_toggled(move |btn| {
                let active = btn.is_active();
                boxes_clone.iter().for_each(|cb| cb.set_active(active));
            });
    
            bit_checkboxes.extend(channel_boxes);
        }
    
        bit_planes_frame.set_child(Some(&bit_planes_grid));
        options_box.append(&bit_planes_frame);
    
        // -- 提取选项框架
        let extract_options_frame = Frame::builder()
            .label("提取选项")
            .margin_start(5)
            .margin_end(5)
            .build();
    
        // 使用 Grid 布局
        let extract_options_grid = gtk::Grid::new();
        extract_options_grid.set_row_spacing(5);
        extract_options_grid.set_column_spacing(10);
        extract_options_grid.set_margin_top(5);
        extract_options_grid.set_margin_bottom(5);
    
        // === 提取方向 ===
        let extract_by_row = CheckButton::with_label("按行");
        let extract_by_col = CheckButton::with_label("按列");
        extract_by_row.set_group(Some(&extract_by_col));
        extract_by_row.set_active(true);
    
        let direction_label = Label::new(Some("提取方向:"));
        extract_options_grid.attach(&direction_label, 0, 0, 1, 1);
        extract_options_grid.attach(&extract_by_row, 1, 0, 1, 1);
        extract_options_grid.attach(&extract_by_col, 2, 0, 1, 1);
    
        // === 位顺序 ===
        let msb_first = CheckButton::with_label("MSB优先");
        let lsb_first = CheckButton::with_label("LSB优先");
        msb_first.set_group(Some(&lsb_first));
        msb_first.set_active(true);
    
        let bit_order_label = Label::new(Some("位顺序:"));
        extract_options_grid.attach(&bit_order_label, 0, 1, 1, 1);
        extract_options_grid.attach(&msb_first, 1, 1, 1, 1);
        extract_options_grid.attach(&lsb_first, 2, 1, 1, 1);
    
        // === RGB顺序 ===
        let mut rgb_order_buttons = Vec::new();
        let orders = ["RGB", "RBG", "GRB", "GBR", "BRG", "BGR"];
        let mut prev_btn: Option<CheckButton> = None;
    
        for order in orders {
            let btn = CheckButton::with_label(order);
            if let Some(ref p) = prev_btn {
                btn.set_group(Some(p));
            }
            prev_btn = Some(btn.clone());
            rgb_order_buttons.push(btn);
        }
    
        if let Some(first) = rgb_order_buttons.first() {
            first.set_active(true);
        }
    
        let rgb_order_label = Label::new(Some("RGB顺序:"));
        extract_options_grid.attach(&rgb_order_label, 0, 2, 1, 1);
    
        // 将 RGB 顺序按钮分成两行
        let rgb_order_grid = gtk::Grid::new();
        rgb_order_grid.set_row_spacing(5);
        rgb_order_grid.set_column_spacing(5);
    
        for (i, btn) in rgb_order_buttons.iter().enumerate() {
            let row = i / 3; // 每行 3 个按钮
            let col = i % 3;
            rgb_order_grid.attach(btn, col as i32, row as i32, 1, 1);
        }
    
        extract_options_grid.attach(&rgb_order_grid, 1, 2, 2, 1);
    
        // 将提取选项框架添加到主布局
        extract_options_frame.set_child(Some(&extract_options_grid));
        options_box.append(&extract_options_frame);

        // == 预览设置 ==
        let preview_settings_frame = Frame::builder()
            .label("预览设置")
            .margin_start(5)
            .margin_end(5)
            .margin_top(5) 
            .build();

        let preview_settings_box = Box::new(Orientation::Horizontal, 5);
        preview_settings_box.set_margin_top(5);
        preview_settings_box.set_margin_bottom(5);

        let hex_dump_label = Label::new(Some("在预览中包含十六进制转储"));
        preview_settings_box.append(&hex_dump_label);

        let hex_dump_enabled = CheckButton::new();
        hex_dump_enabled.set_active(true);
        preview_settings_box.append(&hex_dump_enabled);

        preview_settings_frame.set_child(Some(&preview_settings_box));
        main_box.append(&preview_settings_frame);
    
        main_box.append(&options_box);
    
        // === 预览区域 ===
        let preview_frame = Frame::builder()
            .label("预览")
            .margin_top(10)
            .margin_bottom(10)
            .vexpand(true)
            .build();
    
        let scroll = ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .build();
    
        let preview_text = TextView::builder()
            .editable(false)
            .monospace(true)
            .build();
    
        scroll.set_child(Some(&preview_text));
        preview_frame.set_child(Some(&scroll));
    
        main_box.append(&preview_frame);
    
        // === 按钮区域 ===
        let button_box = Box::new(Orientation::Horizontal, 5);
        button_box.set_halign(gtk::Align::End);
        button_box.set_margin_top(10);
    
        let preview_btn = Button::with_label("预览");
        let save_text_btn = Button::with_label("保存文本");
        let save_bin_btn = Button::with_label("保存二进制");
        let cancel_btn = Button::with_label("取消");
    
        button_box.append(&preview_btn);
        button_box.append(&save_text_btn);
        button_box.append(&save_bin_btn);
        button_box.append(&cancel_btn);
    
        main_box.append(&button_box);
        window.set_child(Some(&main_box));
    
        // 取消按钮行为
        let window_clone = window.clone();
        cancel_btn.connect_clicked(move |_| {
            window_clone.close();
        });
    
        let dialog = Self {
            window,
            bit_checkboxes,
            preview_text,
            extract_data: RefCell::new(Vec::new()),
            extract_by_row,
            extract_by_col,
            msb_first,
            lsb_first,
            rgb_order_buttons,
            hex_dump_enabled,
        };
    
        // 预览按钮行为
        let image_clone = image.clone();
        let dialog_clone = dialog.clone();
        preview_btn.connect_clicked(move |_| {
            dialog_clone.generate_extract(&image_clone);
            dialog_clone.generate_preview();
        });
    
        // 保存文本按钮
        let image_clone = image.clone();
        let dialog_clone = dialog.clone();
        save_text_btn.connect_clicked(move |_| {
            dialog_clone.generate_extract(&image_clone);
            dialog_clone.generate_preview();
            dialog_clone.save_preview();
        });
    
        // 保存二进制按钮
        let image_clone = image.clone();
        let dialog_clone = dialog.clone();
        save_bin_btn.connect_clicked(move |_| {
            dialog_clone.generate_extract(&image_clone);
            dialog_clone.save_binary();
        });
    
        dialog
    }

    pub fn show(&self) {
        self.window.show();
    }


    fn get_mask(&self) -> (u32, u32) {
        let mut mask = 0u32;
        let mut maskbits = 0u32;
        
        // 按ARGB顺序,从高位到低位检查
        for (i, cb) in self.bit_checkboxes.iter().enumerate() {
            if cb.is_active() {
                let shift = 31 - i;  // 从最高位31开始
                mask |= 1 << shift;
                maskbits += 1;
            }
        }
        
        (mask, maskbits)
    }
    // 获取提取顺序选项
    fn get_bit_order_options(&self) -> (bool, bool, u8) {
        let row_first = self.extract_by_row.is_active();
        let lsb_first = self.lsb_first.is_active();
        
        // 确定RGB通道顺序
        let rgb_order = self.rgb_order_buttons.iter()
            .position(|btn| btn.is_active())
            .map(|pos| pos as u8 + 1)
            .unwrap_or(1);
            
        (row_first, lsb_first, rgb_order)
    }

    // 向提取缓冲区添加一个位
    fn add_bit(extract: &mut Vec<u8>, bit_pos: &mut u8, byte_pos: &mut usize, num: u8) {
        if num != 0 {
            extract[*byte_pos] += *bit_pos;
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

    // 提取8位
    fn extract_8bits(
        extract: &mut Vec<u8>,
        bit_pos: &mut u8,
        byte_pos: &mut usize,
        next_byte: u32,
        bit_mask: u32,
        mask: u32,
        lsb_first: bool
    ) {
        let mut current_mask = bit_mask;
        for _ in 0..8 {
            if mask & current_mask != 0 {
                Self::add_bit(extract, bit_pos, byte_pos, 
                    if next_byte & current_mask != 0 { 1 } else { 0 });
            }
            if lsb_first {
                current_mask <<= 1;
            } else {
                current_mask >>= 1;
            }
        }
    }

    // 从像素提取位
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
            // LSB 优先：从每个通道的最低位开始提取（位0开始）
            // Alpha 通道（位0-7）
            Self::extract_8bits(extract, bit_pos, byte_pos, next_byte, 1 << 0, mask, lsb_first);
    
            // RGB 通道按顺序处理
            let channels = match rgb_order {
                1 => [(1 << 8), (1 << 16), (1 << 24)],  // RGB（Blue, Green, Red）
                2 => [(1 << 8), (1 << 24), (1 << 16)],  // RBG
                3 => [(1 << 16), (1 << 8), (1 << 24)],  // GRB
                4 => [(1 << 16), (1 << 24), (1 << 8)],  // GBR
                5 => [(1 << 24), (1 << 8), (1 << 16)],  // BRG
                _ => [(1 << 24), (1 << 16), (1 << 8)],  // BGR
            };
    
            for &shift in channels.iter() {
                Self::extract_8bits(extract, bit_pos, byte_pos, next_byte, shift, mask, lsb_first);
            }
        } else {
            // MSB 优先：从每个通道的最高位开始提取
            // Alpha 通道（位7-0）
            Self::extract_8bits(extract, bit_pos, byte_pos, next_byte, 1 << 7, mask, lsb_first);
    
            // RGB 通道按顺序处理
            let channels = match rgb_order {
                1 => [(1 << 31), (1 << 23), (1 << 15)], // RGB（Red, Green, Blue）
                2 => [(1 << 31), (1 << 15), (1 << 23)], // RBG
                3 => [(1 << 23), (1 << 31), (1 << 15)], // GRB
                4 => [(1 << 23), (1 << 15), (1 << 31)], // GBR
                5 => [(1 << 15), (1 << 31), (1 << 23)], // BRG
                _ => [(1 << 15), (1 << 23), (1 << 31)], // BGR
            };
    
            for &shift in channels.iter() {
                Self::extract_8bits(extract, bit_pos, byte_pos, next_byte, shift, mask, lsb_first);
            }
        }
    }

    // 生成提取数据
    fn generate_extract(&self, image: &RgbaImage) {
        let (mask, maskbits) = self.get_mask();
        let (row_first, lsb_first, rgb_order) = self.get_bit_order_options();

        // 计算提取字节数
        let total_bits = (image.height() * image.width()) as u32 * maskbits;
        let len = (total_bits + 7) / 8;
        
        let mut extract = vec![0u8; len as usize];
        let mut bit_pos = 128u8;
        let mut byte_pos = 0usize;

        if row_first {
            // 按行遍历
            for y in 0..image.height() {
                for x in 0..image.width() {
                    let pixel = image.get_pixel(x, y);
                    let rgba = u32::from_be_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]);
                    Self::extract_bits(&mut extract, &mut bit_pos, &mut byte_pos, 
                                     rgba, mask, lsb_first, rgb_order);
                }
            }
        } else {
            // 按列遍历
            for x in 0..image.width() {
                for y in 0..image.height() {
                    let pixel = image.get_pixel(x, y);
                    let rgba = u32::from_be_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]);
                    Self::extract_bits(&mut extract, &mut bit_pos, &mut byte_pos,
                                     rgba, mask, lsb_first, rgb_order);
                }
            }
        }

        *self.extract_data.borrow_mut() = extract;
    }

    // 生成预览
    fn generate_preview(&self) {
        let extract = self.extract_data.borrow();
        let mut preview = String::new();
        let hex_dump = self.hex_dump_enabled.is_active();
    
        // 按16字节一组显示
        for chunk_start in (0..extract.len()).step_by(16) {
            // 十六进制部分
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
    
            // ASCII部分 
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
    
        // 更新预览文本
        let buffer = self.preview_text.buffer();
        buffer.set_text(&preview);
    }

       // 保存为文本文件
    fn save_preview(&self) {
        let dialog = FileDialog::builder()
            .title("保存预览文本")
            .accept_label("保存")
            .modal(true)
            .build();

        let future = dialog.save_future(Some(&self.window));
        let preview_text = self.preview_text.clone();
        
        MainContext::default().spawn_local(async move {
            match future.await {
                Ok(file) => {
                    if let Some(path) = file.path() {
                        if let Ok(mut file) = File::create(path) {
                            let text = preview_text.buffer()
                                .text(&preview_text.buffer().start_iter(),
                                     &preview_text.buffer().end_iter(),
                                     false);
                            if let Err(e) = file.write_all(text.as_bytes()) {
                                eprintln!("保存文件失败: {}", e);
                            }
                        }
                    }
                },
                Err(e) => eprintln!("保存对话框错误: {}", e),
            }
        });
    }

    // 保存为二进制文件
    fn save_binary(&self) {
        let dialog = FileDialog::builder()
            .title("保存二进制数据")
            .accept_label("保存") 
            .modal(true)
            .build();

        let future = dialog.save_future(Some(&self.window));
        let extract_data = self.extract_data.clone();

        MainContext::default().spawn_local(async move {
            match future.await {
                Ok(file) => {
                    if let Some(path) = file.path() {
                        if let Ok(mut file) = File::create(path) {
                            if let Err(e) = file.write_all(&extract_data.borrow()) {
                                eprintln!("保存文件失败: {}", e);
                            }
                        }
                    }
                },
                Err(e) => eprintln!("保存对话框错误: {}", e),
            }
        });
    }

}


// 需要 Clone trait 以支持按钮回调中使用
impl Clone for ExtractDialog {
    fn clone(&self) -> Self {
        Self {
            window: self.window.clone(),
            bit_checkboxes: self.bit_checkboxes.clone(),
            preview_text: self.preview_text.clone(),
            extract_data: RefCell::new(Vec::new()),
            extract_by_row: self.extract_by_row.clone(),
            extract_by_col: self.extract_by_col.clone(),
            msb_first: self.msb_first.clone(),
            lsb_first: self.lsb_first.clone(),
            rgb_order_buttons: self.rgb_order_buttons.clone(),
            hex_dump_enabled: self.hex_dump_enabled.clone(),
        }
    }
}