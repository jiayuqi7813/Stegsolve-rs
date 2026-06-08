//! MCP 服务：把 StegSolve-rs 的隐写分析能力暴露给大模型。
//!
//! - 传输：Streamable HTTP（在 GUI 进程内托管，按钮启停，给客户端一个 URL）。
//! - 工具逻辑全部转调已单测的 `stegsolve-core` / 现有 `fileanalysis`、`frames`。
//! - 设计上把"真正干活的逻辑"放在 `op_*` 纯函数里（可 headless 单测），
//!   `#[tool]` 方法只是薄封装；这样即使不连真实 MCP 客户端也能测。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use base64::Engine;
use image::RgbaImage;

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router, ErrorData, ServerHandler,
};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use tokio_util::sync::CancellationToken;

use stegsolve_core::combine::{combine, CombineMode};
use stegsolve_core::extract::{
    generate_extract, generate_preview, BitOrder, Channel, ChannelMask, ExtractDirection,
    ExtractOptions, RgbOrder,
};
use stegsolve_core::stereo::StereoTransform;
use stegsolve_core::transform::{Transform, MAX_TRANS};

/// GUI 与服务共享的请求日志。
pub type SharedLog = Arc<Mutex<Vec<String>>>;

// ───────────────────────── 纯逻辑 helper（可单测）─────────────────────────

fn load_rgba(path: &str) -> Result<RgbaImage, String> {
    image::open(path)
        .map(|i| i.to_rgba8())
        .map_err(|e| format!("打开图片失败 {path}: {e}"))
}

/// 超过 `max` 边长则按最近邻缩小（位平面用最近邻可保边缘）。
fn maybe_downscale(img: &RgbaImage, max: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    if w.max(h) <= max || w == 0 || h == 0 {
        return img.clone();
    }
    let scale = max as f32 / w.max(h) as f32;
    let nw = ((w as f32 * scale).round() as u32).max(1);
    let nh = ((h as f32 * scale).round() as u32).max(1);
    image::imageops::resize(img, nw, nh, image::imageops::FilterType::Nearest)
}

fn png_bytes(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgba8(img.clone())
        .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .map_err(|e| format!("PNG 编码失败: {e}"))?;
    Ok(buf)
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// 文件格式分析报告。
pub fn op_analyze_format(path: &str) -> String {
    crate::fileanalysis::analyse_file_format(path).join("\n")
}

/// 列出 0..=41 号变换名称。
pub fn op_list_transforms() -> String {
    let dummy = RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 255]));
    let mut t = Transform::from_rgba(dummy);
    let mut out = String::from("可用变换（render_transform 的 transform 取值）:\n");
    for n in 0..=MAX_TRANS {
        t.set_trans(n);
        out.push_str(&format!("{:>2}: {}\n", n, t.get_text()));
    }
    out
}

/// 渲染指定变换，返回 (PNG 字节, 宽, 高, 名称)。
pub fn op_render_transform(path: &str, trans: i32) -> Result<(Vec<u8>, u32, u32, String), String> {
    let img = load_rgba(path)?;
    let mut t = Transform::from_rgba(img);
    t.set_trans(trans);
    let out = maybe_downscale(t.get_image(), 2048);
    let (w, h) = out.dimensions();
    Ok((png_bytes(&out)?, w, h, t.get_text()))
}

/// 渲染某通道某位平面。channel: r/g/b/a，plane: 0(LSB)..7(MSB)。
pub fn op_bit_plane(path: &str, channel: &str, plane: u8) -> Result<(Vec<u8>, u32, u32, String), String> {
    let plane = plane.min(7);
    let base: i32 = match channel.to_ascii_lowercase().as_str() {
        "a" | "alpha" => 2,
        "r" | "red" => 10,
        "g" | "green" => 18,
        "b" | "blue" => 26,
        other => return Err(format!("未知通道 {other}（用 r/g/b/a）")),
    };
    let trans = base + (7 - plane as i32);
    op_render_transform(path, trans)
}

