use image::RgbaImage;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// APNG信息结构
pub struct ApngInfo {
    pub is_apng: bool,
    pub frame_count: u32,
    pub width: u32,
    pub height: u32,
}

/// 检查PNG文件是否为APNG格式，并获取帧数信息
pub fn check_apng<P: AsRef<Path>>(path: P) -> Result<ApngInfo, std::io::Error> {
    let mut file = File::open(path)?;
    let mut reader = BufReader::new(&mut file);
    
    // 检查PNG签名
    let mut signature = [0u8; 8];
    reader.read_exact(&mut signature)?;
    
    if signature != [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        return Ok(ApngInfo {
            is_apng: false,
            frame_count: 1,
            width: 0,
            height: 0,
        });
    }

    let mut is_apng = false;
    let mut frame_count = 1;
    let mut width = 0;
    let mut height = 0;
    
    // 扫描chunks
    loop {
        let mut length_bytes = [0u8; 4];
        if reader.read_exact(&mut length_bytes).is_err() {
            break;
        }
        let length = u32::from_be_bytes(length_bytes);
        
        let mut chunk_type = [0u8; 4];
        if reader.read_exact(&mut chunk_type).is_err() {
            break;
        }
        
        match &chunk_type {
            b"IHDR" => {
                // 读取图片尺寸
                let mut data = [0u8; 8];
                if reader.read_exact(&mut data).is_ok() {
                    width = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                    height = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
                }
                // 跳过剩余数据和CRC
                let remaining = length.saturating_sub(8) + 4;
                reader.seek(SeekFrom::Current(remaining as i64))?;
            }
            b"acTL" => {
                // 动画控制chunk
                is_apng = true;
                if length >= 8 {
                    let mut data = [0u8; 8];
                    if reader.read_exact(&mut data).is_ok() {
                        frame_count = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                    }
                }
                // 跳过剩余数据和CRC
                let remaining = length.saturating_sub(8) + 4;
                reader.seek(SeekFrom::Current(remaining as i64))?;
            }
            b"IEND" => {
                break;
            }
            _ => {
                // 跳过chunk数据和CRC
                reader.seek(SeekFrom::Current((length + 4) as i64))?;
            }
        }
    }
    
    Ok(ApngInfo {
        is_apng,
        frame_count,
        width,
        height,
    })
}

/// APNG解码器 - 使用png crate解码所有帧
pub struct ApngDecoder {
    frames: Vec<RgbaImage>,
}

impl ApngDecoder {
    /// 从文件路径解码APNG的所有帧
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let file = File::open(&path)?;
        let decoder = png::Decoder::new(file);
        let mut reader = decoder.read_info()?;
        
        let info = reader.info();
        let width = info.width;
        let height = info.height;
        let color_type = info.color_type;
        
        let mut frames = Vec::new();
        
        // 读取第一帧
        let mut buf = vec![0; reader.output_buffer_size()];
        let output_info = reader.next_frame(&mut buf)?;
        
        // 转换为RGBA格式
        let frame = match color_type {
            png::ColorType::Rgba => {
                RgbaImage::from_raw(width, height, buf[..output_info.buffer_size()].to_vec())
                    .ok_or("无法创建图像")?
            }
            png::ColorType::Rgb => {
                // RGB转RGBA
                let rgb_data = &buf[..output_info.buffer_size()];
                let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
                for chunk in rgb_data.chunks(3) {
                    rgba_data.extend_from_slice(chunk);
                    rgba_data.push(255); // Alpha通道
                }
                RgbaImage::from_raw(width, height, rgba_data)
                    .ok_or("无法创建图像")?
            }
            png::ColorType::Grayscale => {
                // 灰度转RGBA
                let gray_data = &buf[..output_info.buffer_size()];
                let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
                for &gray in gray_data {
                    rgba_data.extend_from_slice(&[gray, gray, gray, 255]);
                }
                RgbaImage::from_raw(width, height, rgba_data)
                    .ok_or("无法创建图像")?
            }
            png::ColorType::GrayscaleAlpha => {
                // 灰度+Alpha转RGBA
                let ga_data = &buf[..output_info.buffer_size()];
                let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
                for chunk in ga_data.chunks(2) {
                    let gray = chunk[0];
                    let alpha = chunk[1];
                    rgba_data.extend_from_slice(&[gray, gray, gray, alpha]);
                }
                RgbaImage::from_raw(width, height, rgba_data)
                    .ok_or("无法创建图像")?
            }
            _ => {
                return Err("不支持的颜色类型".into());
            }
        };
        
        frames.push(frame);
        
        // 尝试读取后续帧（如果是APNG）
        loop {
            buf.clear();
            buf.resize(reader.output_buffer_size(), 0);
            
            match reader.next_frame(&mut buf) {
                Ok(output_info) => {
                    let frame = match color_type {
                        png::ColorType::Rgba => {
                            RgbaImage::from_raw(width, height, buf[..output_info.buffer_size()].to_vec())
                                .ok_or("无法创建图像")?
                        }
                        png::ColorType::Rgb => {
                            let rgb_data = &buf[..output_info.buffer_size()];
                            let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
                            for chunk in rgb_data.chunks(3) {
                                rgba_data.extend_from_slice(chunk);
                                rgba_data.push(255);
                            }
                            RgbaImage::from_raw(width, height, rgba_data)
                                .ok_or("无法创建图像")?
                        }
                        _ => continue,
                    };
                    frames.push(frame);
                }
                Err(_) => break,
            }
        }
        
        Ok(Self { frames })
    }

    /// 获取所有帧
    pub fn into_frames(self) -> Vec<RgbaImage> {
        self.frames
    }
    
    /// 获取帧数
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }
}

