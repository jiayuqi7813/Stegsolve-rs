//! 帧加载（纯逻辑，无 UI）：支持 GIF / APNG / WebP / 静态图片，返回帧序列。
//! 由原 `framebrowser.rs` 的加载部分重构而来。

use crate::apng_decoder::{check_apng, ApngDecoder};
use image::codecs::gif::GifDecoder;
use image::AnimationDecoder;
use image::{ImageError, ImageFormat, RgbaImage};
use std::io::BufReader;
use std::path::Path;

/// 从路径加载所有帧。静态图片返回单帧。
pub fn load_frames<P: AsRef<Path>>(path: P) -> Result<Vec<RgbaImage>, ImageError> {
    let path = path.as_ref();
    let mut frames: Vec<RgbaImage> = Vec::new();

    // 优先检测 APNG（避免某些解码器卡死）
    if let Ok(info) = check_apng(path) {
        if info.is_apng {
            match ApngDecoder::from_path(path) {
                Ok(decoder) => {
                    frames.extend(decoder.into_frames());
                    if !frames.is_empty() {
                        return Ok(frames);
                    }
                }
                Err(e) => {
                    eprintln!("APNG 解码失败: {}, 回退为静态 PNG", e);
                    frames.push(image::open(path)?.to_rgba8());
                    return Ok(frames);
                }
            }
        }
    }

    // 常规格式
    let file = std::fs::File::open(path)?;
    let reader = image::ImageReader::new(BufReader::new(file)).with_guessed_format()?;

    match reader.format() {
        Some(ImageFormat::Gif) => {
            let file = std::fs::File::open(path)?;
            let decoder = GifDecoder::new(BufReader::new(file))?;
            for frame in decoder.into_frames().collect_frames()? {
                frames.push(frame.into_buffer());
            }
        }
        _ => {
            frames.push(reader.decode()?.to_rgba8());
        }
    }

    if frames.is_empty() {
        frames.push(image::open(path)?.to_rgba8());
    }
    Ok(frames)
}
