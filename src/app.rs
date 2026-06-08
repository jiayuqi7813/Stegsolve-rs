//! 应用主体：单窗口、可停靠标签页（egui_dock）。
//!
//! 相比旧版本用 5 个 `show_viewport_immediate` 弹独立系统窗口的做法，这里把
//! 所有功能（图像查看、数据提取、立体视图、帧浏览器、图像合成、文件分析）
//! 统一收进同一个窗口的可停靠 Tab，彻底消除旧实现里"子窗口尺寸算不对 / 突然
//! 变很大 / 显示不全"的问题。全部算法逻辑来自已单测的 `stegsolve-core`。

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui::{self, Color32, RichText, TextureHandle, TextureOptions, Ui};
use egui_dock::{DockArea, DockState, Style, TabViewer};
use image::RgbaImage;

use crate::mcp;
use stegsolve_core::combine::{CombineTransform, NUM_MODES};
use stegsolve_core::extract::{
    generate_extract, generate_preview, BitOrder, Channel, ChannelMask, ExtractDirection,
    ExtractOptions, RgbOrder,
};
use stegsolve_core::stereo::StereoTransform;
use stegsolve_core::transform::Transform;

/// 标签页种类。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Image,
    Extract,
    Stereo,
    Frames,
    Combine,
    Analysis,
    Mcp,
}

impl Tab {
    fn title(self) -> &'static str {
        match self {
            Tab::Image => "图像",
            Tab::Extract => "数据提取",
            Tab::Stereo => "立体视图",
            Tab::Frames => "帧浏览器",
            Tab::Combine => "图像合成器",
            Tab::Analysis => "文件分析",
            Tab::Mcp => "MCP 服务",
        }
    }
}

/// MCP 服务的 UI/运行状态。
struct McpState {
    handle: Option<mcp::McpHandle>,
    running: bool,
    status: String,
    port_text: String,
    url: String,
    token: String,
    require_token: bool,
    log: mcp::SharedLog,
    requests: Arc<AtomicUsize>,
}

impl Default for McpState {
    fn default() -> Self {
        Self {
            handle: None,
            running: false,
            status: "未启动".to_string(),
            port_text: "7878".to_string(),
            url: String::new(),
            token: gen_token(),
            require_token: true,
            log: Arc::new(Mutex::new(Vec::new())),
            requests: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl McpState {
    fn start(&mut self) {
        let port: u16 = self.port_text.trim().parse().unwrap_or(7878);
        let token = if self.require_token { Some(self.token.clone()) } else { None };
        self.handle = Some(mcp::spawn_http_server(port, token, self.log.clone(), self.requests.clone()));
        self.running = true;
        self.status = "启动中…".to_string();
        self.url.clear();
    }

    fn stop(&mut self) {
        if let Some(mut h) = self.handle.take() {
            h.stop();
        }
        self.running = false;
        self.status = "已停止".to_string();
        self.url.clear();
    }

    /// 轮询绑定结果。
    fn poll(&mut self) {
        let mut clear = false;
        if let Some(h) = &self.handle {
            if let Ok(res) = h.bind_rx.try_recv() {
                match res {
                    Ok(addr) => {
                        self.url = format!("http://{addr}/mcp");
                        self.status = "运行中".to_string();
                    }
                    Err(e) => {
                        self.status = format!("启动失败: {e}");
                        self.running = false;
                        clear = true;
                    }
                }
            }
        }
        if clear {
            self.handle = None;
        }
    }
}

/// 生成一个 16 字节的随机十六进制令牌（无外部依赖）。
fn gen_token() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x1234_5678);
    let mut r = stegsolve_core::rng::Rng::new(nanos ^ (std::process::id() as u64).rotate_left(17));
    (0..16).map(|_| format!("{:02x}", r.byte() as u8)).collect()
}

/// 已打开图像的全部派生状态。
struct Document {
    path: String,

    transform: Transform,
    main_tex: Option<TextureHandle>,
    main_key: i32,
    zoom: f32,

    stereo: StereoTransform,
    stereo_tex: Option<TextureHandle>,
    stereo_key: i32,

    extract_opt: ExtractOptions,
    extract_preview: String,

    combine: Option<CombineTransform>,
    combine_tex: Option<TextureHandle>,
    combine_key: i32,