fn parse_bit_order(s: &str) -> BitOrder {
    if s.to_ascii_lowercase().contains("lsb") {
        BitOrder::LsbFirst
    } else {
        BitOrder::MsbFirst
    }
}
fn parse_direction(s: &str) -> ExtractDirection {
    if s.to_ascii_lowercase().starts_with('c') {
        ExtractDirection::Column
    } else {
        ExtractDirection::Row
    }
}
fn parse_rgb_order(s: &str) -> RgbOrder {
    match s.to_ascii_uppercase().as_str() {
        "RBG" => RgbOrder::Rbg,
        "GRB" => RgbOrder::Grb,
        "GBR" => RgbOrder::Gbr,
        "BRG" => RgbOrder::Brg,
        "BGR" => RgbOrder::Bgr,
        _ => RgbOrder::Rgb,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn op_extract(
    path: &str,
    red: &[u8],
    green: &[u8],
    blue: &[u8],
    alpha: &[u8],
    bit_order: &str,
    direction: &str,
    rgb_order: &str,
    max_preview: Option<usize>,
) -> Result<(String, Vec<u8>), String> {
    let img = load_rgba(path)?;
    let mut ch = ChannelMask::new();
    for &b in red {
        ch.set(Channel::Red, b, true);
    }
    for &b in green {
        ch.set(Channel::Green, b, true);
    }
    for &b in blue {
        ch.set(Channel::Blue, b, true);
    }
    for &b in alpha {
        ch.set(Channel::Alpha, b, true);
    }
    let opt = ExtractOptions {
        channels: ch,
        direction: parse_direction(direction),
        bit_order: parse_bit_order(bit_order),
        rgb_order: parse_rgb_order(rgb_order),
        include_hex: true,
    };
    let data = generate_extract(&img, &opt);
    let cap = max_preview.unwrap_or(4096).min(data.len());
    let preview = generate_preview(&data[..cap], true);
    Ok((preview, data))
}

/// 提取所有 ≥ min_len 的可打印 ASCII 串。
fn printable_runs(data: &[u8], min_len: usize) -> Vec<String> {
    let mut runs = Vec::new();
    let mut cur = String::new();
    for &byte in data {
        if (0x20..=0x7e).contains(&byte) {
            cur.push(byte as char);
        } else {
            if cur.len() >= min_len {
                runs.push(std::mem::take(&mut cur));
            }
            cur.clear();
        }
    }
    if cur.len() >= min_len {
        runs.push(cur);
    }
    runs
}

fn looks_like_flag(s: &str) -> bool {
    let l = s.to_ascii_lowercase();
    l.contains("flag") || l.contains("ctf") || s.contains('{') || l.contains("key")
}

/// 自动尝试一组常见 LSB 配置，汇总可疑可打印串 / flag 候选。
pub fn op_auto_lsb(path: &str, min_len: usize) -> Result<String, String> {
    let img = load_rgba(path)?;
    let min_len = min_len.max(4);

    // (标签, 通道掩码, 位顺序, 方向)
    let mk = |r: &[u8], g: &[u8], b: &[u8], a: &[u8]| {
        let mut m = ChannelMask::new();
        for &x in r {
            m.set(Channel::Red, x, true);
        }
        for &x in g {
            m.set(Channel::Green, x, true);
        }
        for &x in b {
            m.set(Channel::Blue, x, true);
        }
        for &x in a {
            m.set(Channel::Alpha, x, true);
        }
        m
    };

    let configs: Vec<(String, ChannelMask, BitOrder, ExtractDirection)> = vec![
        ("RGB bit0, MSB, 行".into(), mk(&[0], &[0], &[0], &[]), BitOrder::MsbFirst, ExtractDirection::Row),
        ("RGB bit0, LSB, 行".into(), mk(&[0], &[0], &[0], &[]), BitOrder::LsbFirst, ExtractDirection::Row),
        ("RGB bit0, MSB, 列".into(), mk(&[0], &[0], &[0], &[]), BitOrder::MsbFirst, ExtractDirection::Column),
        ("Red bit0, MSB, 行".into(), mk(&[0], &[], &[], &[]), BitOrder::MsbFirst, ExtractDirection::Row),
        ("Green bit0, MSB, 行".into(), mk(&[], &[0], &[], &[]), BitOrder::MsbFirst, ExtractDirection::Row),
        ("Blue bit0, MSB, 行".into(), mk(&[], &[], &[0], &[]), BitOrder::MsbFirst, ExtractDirection::Row),
        ("Alpha bit0, MSB, 行".into(), mk(&[], &[], &[], &[0]), BitOrder::MsbFirst, ExtractDirection::Row),
    ];

    let mut report = String::from("auto_lsb 结果（每个配置列出可疑可打印串）:\n");
    let mut any_flag = false;
    for (label, channels, bit_order, direction) in configs {
        let opt = ExtractOptions { channels, direction, bit_order, rgb_order: RgbOrder::Rgb, include_hex: false };
        let data = generate_extract(&img, &opt);
        let mut runs = printable_runs(&data, min_len);
        // flag 候选优先、其次最长，最多 5 条
        runs.sort_by(|a, b| (looks_like_flag(b), b.len()).cmp(&(looks_like_flag(a), a.len())));
        runs.truncate(5);
        report.push_str(&format!("\n[{label}]\n"));
        if runs.is_empty() {
            report.push_str("  (无)\n");
        } else {
            for r in &runs {
                let mark = if looks_like_flag(r) {
                    any_flag = true;
                    "★ "
                } else {
                    "  "
                };
                let shown: String = r.chars().take(120).collect();
                report.push_str(&format!("{mark}{shown}\n"));
            }
        }
    }
    if any_flag {
        report.push_str("\n（★ 标记的串疑似 flag/key，建议优先关注）\n");
    }
    Ok(report)
}

pub fn op_combine(path1: &str, path2: &str, mode: i32) -> Result<(Vec<u8>, u32, u32, String), String> {
    let a = load_rgba(path1)?;
    let b = load_rgba(path2)?;
    let m = CombineMode::from_index(mode);
    let out = maybe_downscale(&combine(&a, &b, m), 2048);
    let (w, h) = out.dimensions();
    Ok((png_bytes(&out)?, w, h, m.text().to_string()))
}

pub fn op_stereo(path: &str, offset: i32) -> Result<(Vec<u8>, u32, u32), String> {
    let img = load_rgba(path)?;
    let mut s = StereoTransform::new(img);
    s.set_offset(offset);
    let out = maybe_downscale(s.get_image(), 2048);
    let (w, h) = out.dimensions();
    Ok((png_bytes(&out)?, w, h))
}

pub fn op_frame_count(path: &str) -> Result<usize, String> {
    crate::frames::load_frames(path)
        .map(|f| f.len())
        .map_err(|e| format!("加载帧失败: {e}"))
}

pub fn op_extract_frame(path: &str, index: usize) -> Result<(Vec<u8>, u32, u32, usize), String> {
    let frames = crate::frames::load_frames(path).map_err(|e| format!("加载帧失败: {e}"))?;
    let total = frames.len();
    let f = frames.get(index).ok_or_else(|| format!("帧下标越界: {index}（共 {total} 帧）"))?;
    let out = maybe_downscale(f, 2048);
    let (w, h) = out.dimensions();
    Ok((png_bytes(&out)?, w, h, total))
}

// ───────────────────────── MCP 工具参数 ─────────────────────────

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct PathArg {
    /// 图片文件的绝对路径
    pub path: String,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct RenderArg {
    /// 图片绝对路径
    pub path: String,
    /// 变换编号 0..=41（见 list_transforms）
    pub transform: i32,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct BitPlaneArg {
    /// 图片绝对路径
    pub path: String,
    /// 通道：r/g/b/a
    pub channel: String,
    /// 位平面：0(最低位)..7(最高位)
    pub plane: u8,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct ExtractArg {
    /// 图片绝对路径
    pub path: String,
    /// 红通道选中的位号列表（0..7，0=LSB）
    #[serde(default)]
    pub red_bits: Vec<u8>,
    #[serde(default)]
    pub green_bits: Vec<u8>,
    #[serde(default)]
    pub blue_bits: Vec<u8>,
    #[serde(default)]
    pub alpha_bits: Vec<u8>,
    /// "msb" 或 "lsb"
    #[serde(default)]
    pub bit_order: Option<String>,
    /// "row" 或 "column"
    #[serde(default)]
    pub direction: Option<String>,
    /// 通道顺序 rgb/rbg/grb/gbr/brg/bgr
    #[serde(default)]
    pub rgb_order: Option<String>,
    /// 预览最多字节数（默认 4096）
    #[serde(default)]
    pub max_preview: Option<usize>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct AutoArg {
    /// 图片绝对路径
    pub path: String,
    /// 可打印串的最小长度（默认 6）
    #[serde(default)]
    pub min_len: Option<usize>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct CombineArg {
    /// 第一张图绝对路径
    pub path1: String,
    /// 第二张图绝对路径
    pub path2: String,
    /// 合成模式 0..=12（0 XOR,1 OR,2 AND,3 ADD,4 ADD分通道,5 SUB,6 SUB分通道,7 MUL,8 MUL分通道,9 Lightest,10 Darkest,11 横交错,12 竖交错）
    pub mode: i32,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct StereoArg {
    /// 图片绝对路径
    pub path: String,
    /// 水平偏移量
    pub offset: i32,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct FrameArg {
    /// 图片（GIF/APNG/WebP）绝对路径
    pub path: String,
    /// 帧下标（从 0 起）
    pub index: usize,
}

// ───────────────────────── MCP 服务实现 ─────────────────────────

#[derive(Clone)]
pub struct StegMcp {
    tool_router: ToolRouter<Self>,
    log: SharedLog,
    requests: Arc<AtomicUsize>,
}

impl StegMcp {
    pub fn new(log: SharedLog, requests: Arc<AtomicUsize>) -> Self {
        Self { tool_router: Self::tool_router(), log, requests }
    }

    fn note(&self, msg: impl Into<String>) {
        self.requests.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut l) = self.log.lock() {
            l.push(msg.into());
            let n = l.len();
            if n > 300 {
                l.drain(0..n - 300);
            }
        }
    }

    fn err(e: String) -> ErrorData {
        ErrorData::internal_error(e, None)
    }

    fn image_result(caption: String, png: Vec<u8>) -> CallToolResult {
        CallToolResult::success(vec![Content::text(caption), Content::image(b64(&png), "image/png")])
    }
}

#[tool_router]
impl StegMcp {
    #[tool(description = "分析图片文件格式（PNG/BMP/GIF/JPEG/APNG 等），返回结构化报告（含 chunk、尺寸、CRC 等）")]
    fn analyze_format(&self, Parameters(PathArg { path }): Parameters<PathArg>) -> Result<CallToolResult, ErrorData> {
        self.note(format!("analyze_format({path})"));
        Ok(CallToolResult::success(vec![Content::text(op_analyze_format(&path))]))
    }

    #[tool(description = "列出全部 0..=41 号图像变换的名称，供 render_transform 选择")]
    fn list_transforms(&self) -> Result<CallToolResult, ErrorData> {
        self.note("list_transforms()");
        Ok(CallToolResult::success(vec![Content::text(op_list_transforms())]))
    }

    #[tool(description = "渲染指定编号(0..=41)的变换并以 PNG 图片返回，便于模型直接观察位平面/通道等")]
    fn render_transform(&self, Parameters(RenderArg { path, transform }): Parameters<RenderArg>) -> Result<CallToolResult, ErrorData> {
        self.note(format!("render_transform({path}, {transform})"));
        let (png, w, h, name) = op_render_transform(&path, transform).map_err(Self::err)?;
        Ok(Self::image_result(format!("变换 {transform}: {name}（{w}x{h}）"), png))
    }

    #[tool(description = "渲染某通道某位平面为 PNG。channel=r/g/b/a，plane=0(LSB)..7(MSB)")]
    fn bit_plane(&self, Parameters(BitPlaneArg { path, channel, plane }): Parameters<BitPlaneArg>) -> Result<CallToolResult, ErrorData> {
        self.note(format!("bit_plane({path}, {channel}, {plane})"));
        let (png, w, h, name) = op_bit_plane(&path, &channel, plane).map_err(Self::err)?;
        Ok(Self::image_result(format!("{name}（{w}x{h}）"), png))
    }

    #[tool(description = "按指定位平面/通道/位序/方向提取数据，返回 hex+ascii 预览与原始字节(base64)")]
    fn extract_data(&self, Parameters(arg): Parameters<ExtractArg>) -> Result<CallToolResult, ErrorData> {
        self.note(format!("extract_data({})", arg.path));
        let (preview, data) = op_extract(
            &arg.path,
            &arg.red_bits,
            &arg.green_bits,
            &arg.blue_bits,
            &arg.alpha_bits,
            arg.bit_order.as_deref().unwrap_or("msb"),
            arg.direction.as_deref().unwrap_or("row"),
            arg.rgb_order.as_deref().unwrap_or("rgb"),
            arg.max_preview,
        )
        .map_err(Self::err)?;
        Ok(CallToolResult::success(vec![
            Content::text(format!("共 {} 字节。预览:\n{}", data.len(), preview)),
            Content::text(format!("data_base64: {}", b64(&data))),
        ]))
    }

    #[tool(description = "自动尝试一组常见 LSB 提取配置，汇总可疑可打印串/flag 候选（CTF 自动解题首选）")]
    fn auto_lsb(&self, Parameters(AutoArg { path, min_len }): Parameters<AutoArg>) -> Result<CallToolResult, ErrorData> {
        self.note(format!("auto_lsb({path})"));
        let report = op_auto_lsb(&path, min_len.unwrap_or(6)).map_err(Self::err)?;
        Ok(CallToolResult::success(vec![Content::text(report)]))
    }

    #[tool(description = "把两张图按模式(0..=12)合成并以 PNG 返回")]
    fn combine_images(&self, Parameters(CombineArg { path1, path2, mode }): Parameters<CombineArg>) -> Result<CallToolResult, ErrorData> {
        self.note(format!("combine_images({path1}, {path2}, {mode})"));
        let (png, w, h, name) = op_combine(&path1, &path2, mode).map_err(Self::err)?;
        Ok(Self::image_result(format!("合成 [{name}]（{w}x{h}）"), png))
    }

    #[tool(description = "立体图：图像与自身水平偏移版本逐像素 XOR，返回 PNG")]
    fn stereo(&self, Parameters(StereoArg { path, offset }): Parameters<StereoArg>) -> Result<CallToolResult, ErrorData> {
        self.note(format!("stereo({path}, {offset})"));
        let (png, w, h) = op_stereo(&path, offset).map_err(Self::err)?;
        Ok(Self::image_result(format!("立体图 偏移={offset}（{w}x{h}）"), png))
    }

    #[tool(description = "返回图片（GIF/APNG/WebP）的帧数")]
    fn frame_count(&self, Parameters(PathArg { path }): Parameters<PathArg>) -> Result<CallToolResult, ErrorData> {
        self.note(format!("frame_count({path})"));
        let n = op_frame_count(&path).map_err(Self::err)?;
        Ok(CallToolResult::success(vec![Content::text(format!("帧数: {n}"))]))
    }

    #[tool(description = "提取第 index 帧并以 PNG 返回")]
    fn extract_frame(&self, Parameters(FrameArg { path, index }): Parameters<FrameArg>) -> Result<CallToolResult, ErrorData> {
        self.note(format!("extract_frame({path}, {index})"));
        let (png, w, h, total) = op_extract_frame(&path, index).map_err(Self::err)?;
        Ok(Self::image_result(format!("帧 {}/{}（{w}x{h}）", index + 1, total), png))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for StegMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }
}

// ───────────────────────── HTTP 服务启停 ─────────────────────────

/// 服务句柄：持有取消令牌与"绑定结果"通道。
pub struct McpHandle {
    ct: CancellationToken,
    pub bind_rx: std::sync::mpsc::Receiver<Result<String, String>>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl McpHandle {
    /// 请求停止（优雅关闭）。
    pub fn stop(&mut self) {
        self.ct.cancel();
    }
}

impl Drop for McpHandle {
    fn drop(&mut self) {
        self.ct.cancel();
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

/// 在后台线程内起一个 tokio 运行时 + Streamable-HTTP MCP 服务。
/// 通过 `bind_rx` 回传绑定结果（Ok(addr) / Err(msg)）。
pub fn spawn_http_server(
    port: u16,
    token: Option<String>,
    log: SharedLog,
    requests: Arc<AtomicUsize>,
) -> McpHandle {
    let ct = CancellationToken::new();
    let (tx, rx) = std::sync::mpsc::channel();
    let ct_thread = ct.clone();

    let join = std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                let _ = tx.send(Err(format!("tokio 运行时启动失败: {e}")));
                return;
            }
        };
        rt.block_on(async move {
            let cfg = StreamableHttpServerConfig::default()
                .with_cancellation_token(ct_thread.child_token());
            let log2 = log.clone();
            let req2 = requests.clone();
            let service = StreamableHttpService::new(
                move || Ok(StegMcp::new(log2.clone(), req2.clone())),
                Arc::new(LocalSessionManager::default()),
                cfg,
            );
            let mut app = axum::Router::new().nest_service("/mcp", service);
            if let Some(tok) = token {
                app = app.layer(axum::middleware::from_fn_with_state(tok, auth_middleware));
            }
            let listener = match tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
                Ok(l) => l,
                Err(e) => {
                    let _ = tx.send(Err(format!("绑定 127.0.0.1:{port} 失败: {e}")));
                    return;
                }
            };
            let addr = listener
                .local_addr()
                .map(|a| a.to_string())
                .unwrap_or_else(|_| format!("127.0.0.1:{port}"));
            let _ = tx.send(Ok(addr));

            let shutdown = {
                let ct = ct_thread.clone();
                async move { ct.cancelled_owned().await }
            };
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(shutdown)
                .await;
        });
    });

    McpHandle { ct, bind_rx: rx, join: Some(join) }
}

/// Bearer 令牌校验中间件。
async fn auth_middleware(
    axum::extract::State(expected): axum::extract::State<String>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let ok = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|t| t == expected)
        .unwrap_or(false);
    if ok {
        next.run(req).await
    } else {
        (axum::http::StatusCode::UNAUTHORIZED, "missing or invalid bearer token").into_response()
    }
}

// ───────────────────────── 测试（headless）─────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn write_tmp_png(name: &str, img: &RgbaImage) -> String {
        let mut p = std::env::temp_dir();
        p.push(name);
        img.save(&p).unwrap();
        p.to_string_lossy().to_string()
    }

    fn embed_blue_lsb(img: &mut RgbaImage, msg: &[u8]) {
        let bits: Vec<u8> = msg.iter().flat_map(|b| (0..8).rev().map(move |i| (b >> i) & 1)).collect();
        let w = img.width();
        for (k, &bit) in bits.iter().enumerate() {
            let x = (k as u32) % w;
            let y = (k as u32) / w;
            let p = img.get_pixel_mut(x, y);
            p[2] = (p[2] & 0xFE) | bit;
        }
    }

    #[test]
    fn analyze_reports_png() {
        let img = RgbaImage::from_pixel(8, 8, Rgba([1, 2, 3, 255]));
        let path = write_tmp_png("mcp_analyze.png", &img);
        let r = op_analyze_format(&path);
        assert!(r.contains("PNG"), "应识别为 PNG: {r}");
    }

    #[test]
    fn render_transform_returns_decodable_png() {
        let img = RgbaImage::from_pixel(16, 16, Rgba([10, 20, 30, 255]));
        let path = write_tmp_png("mcp_render.png", &img);
        let (png, w, h, _name) = op_render_transform(&path, 33).unwrap(); // Blue plane 0
        assert_eq!((w, h), (16, 16));
        // 能被重新解码为 PNG
        let decoded = image::load_from_memory(&png).unwrap();
        assert_eq!(decoded.width(), 16);
    }

    #[test]
    fn extract_recovers_hidden_flag() {
        let mut img = RgbaImage::from_pixel(64, 4, Rgba([120, 90, 200, 255]));
        let flag = b"flag{mcp_extract_ok}";
        embed_blue_lsb(&mut img, flag);
        let path = write_tmp_png("mcp_extract.png", &img);
        let (_preview, data) = op_extract(&path, &[], &[], &[0], &[], "msb", "row", "rgb", None).unwrap();
        assert_eq!(&data[..flag.len()], flag);
    }

    #[test]
    fn auto_lsb_finds_flag() {
        let mut img = RgbaImage::from_pixel(64, 4, Rgba([0, 0, 0, 255]));
        embed_blue_lsb(&mut img, b"flag{auto_lsb_works}");
        let path = write_tmp_png("mcp_auto.png", &img);
        let report = op_auto_lsb(&path, 6).unwrap();
        assert!(report.contains("flag{auto_lsb_works}"), "auto_lsb 应找到 flag:\n{report}");
    }

    #[test]
    fn combine_and_stereo_return_png() {
        let a = RgbaImage::from_pixel(8, 8, Rgba([200, 0, 0, 255]));
        let b = RgbaImage::from_pixel(8, 8, Rgba([0, 0, 200, 255]));
        let pa = write_tmp_png("mcp_a.png", &a);
        let pb = write_tmp_png("mcp_b.png", &b);
        let (png, _, _, name) = op_combine(&pa, &pb, 0).unwrap();
        assert_eq!(name, "XOR");
        assert!(image::load_from_memory(&png).is_ok());
        let (png2, _, _) = op_stereo(&pa, 1).unwrap();
        assert!(image::load_from_memory(&png2).is_ok());
    }

    #[test]
    fn list_transforms_has_42_entries() {
        let s = op_list_transforms();
        assert!(s.contains("41:"));
        assert!(s.contains("Blue plane 0"));
    }

    /// 真·网络层验证：启动 HTTP 服务，确认能绑定，且令牌中间件生效
    /// （无令牌→401，带正确令牌→非 401）。证明"点按钮启动、客户端连 URL"这条路通。
    #[test]
    fn http_server_binds_and_enforces_token() {
        use std::io::{Read, Write};
        use std::time::Duration;

        let log: SharedLog = Arc::new(Mutex::new(Vec::new()));
        let reqs = Arc::new(AtomicUsize::new(0));
        // 端口传 0 → 由系统分配空闲端口，回传真实地址
        let mut handle = spawn_http_server(0, Some("secret-token".to_string()), log, reqs);
        let addr = handle
            .bind_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("应在 5s 内回传绑定结果")
            .expect("应绑定成功");

        let status_line = |token: Option<&str>| -> String {
            let mut s = std::net::TcpStream::connect(&addr).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(3))).ok();
            let body = "{}";
            let mut req = format!(
                "POST /mcp HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\n\
                 Accept: application/json, text/event-stream\r\nContent-Length: {}\r\n",
                body.len()
            );
            if let Some(t) = token {
                req.push_str(&format!("Authorization: Bearer {t}\r\n"));
            }
            req.push_str("Connection: close\r\n\r\n");
            req.push_str(body);
            s.write_all(req.as_bytes()).unwrap();
            let mut buf = [0u8; 256];
            let n = s.read(&mut buf).unwrap_or(0);
            String::from_utf8_lossy(&buf[..n])
                .lines()
                .next()
                .unwrap_or("")
                .to_string()
        };

        let no_auth = status_line(None);
        assert!(no_auth.contains("401"), "无令牌应 401，实际: {no_auth}");
        let with_auth = status_line(Some("secret-token"));
        assert!(!with_auth.contains("401"), "带正确令牌不应 401，实际: {with_auth}");

        handle.stop();
    }
}
