//! 端到端集成测试：把一段已知文本以 LSB 隐写嵌入图像，再用提取器还原，
//! 验证修复后的提取在真实数据上确实可用（尤其是多通道 LSB 顺序）。

use image::{Rgba, RgbaImage};
use stegsolve_core::extract::{
    generate_extract, BitOrder, Channel, ChannelMask, ExtractDirection, ExtractOptions, RgbOrder,
};

/// 把 message 的比特（每字节 MSB 优先）逐位写进指定通道的 bit0。
fn embed_lsb_single_channel(img: &mut RgbaImage, channel_idx: usize, message: &[u8]) {
    let bits: Vec<u8> = message
        .iter()
        .flat_map(|b| (0..8).rev().map(move |i| (b >> i) & 1))
        .collect();
    let (w, _h) = (img.width(), img.height());
    for (k, &bit) in bits.iter().enumerate() {
        let x = (k as u32) % w;
        let y = (k as u32) / w;
        let p = img.get_pixel_mut(x, y);
        p[channel_idx] = (p[channel_idx] & 0xFE) | bit;
    }
}

#[test]
fn hide_and_recover_flag_blue_lsb() {
    let message = b"flag{lsb_recovered_ok}";
    let mut img = RgbaImage::from_pixel(64, 8, Rgba([120, 90, 200, 255]));
    embed_lsb_single_channel(&mut img, 2 /* blue */, message);

    let opt = ExtractOptions {
        channels: ChannelMask::new().with(Channel::Blue, 0),
        direction: ExtractDirection::Row,
        bit_order: BitOrder::MsbFirst,
        rgb_order: RgbOrder::Rgb,
        include_hex: false,
    };
    let data = generate_extract(&img, &opt);
    assert_eq!(&data[..message.len()], message);
}

/// 多通道 LSB-First 往返：把消息按 R,G,B 三通道交替嵌入 bit0，
/// 再用 LSB-First + RGB 提取还原。修复前（红蓝顺序反）这里必然失败。
#[test]
fn hide_and_recover_multichannel_lsb_first() {
    // 嵌入顺序需与提取顺序一致：提取在 LSB-First 下对每个像素按 R(bit0),G(bit0),B(bit0)
    // 的顺序各取 1 位。这里按同样顺序把消息比特写进 R/G/B 的 bit0。
    let message = b"RGB-order!";
    let bits: Vec<u8> = message
        .iter()
        .flat_map(|b| (0..8).rev().map(move |i| (b >> i) & 1))
        .collect();

    let w = 64u32;
    let h = 8u32;
    let mut img = RgbaImage::from_pixel(w, h, Rgba([10, 20, 30, 255]));

    // 每个像素承载 3 个比特（R,G,B 各一）
    for (k, &bit) in bits.iter().enumerate() {
        let pixel_index = (k / 3) as u32;
        let channel = k % 3; // 0=R,1=G,2=B
        let x = pixel_index % w;
        let y = pixel_index / w;
        let p = img.get_pixel_mut(x, y);
        p[channel] = (p[channel] & 0xFE) | bit;
    }

    let opt = ExtractOptions {
        channels: ChannelMask::new()
            .with(Channel::Red, 0)
            .with(Channel::Green, 0)
            .with(Channel::Blue, 0),
        direction: ExtractDirection::Row,
        bit_order: BitOrder::LsbFirst,
        rgb_order: RgbOrder::Rgb,
        include_hex: false,
    };
    let data = generate_extract(&img, &opt);
    assert_eq!(&data[..message.len()], message, "LSB-First 多通道往返必须一致");
}