    frames: Vec<RgbaImage>,
    frame_index: usize,
    frame_tex: Option<TextureHandle>,
    frame_key: usize,

    analysis: Vec<String>,
}

impl Document {
    fn open(path: &Path) -> Option<Self> {
        let dynimg = match image::open(path) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("打开图片失败: {:?}", e);
                return None;
            }
        };
        let rgba = dynimg.to_rgba8();
        let transform = Transform::from_rgba(rgba.clone());
        let stereo = StereoTransform::new(rgba.clone());
        let frames = crate::frames::load_frames(path).unwrap_or_else(|_| vec![rgba.clone()]);
        let analysis = crate::fileanalysis::analyse_file_format(&path.to_string_lossy());

        // 默认勾选 RGB 三通道的 bit0（最常用的 LSB 提取）
        let mut channels = ChannelMask::new();
        channels.set(Channel::Red, 0, true);
        channels.set(Channel::Green, 0, true);
        channels.set(Channel::Blue, 0, true);
        let extract_opt = ExtractOptions { channels, ..Default::default() };

        Some(Self {
            path: path.to_string_lossy().to_string(),
            transform,
            main_tex: None,
            main_key: i32::MIN,
            zoom: 1.0,
            stereo,
            stereo_tex: None,
            stereo_key: i32::MIN,
            extract_opt,
            extract_preview: String::new(),
            combine: None,
            combine_tex: None,
            combine_key: i32::MIN,
            frames,
            frame_index: 0,
            frame_tex: None,
            frame_key: usize::MAX,
            analysis,
        })
    }
}

/// 顶层应用。
pub struct StegApp {
    dock: DockState<Tab>,
    doc: Option<Document>,
    show_about: bool,
    /// 菜单触发的"打开标签页"请求（在 DockArea 借用前处理，避免借用冲突）。
    pending_open: Vec<Tab>,
    mcp: McpState,
}

impl StegApp {
    pub fn new() -> Self {
        Self {
            dock: DockState::new(vec![Tab::Image]),
            doc: None,
            show_about: false,
            pending_open: Vec::new(),
            mcp: McpState::default(),
        }
    }

    fn open_tab(&mut self, tab: Tab) {
        if let Some(loc) = self.dock.find_tab(&tab) {
            let _ = self.dock.set_active_tab(loc);
        } else {
            self.dock.push_to_focused_leaf(tab);
        }
    }

    fn open_image_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("图片", &["png", "jpg", "jpeg", "gif", "bmp", "webp"])
            .pick_file()
        {
            self.open_path(&path);
        }
    }

    fn open_path(&mut self, path: &Path) {
        if let Some(doc) = Document::open(path) {
            self.doc = Some(doc);
        }
    }

    fn save_current(&self) {
        if let Some(doc) = &self.doc {
            if let Some(mut path) = rfd::FileDialog::new().set_file_name("solved.png").save_file() {
                if path.extension().is_none() {
                    path.set_extension("png");
                }
                if let Err(e) = doc.transform.get_image().save(&path) {
                    eprintln!("保存失败: {:?}", e);
                }
            }
        }
    }
}

