#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // release 下隐藏 Windows 控制台
#![allow(rustdoc::missing_crate_level_docs)]

mod apng_decoder;
mod fileanalysis;
mod frames;
mod mcp;
mod app;

use app::StegApp;

fn main() {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 760.0])
            .with_min_inner_size([640.0, 480.0])
            .with_title("StegSolve-rs"),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "StegSolve-rs",
        options,
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            Ok(Box::new(StegApp::new()))
        }),
    ) {
        eprintln!("启动失败: {}", e);
    }
}

/// 安装内置 MiSans 字体，保证中文正常显示。
fn install_fonts(ctx: &eframe::egui::Context) {
    use eframe::egui::{FontData, FontDefinitions, FontFamily};
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "misans".to_owned(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../font/MiSans-Normal.ttf"
        ))),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "misans".to_owned());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .push("misans".to_owned());
    ctx.set_fonts(fonts);
}
