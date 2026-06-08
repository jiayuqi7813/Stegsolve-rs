//! LSB/MSB 数据提取。
//!
//! 对照原版 `Extract.java` 修复：
//! - **E1（高危）**：原 Rust 实现的 LSB-First 通道顺序数组写反了（标 "RGB" 实际按
//!   B,G,R 提取，红蓝对调）。本实现用"通道置换 + 统一字节基址"的方式重写，
//!   MSB / LSB 两条路径共用同一套顺序定义，从结构上杜绝该类错误。
//! - **E2**：预览 ASCII 列只把 0x20..=0x7E 视为可打印，其余显示 `.`，
//!   不再把 `\n \t \r` 等控制符原样输出而破坏 hexdump 对齐。
//!
//! 像素打包约定：`value = (R<<24)|(G<<16)|(B<<8)|A`，即
//! R 占 bit31..24、G 占 23..16、B 占 15..8、A 占 7..0。掩码与之保持一致。

use image::RgbaImage;

/// 颜色通道。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Red,
    Green,
    Blue,
    Alpha,
}

impl Channel {
    /// 该通道在打包 u32 中的字节基址（bit0 所在位）。
    fn base(self) -> u32 {
        match self {
            Channel::Red => 24,
            Channel::Green => 16,
            Channel::Blue => 8,
            Channel::Alpha => 0,
        }
    }
    fn index(self) -> usize {
        match self {
            Channel::Red => 0,
            Channel::Green => 1,
            Channel::Blue => 2,
            Channel::Alpha => 3,
        }
    }
}

/// 提取方向。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtractDirection {
    Row,
    Column,
}

/// 位顺序。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitOrder {
    MsbFirst,
    LsbFirst,
}

/// RGB 通道处理顺序。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RgbOrder {
    Rgb,
    Rbg,
    Grb,
    Gbr,
    Brg,
    Bgr,
}

impl RgbOrder {
    /// 返回 R/G/B 三个通道的处理次序（用 Channel 表示）。
    fn sequence(self) -> [Channel; 3] {
        use Channel::{Blue as B, Green as G, Red as R};
        match self {
            RgbOrder::Rgb => [R, G, B],
            RgbOrder::Rbg => [R, B, G],
            RgbOrder::Grb => [G, R, B],
            RgbOrder::Gbr => [G, B, R],
            RgbOrder::Brg => [B, R, G],
            RgbOrder::Bgr => [B, G, R],
        }
    }
}

/// 各通道的位选择掩码（每通道 8 位，索引 = 位号，0 = LSB，7 = MSB）。
#[derive(Clone, Debug, Default)]
pub struct ChannelMask {
    /// `[R, G, B, A]`，每个元素 `[bit0..bit7]`。
    bits: [[bool; 8]; 4],
}

impl ChannelMask {
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置某通道某位号（0..=7）是否选中。
    pub fn set(&mut self, ch: Channel, bit: u8, value: bool) {
        self.bits[ch.index()][bit as usize & 7] = value;
    }

    /// 链式设置（便于测试与初始化）。
    pub fn with(mut self, ch: Channel, bit: u8) -> Self {
        self.set(ch, bit, true);
        self
    }

    /// 整通道全选/全不选。
    pub fn set_all(&mut self, ch: Channel, value: bool) {
        for b in self.bits[ch.index()].iter_mut() {
            *b = value;
        }
    }

    /// 读取某通道某位号是否选中。
    pub fn get(&self, ch: Channel, bit: u8) -> bool {
        self.bits[ch.index()][bit as usize & 7]
    }

    /// 计算 32 位掩码与选中的位数。
    fn mask(&self) -> (u32, u32) {
        let mut mask = 0u32;
        let mut count = 0u32;
        for ch in [Channel::Red, Channel::Green, Channel::Blue, Channel::Alpha] {
            for n in 0..8u32 {
                if self.bits[ch.index()][n as usize] {
                    mask |= 1 << (ch.base() + n);
                    count += 1;
                }
            }
        }
        (mask, count)
    }
}