impl eframe::App for StegApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 拖放打开
        let dropped = ctx.input(|i| i.raw.dropped_files.first().and_then(|f| f.path.clone()));
        if let Some(path) = dropped {
            self.open_path(&path);
        }

        // 顶部菜单栏
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("文件", |ui| {
                    if ui.button("打开").clicked() {
                        self.open_image_dialog();
                        ui.close_menu();
                    }
                    if ui.button("另存为").clicked() {
                        self.save_current();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("退出").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("分析", |ui| {
                    for (label, tab) in [
                        ("文件格式", Tab::Analysis),
                        ("数据提取", Tab::Extract),
                        ("立体视图", Tab::Stereo),
                        ("帧浏览器", Tab::Frames),
                        ("图像合成器", Tab::Combine),
                    ] {
                        if ui.button(label).clicked() {
                            self.pending_open.push(tab);
                            ui.close_menu();
                        }
                    }
                });
                ui.menu_button("工具", |ui| {
                    if ui.button("MCP 服务（供大模型调用）").clicked() {
                        self.pending_open.push(Tab::Mcp);
                        ui.close_menu();
                    }
                });
                ui.menu_button("帮助", |ui| {
                    if ui.button("关于").clicked() {
                        self.show_about = true;
                        ui.close_menu();
                    }
                });
                ui.separator();
                if self.doc.is_none() {
                    ui.label(RichText::new("未打开图片（菜单→文件→打开，或拖入图片）").weak());
                } else if let Some(doc) = &self.doc {
                    ui.label(RichText::new(&doc.path).weak());
                }
            });
        });

        // 处理菜单触发的开标签请求
        let pending = std::mem::take(&mut self.pending_open);
        for tab in pending {
            self.open_tab(tab);
        }

        // 关于窗口
        if self.show_about {
            let mut open = self.show_about;
            egui::Window::new("关于")
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label(RichText::new("StegSolve-rs").strong());
                    ui.label("版本: 0.3.0 (egui 0.33 + egui_dock 单窗口；内置 MCP 服务)");
                    ui.label("基于 Rust + egui 重构的图像隐写分析工具");
                    ui.label("作者: Sn1waR");
                });
            self.show_about = open;
        }

        // Dock 区域
        let mut viewer = Viewer { doc: &mut self.doc, mcp: &mut self.mcp };
        DockArea::new(&mut self.dock)
            .style(Style::from_egui(ctx.style().as_ref()))
            .show(ctx, &mut viewer);
    }
}

/// TabViewer：渲染各标签页内容。
struct Viewer<'a> {
    doc: &'a mut Option<Document>,
    mcp: &'a mut McpState,
}

impl TabViewer for Viewer<'_> {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn closeable(&mut self, tab: &mut Self::Tab) -> bool {
        // 图像主视图不可关闭，其余可关闭
        *tab != Tab::Image
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        // MCP 面板与图片无关，单独处理
        if *tab == Tab::Mcp {
            mcp_tab(ui, self.mcp);
            return;
        }
        match self.doc {
            None => {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("请先打开一张图片\n（菜单 → 文件 → 打开，或直接拖入图片）").weak());
                });
            }
            Some(doc) => match tab {
                Tab::Image => image_tab(ui, doc),
                Tab::Extract => extract_tab(ui, doc),
                Tab::Stereo => stereo_tab(ui, doc),
                Tab::Frames => frames_tab(ui, doc),
                Tab::Combine => combine_tab(ui, doc),
                Tab::Analysis => analysis_tab(ui, doc),
                Tab::Mcp => {} // 已在上面提前处理
            },
        }
    }
}

/// 「MCP 服务」面板：启停、URL、令牌、请求日志。
fn mcp_tab(ui: &mut Ui, mcp: &mut McpState) {
    mcp.poll();

    ui.heading("MCP 服务");
    ui.label("让大模型通过 MCP 调用本工具的隐写分析能力。点「启动」后，把下面的 URL（和令牌）加到你的 MCP 客户端即可。");
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("端口:");
        ui.add_enabled(
            !mcp.running,
            egui::TextEdit::singleline(&mut mcp.port_text).desired_width(70.0),
        );
        ui.add_enabled_ui(!mcp.running, |ui| {
            ui.checkbox(&mut mcp.require_token, "需要令牌");
        });
        if mcp.running {
            if ui.button("停止").clicked() {
                mcp.stop();
            }
        } else if ui.button("启动").clicked() {
            mcp.start();
        }
    });

    ui.separator();
    ui.horizontal(|ui| {
        let col = if mcp.running { Color32::from_rgb(0x35, 0xc0, 0x59) } else { Color32::GRAY };
        ui.colored_label(col, "●");
        ui.label(format!("状态: {}", mcp.status));
        ui.label(format!("已处理请求: {}", mcp.requests.load(Ordering::Relaxed)));
    });

    if !mcp.url.is_empty() {
        ui.horizontal(|ui| {
            ui.label("URL:");
            ui.monospace(&mcp.url);
            if ui.button("复制").clicked() {
                ui.ctx().copy_text(mcp.url.clone());
            }
        });
        if mcp.require_token {
            ui.horizontal(|ui| {
                ui.label("令牌:");
                ui.monospace(&mcp.token);
                if ui.button("复制").clicked() {
                    ui.ctx().copy_text(mcp.token.clone());
                }
            });
            ui.label(RichText::new("客户端需在请求头带: Authorization: Bearer <令牌>").weak());
        }
        ui.label(RichText::new("提示：在 Claude Desktop / Cline 等里把上面的 URL 添加为自定义 MCP/HTTP 连接器。").weak());
    }

    ui.separator();
    ui.label("请求日志:");
    egui::ScrollArea::vertical()
        .max_height(260.0)
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if let Ok(log) = mcp.log.lock() {
                if log.is_empty() {
                    ui.label(RichText::new("(暂无调用)").weak());
                } else {
                    for line in log.iter() {
                        ui.monospace(line);
                    }
                }
            }
        });

    // 运行时保持轻量刷新，让状态/日志实时更新
    if mcp.running {
        ui.ctx().request_repaint_after(Duration::from_millis(500));
    }
}

