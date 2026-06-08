//! StegSolve-rs 核心算法层（与 UI 完全解耦，可独立单元测试）。
//!
//! 本 crate 把原先散落在各 egui 模块里的隐写分析算法抽出来，并修复了对照
//! 原版 Java StegSolve 发现的一系列正确性缺陷（详见各模块注释里的 `FIX:` 标记）。
//!
//! 模块：
//! - [`transform`] 位平面 / 通道 / 反色 / 随机映射 / 灰度 等单图变换
//! - [`extract`]   LSB/MSB 数据提取（修复了 LSB 多通道顺序反转的 bug）
//! - [`stereo`]    立体图（自身水平偏移 XOR）
//! - [`combine`]   双图合成（补齐了原本缺失的 SUB/MUL/Lightest/Darkest 等模式）
//! - [`rng`]       一个无依赖的可重现伪随机数发生器（供随机颜色映射使用）

pub mod transform;
pub mod extract;
pub mod stereo;
pub mod combine;
pub mod rng;

pub use transform::Transform;
pub use extract::{ExtractOptions, BitOrder, ExtractDirection, RgbOrder, ChannelMask};
pub use stereo::StereoTransform;
pub use combine::{CombineTransform, CombineMode};