/// 一次数据提取的全部选项。
#[derive(Clone, Debug)]
pub struct ExtractOptions {
    pub channels: ChannelMask,
    pub direction: ExtractDirection,
    pub bit_order: BitOrder,
    pub rgb_order: RgbOrder,
    pub include_hex: bool,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            channels: ChannelMask::new(),
            direction: ExtractDirection::Row,
            bit_order: BitOrder::MsbFirst,
            rgb_order: RgbOrder::Rgb,
            include_hex: true,
        }
    }
}

/// 比特写入累加器（对应原版 extractBitPos / extractBytePos / addBit）。
struct BitWriter {
    bytes: Vec<u8>,
    bit_pos: u8,
    byte_pos: usize,
}

impl BitWriter {
    fn new(len: usize) -> Self {
        Self { bytes: vec![0u8; len], bit_pos: 128, byte_pos: 0 }
    }

    #[inline]
    fn add_bit(&mut self, one: bool) {
        if one {
            if let Some(b) = self.bytes.get_mut(self.byte_pos) {
                *b |= self.bit_pos;
            }
        }
        self.bit_pos >>= 1;
        if self.bit_pos == 0 {
            self.bit_pos = 128;
            self.byte_pos += 1;
            if self.byte_pos < self.bytes.len() {
                self.bytes[self.byte_pos] = 0;
            }
        }
    }
}

/// 某通道的"起始基位掩码"：MSB-first 从该通道最高位开始，LSB-first 从最低位开始。
#[inline]
fn channel_start_mask(ch: Channel, bit_order: BitOrder) -> u32 {
    match bit_order {
        BitOrder::MsbFirst => 1 << (ch.base() + 7),
        BitOrder::LsbFirst => 1 << ch.base(),
    }
}

/// 检查一个通道字节的 8 个位，按掩码决定哪些位写入输出。
#[inline]
fn extract_channel(value: u32, ch: Channel, opt_mask: u32, bit_order: BitOrder, w: &mut BitWriter) {
    let mut cur = channel_start_mask(ch, bit_order);
    for _ in 0..8 {
        if opt_mask & cur != 0 {
            w.add_bit(value & cur != 0);
        }
        cur = match bit_order {
            BitOrder::MsbFirst => cur >> 1,
            BitOrder::LsbFirst => cur << 1,
        };
    }
}

/// 从单个像素值中提取选中的位（顺序：Alpha 先，再按 rgb_order 处理 R/G/B）。
#[inline]
fn extract_pixel(value: u32, opt: &ExtractOptions, opt_mask: u32, w: &mut BitWriter) {
    // 与原版一致：Alpha 通道总是最先处理
    extract_channel(value, Channel::Alpha, opt_mask, opt.bit_order, w);
    for ch in opt.rgb_order.sequence() {
        extract_channel(value, ch, opt_mask, opt.bit_order, w);
    }
}

/// 根据选项从图像提取数据。
pub fn generate_extract(image: &RgbaImage, opt: &ExtractOptions) -> Vec<u8> {
    let (opt_mask, maskbits) = opt.channels.mask();
    if maskbits == 0 {
        return Vec::new();
    }
    let total_bits = image.width() as u64 * image.height() as u64 * maskbits as u64;
    let len = ((total_bits + 7) / 8) as usize;
    let mut w = BitWriter::new(len);

    let (width, height) = (image.width(), image.height());
    let handle = |x: u32, y: u32, w: &mut BitWriter| {
        let p = image.get_pixel(x, y);
        let value = ((p[0] as u32) << 24) | ((p[1] as u32) << 16) | ((p[2] as u32) << 8) | p[3] as u32;
        extract_pixel(value, opt, opt_mask, w);
    };

    match opt.direction {
        ExtractDirection::Row => {
            for y in 0..height {
                for x in 0..width {
                    handle(x, y, &mut w);
                }
            }
        }
        ExtractDirection::Column => {
            for x in 0..width {
                for y in 0..height {
                    handle(x, y, &mut w);
                }
            }
        }
    }

    w.bytes
}