// ───────────────────────── 工具函数 ─────────────────────────

fn color_image(img: &RgbaImage) -> egui::ColorImage {
    let size = [img.width() as usize, img.height() as usize];
    egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw())
}

fn make_texture(ui: &Ui, name: &str, img: &RgbaImage) -> TextureHandle {
    ui.ctx()
        .load_texture(name, color_image(img), TextureOptions::NEAREST)
}

/// 在可滚动区域里按 zoom 显示纹理。返回是否处于该区域内（用于缩放快捷键）。
fn show_image(ui: &mut Ui, tex: &TextureHandle, zoom: f32) {
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let size = tex.size_vec2() * zoom;
            ui.image(egui::load::SizedTexture::new(tex.id(), size));
        });
}

fn nav_buttons(ui: &mut Ui) -> (bool, bool) {
    let mut back = false;
    let mut fwd = false;
    if ui.button(RichText::new("◀").monospace()).clicked() {
        back = true;
    }
    if ui.button(RichText::new("▶").monospace()).clicked() {
        fwd = true;
    }
    // 方向键
    let (l, r) = ui.input(|i| {
        (
            i.key_pressed(egui::Key::ArrowLeft),
            i.key_pressed(egui::Key::ArrowRight),
        )
    });
    (back || l, fwd || r)
}

// ───────────────────────── 各标签页 ─────────────────────────

fn image_tab(ui: &mut Ui, doc: &mut Document) {
    ui.horizontal_wrapped(|ui| {
        if ui.button("打开").clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_file() {
                if let Some(d) = Document::open(&path) {
                    *doc = d;
                }
            }
        }
        if ui.button("另存为").clicked() {
            if let Some(mut path) = rfd::FileDialog::new().set_file_name("solved.png").save_file() {
                if path.extension().is_none() {
                    path.set_extension("png");
                }
                let _ = doc.transform.get_image().save(&path);
            }
        }
        ui.separator();

        let (back, fwd) = nav_buttons(ui);
        if back {
            doc.transform.back();
        }
        if fwd {
            doc.transform.forward();
        }
        ui.label(RichText::new(format!("通道: {}", doc.transform.get_text())).strong());

        ui.separator();
        ui.label("缩放");
        ui.add(egui::Slider::new(&mut doc.zoom, 0.1..=8.0).step_by(0.05));
        if ui.button("1:1").clicked() {
            doc.zoom = 1.0;
        }
        // Ctrl+滚轮缩放（与普通滚动分离，修复旧版滚轮/缩放冲突）
        let scroll = ui.input(|i| {
            if i.modifiers.command || i.modifiers.ctrl {
                i.smooth_scroll_delta.y
            } else {
                0.0
            }
        });
        if scroll != 0.0 {
            doc.zoom = (doc.zoom * (1.0 + scroll * 0.001)).clamp(0.1, 8.0);
        }
    });
    ui.separator();

    if doc.main_tex.is_none() || doc.main_key != doc.transform.trans_num() {
        doc.main_tex = Some(make_texture(ui, "main_view", doc.transform.get_image()));
        doc.main_key = doc.transform.trans_num();
    }
    if let Some(tex) = &doc.main_tex {
        show_image(ui, tex, doc.zoom);
    }
}

