mod app;

use app::StegApp;


fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0]),
        ..Default::default()
    };
    
    if let Err(e) = eframe::run_native(
        "StegSolve (Rust + Egui)",
        options,
        Box::new(|cc| {
            // 字体设置移动到 app.rs
            StegApp::new(cc)
        }),
    ) {
        eprintln!("Error: {}", e);
    }
}