/// 生成预览文本（16 字节/行，可选 hexdump 列）。
pub fn generate_preview(data: &[u8], include_hex: bool) -> String {
    let mut out = String::new();
    for chunk_start in (0..data.len()).step_by(16) {
        if include_hex {
            for j in 0..16 {
                if chunk_start + j < data.len() {
                    out.push_str(&format!("{:02x}", data[chunk_start + j]));
                    if j == 7 {
                        out.push(' ');
                    }
                }
            }
            out.push_str("  ");
        }
        for j in 0..16 {
            if chunk_start + j < data.len() {
                let byte = data[chunk_start + j];
                // FIX E2：仅可打印 ASCII 原样输出，其余（含控制符与高位字节）显示 '.'
                if (0x20..=0x7e).contains(&byte) {
                    out.push(byte as char);
                } else {
                    out.push('.');
                }
                if j == 7 {
                    out.push(' ');
                }
            }
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    /// 用单像素验证 LSB-First 多通道顺序（E1 回归测试）。
    #[test]
    fn lsb_first_multichannel_order_is_r_then_b_not_reversed() {
        // 像素：R=0x01(bit0=1), G=0, B=0x00(bit0=0), A=0
        let mut img = RgbaImage::new(1, 1);
        img.put_pixel(0, 0, Rgba([0x01, 0x00, 0x00, 0x00]));

        // 选中 R.bit0 与 B.bit0，LSB-First，RGB 顺序
        let channels = ChannelMask::new()
            .with(Channel::Red, 0)
            .with(Channel::Blue, 0);
        let opt = ExtractOptions {
            channels,
            direction: ExtractDirection::Row,
            bit_order: BitOrder::LsbFirst,
            rgb_order: RgbOrder::Rgb,
            include_hex: true,
        };
        let data = generate_extract(&img, &opt);
        // 顺序应为 R(=1) 然后 B(=0)：高位先写 -> 0b1000_0000 = 0x80
        // 若像修复前那样反成 B,R，则会得到 0x40
        assert_eq!(data[0], 0x80, "LSB-First RGB 必须按 R,G,B 顺序（红蓝不能对调）");
    }

    /// 对照：BGR 顺序下同样的输入应得到 0x40（R 和 B 互换）。
    #[test]
    fn lsb_first_bgr_order() {
        let mut img = RgbaImage::new(1, 1);
        img.put_pixel(0, 0, Rgba([0x01, 0x00, 0x00, 0x00]));
        let channels = ChannelMask::new()
            .with(Channel::Red, 0)
            .with(Channel::Blue, 0);
        let opt = ExtractOptions {
            channels,
            bit_order: BitOrder::LsbFirst,
            rgb_order: RgbOrder::Bgr,
            ..Default::default()
        };
        let data = generate_extract(&img, &opt);
        assert_eq!(data[0], 0x40, "BGR 顺序下 B(=0) 先、R(=1) 后 -> 0x40");
    }

    /// 经典用法：仅取 Blue LSB，MSB-First，按行，重建一个已知字节 'A'。
    #[test]
    fn reconstruct_byte_from_blue_lsb_msb_first() {
        // 'A' = 0x41 = 0b0100_0001，MSB-first 比特序：0,1,0,0,0,0,0,1
        let bits = [0u8, 1, 0, 0, 0, 0, 0, 1];
        let mut img = RgbaImage::new(8, 1);
        for (i, &bit) in bits.iter().enumerate() {
            img.put_pixel(i as u32, 0, Rgba([0, 0, bit, 255]));
        }
        let opt = ExtractOptions {
            channels: ChannelMask::new().with(Channel::Blue, 0),
            bit_order: BitOrder::MsbFirst,
            ..Default::default()
        };
        let data = generate_extract(&img, &opt);
        assert_eq!(data, vec![0x41]);
        assert_eq!(data[0] as char, 'A');
    }

    /// 单通道时 LSB/MSB 的通道顺序 bug 不应影响结果（自洽性）。
    #[test]
    fn single_channel_unaffected_by_order() {
        let mut img = RgbaImage::new(8, 1);
        let bits = [1u8, 0, 1, 1, 0, 0, 1, 0];
        for (i, &bit) in bits.iter().enumerate() {
            img.put_pixel(i as u32, 0, Rgba([0, 0, bit, 255]));
        }
        let base = ExtractOptions {
            channels: ChannelMask::new().with(Channel::Blue, 0),
            bit_order: BitOrder::MsbFirst,
            ..Default::default()
        };
        let rgb = generate_extract(&img, &base);
        let bgr = generate_extract(&img, &ExtractOptions { rgb_order: RgbOrder::Bgr, ..base.clone() });
        assert_eq!(rgb, bgr, "单通道下 RGB/BGR 顺序结果应一致");
    }

    #[test]
    fn empty_mask_returns_empty() {
        let img = RgbaImage::new(4, 4);
        let opt = ExtractOptions::default();
        assert!(generate_extract(&img, &opt).is_empty());
    }

    #[test]
    fn mask_bit_count_and_length() {
        // 选 3 个位，4 像素 -> 12 bits -> 2 bytes
        let img = RgbaImage::new(2, 2);
        let channels = ChannelMask::new()
            .with(Channel::Red, 0)
            .with(Channel::Green, 0)
            .with(Channel::Blue, 0);
        let opt = ExtractOptions { channels, ..Default::default() };
        let data = generate_extract(&img, &opt);
        assert_eq!(data.len(), 2);
    }

    #[test]
    fn preview_replaces_control_chars_with_dot() {
        // 含换行/制表符的数据，预览里应显示 '.' 而非原样控制符
        let data = b"\nA\tB\r";
        let s = generate_preview(data, false);
        assert!(!s.contains('\t'), "制表符不应原样出现");
        // 第一行内容部分应包含 'A' 'B' 以及表示控制符的 '.'
        assert!(s.contains('A') && s.contains('B'));
        assert!(s.contains('.'));
    }

    #[test]
    fn preview_hex_dump_format() {
        let data = [0x41u8, 0x42, 0x43];
        let s = generate_preview(&data, true);
        assert!(s.contains("414243"));
        assert!(s.contains("ABC"));
    }

    #[test]
    fn column_vs_row_direction_differs() {
        // 2x2，不同方向应产生不同的 bit 序
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(0, 0, Rgba([0, 0, 1, 255]));
        img.put_pixel(1, 0, Rgba([0, 0, 0, 255]));
        img.put_pixel(0, 1, Rgba([0, 0, 0, 255]));
        img.put_pixel(1, 1, Rgba([0, 0, 1, 255]));
        let opt_row = ExtractOptions {
            channels: ChannelMask::new().with(Channel::Blue, 0),
            direction: ExtractDirection::Row,
            ..Default::default()
        };
        let opt_col = ExtractOptions { direction: ExtractDirection::Column, ..opt_row.clone() };
        // row: (0,0)(1,0)(0,1)(1,1) -> 1,0,0,1 ; col: (0,0)(0,1)(1,0)(1,1) -> 1,0,0,1
        // 这里两者恰好相同，所以改用非对称图案验证差异
        let _ = (generate_extract(&img, &opt_row), generate_extract(&img, &opt_col));

        let mut img2 = RgbaImage::new(2, 2);
        img2.put_pixel(0, 0, Rgba([0, 0, 1, 255]));
        img2.put_pixel(1, 0, Rgba([0, 0, 1, 255]));
        img2.put_pixel(0, 1, Rgba([0, 0, 0, 255]));
        img2.put_pixel(1, 1, Rgba([0, 0, 0, 255]));
        let row = generate_extract(&img2, &opt_row); // 1,1,0,0 -> 0xC0
        let col = generate_extract(&img2, &opt_col); // 1,0,1,0 -> 0xA0
        assert_ne!(row, col);
        assert_eq!(row[0], 0xC0);
        assert_eq!(col[0], 0xA0);
    }
}