fn stereo_tab(ui: &mut Ui, doc: &mut Document) {
    ui.horizontal_wrapped(|ui| {
        let (back, fwd) = nav_buttons(ui);
        if back {
            doc.stereo.back();
        }
        if fwd {
            doc.stereo.forward();
        }
        ui.label(RichText::new(doc.stereo.get_text()).strong());
        ui.separator();
        if ui.button("保存").clicked() {
            if let Some(mut path) = rfd::FileDialog::new().set_file_name("stereo.png").save_file() {
                if path.extension().is_none() {
                    path.set_extension("png");
                }
                let _ = doc.stereo.get_image().save(&path);
            }
        }
        ui.label(RichText::new("◀ ▶ 或方向键调整偏移量").weak());
    });
    ui.separator();

    if doc.stereo_tex.is_none() || doc.stereo_key != doc.stereo.trans_num() {
        doc.stereo_tex = Some(make_texture(ui, "stereo_view", doc.stereo.get_image()));
        doc.stereo_key = doc.stereo.trans_num();
    }
    if let Some(tex) = &doc.stereo_tex {
        show_image(ui, tex, 1.0);
    }
}

fn frames_tab(ui: &mut Ui, doc: &mut Document) {
    let n = doc.frames.len();
    ui.horizontal_wrapped(|ui| {
        let (back, fwd) = nav_buttons(ui);
        if n > 0 {
            if back {
                doc.frame_index = (doc.frame_index + n - 1) % n;
            }
            if fwd {
                doc.frame_index = (doc.frame_index + 1) % n;
            }
        }
        ui.label(RichText::new(format!("帧: {} / {}", doc.frame_index + 1, n)).strong());
        ui.separator();
        if ui.button("保存当前帧").clicked() {
            if let Some(mut path) = rfd::FileDialog::new()
                .set_file_name(format!("frame{}.png", doc.frame_index + 1))
                .save_file()
            {
                if path.extension().is_none() {
                    path.set_extension("png");
                }
                if let Some(f) = doc.frames.get(doc.frame_index) {
                    let _ = f.save(&path);
                }
            }
        }
    });
    ui.separator();

    if doc.frame_tex.is_none() || doc.frame_key != doc.frame_index {
        if let Some(f) = doc.frames.get(doc.frame_index) {
            doc.frame_tex = Some(make_texture(ui, "frame_view", f));
            doc.frame_key = doc.frame_index;
        }
    }
    if let Some(tex) = &doc.frame_tex {
        show_image(ui, tex, 1.0);
    }
}

fn combine_tab(ui: &mut Ui, doc: &mut Document) {
    // 拖放第二张图
    let dropped = ui
        .ctx()
        .input(|i| i.raw.dropped_files.first().and_then(|f| f.path.clone()));

    ui.horizontal_wrapped(|ui| {
        if ui.button("打开第二张图片").clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_file() {
                if let Ok(img2) = image::open(&path) {
                    doc.combine = Some(CombineTransform::new(
                        doc.transform.original_image().clone(),
                        img2.to_rgba8(),
                    ));
                    doc.combine_key = i32::MIN;
                }
            }
        }
        if let Some(c) = &mut doc.combine {
            let (back, fwd) = nav_buttons(ui);
            if back {
                c.back();
            }
            if fwd {
                c.forward();
            }
            ui.label(RichText::new(c.get_text()).strong());
            ui.separator();
            if ui.button("保存").clicked() {
                if let Some(mut path) =
                    rfd::FileDialog::new().set_file_name("combined.png").save_file()
                {
                    if path.extension().is_none() {
                        path.set_extension("png");
                    }
                    let _ = c.get_image().save(&path);
                }
            }
        } else {
            ui.label(RichText::new("请选择或拖入第二张图片").weak());
        }
    });
    ui.separator();

    if let Some(path) = dropped {
        if let Ok(img2) = image::open(&path) {
            doc.combine = Some(CombineTransform::new(
                doc.transform.original_image().clone(),
                img2.to_rgba8(),
            ));
            doc.combine_key = i32::MIN;
        }
    }

    if let Some(c) = &doc.combine {
        if doc.combine_tex.is_none() || doc.combine_key != c.mode().index() {
            doc.combine_tex = Some(make_texture(ui, "combine_view", c.get_image()));
            doc.combine_key = c.mode().index();
        }
        if let Some(tex) = &doc.combine_tex {
            show_image(ui, tex, 1.0);
        }
        ui.label(
            RichText::new(format!("模式 {}/{}（◀ ▶ 切换）", c.mode().index() + 1, NUM_MODES))
                .weak(),
        );
    }
}

fn analysis_tab(ui: &mut Ui, doc: &mut Document) {
    ui.horizontal(|ui| {
        if ui.button("复制到剪贴板").clicked() {
            ui.ctx().copy_text(doc.analysis.join("\n"));
        }
        if ui.button("导出报告").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("文本", &["txt"])
                .set_file_name("analysis.txt")
                .save_file()
            {
                let _ = std::fs::write(&path, doc.analysis.join("\n"));
            }
        }
    });
    ui.separator();
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for line in &doc.analysis {
                ui.label(RichText::new(line).monospace());
            }
        });
}

fn extract_tab(ui: &mut Ui, doc: &mut Document) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("位平面选择");
        egui::Grid::new("bitplane_grid")
            .spacing([6.0, 4.0])
            .show(ui, |ui| {
                ui.label("");
                ui.label(RichText::new("全选").weak());
                for n in (0..8).rev() {
                    ui.label(RichText::new(n.to_string()).monospace());
                }
                ui.end_row();

                for (name, ch) in [
                    ("Red", Channel::Red),
                    ("Green", Channel::Green),
                    ("Blue", Channel::Blue),
                    ("Alpha", Channel::Alpha),
                ] {
                    channel_row(ui, &mut doc.extract_opt.channels, name, ch);
                    ui.end_row();
                }
            });

        ui.separator();
        ui.heading("提取选项");
        egui::Grid::new("opt_grid").spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label("提取方向:");
            ui.horizontal(|ui| {
                ui.radio_value(&mut doc.extract_opt.direction, ExtractDirection::Row, "按行");
                ui.radio_value(&mut doc.extract_opt.direction, ExtractDirection::Column, "按列");
            });
            ui.end_row();

            ui.label("位顺序:");
            ui.horizontal(|ui| {
                ui.radio_value(&mut doc.extract_opt.bit_order, BitOrder::MsbFirst, "MSB 优先");
                ui.radio_value(&mut doc.extract_opt.bit_order, BitOrder::LsbFirst, "LSB 优先");
            });
            ui.end_row();

            ui.label("通道顺序:");
            ui.horizontal_wrapped(|ui| {
                for (label, val) in [
                    ("RGB", RgbOrder::Rgb),
                    ("RBG", RgbOrder::Rbg),
                    ("GRB", RgbOrder::Grb),
                    ("GBR", RgbOrder::Gbr),
                    ("BRG", RgbOrder::Brg),
                    ("BGR", RgbOrder::Bgr),
                ] {
                    ui.radio_value(&mut doc.extract_opt.rgb_order, val, label);
                }
            });
            ui.end_row();

            ui.label("预览十六进制:");
            ui.checkbox(&mut doc.extract_opt.include_hex, "");
            ui.end_row();
        });

        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("预览").clicked() {
                regenerate_preview(doc);
            }
            if ui.button("保存二进制").clicked() {
                let data = generate_extract(doc.transform.original_image(), &doc.extract_opt);
                if let Some(path) = rfd::FileDialog::new().set_file_name("extract.bin").save_file() {
                    let _ = std::fs::write(&path, &data);
                }
            }
            if ui.button("保存文本").clicked() {
                regenerate_preview(doc);
                if let Some(path) = rfd::FileDialog::new().set_file_name("extract.txt").save_file() {
                    let _ = std::fs::write(&path, doc.extract_preview.as_bytes());
                }
            }
        });

        ui.separator();
        ui.label(RichText::new("预览").strong());
        egui::ScrollArea::vertical()
            .id_salt("extract_preview")
            .max_height(260.0)
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut doc.extract_preview.as_str())
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .desired_rows(12),
                );
            });
    });
}

fn channel_row(ui: &mut Ui, mask: &mut ChannelMask, name: &str, ch: Channel) {
    ui.label(RichText::new(name).strong());

    let all = (0..8).all(|n| mask.get(ch, n));
    let mut all_state = all;
    if ui.checkbox(&mut all_state, "").changed() {
        mask.set_all(ch, all_state);
    }

    for n in (0..8u8).rev() {
        let mut b = mask.get(ch, n);
        if ui.checkbox(&mut b, "").changed() {
            mask.set(ch, n, b);
        }
    }
}

fn regenerate_preview(doc: &mut Document) {
    let data = generate_extract(doc.transform.original_image(), &doc.extract_opt);
    doc.extract_preview = generate_preview(&data, doc.extract_opt.include_hex);
}