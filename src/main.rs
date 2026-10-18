#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod model;
mod render;

fn main() -> eframe::Result {
    if let Some(path) = std::env::args()
        .nth(1)
        .filter(|s| s == "--self-test")
        .and_then(|_| std::env::args().nth(2))
    {
        match render::self_test(std::path::Path::new(&path)) {
            Ok(()) => std::process::exit(0),
            Err(error) => {
                let _ = std::fs::write(std::path::Path::new(&path).join("error.txt"), error);
                std::process::exit(1);
            }
        }
    }
    let smoke = std::env::args()
        .position(|s| s == "--smoke-test")
        .and_then(|n| std::env::args().nth(n + 1));
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1500.0, 1000.0])
            .with_min_inner_size([1080.0, 700.0])
            .with_title("Magic Circle Maker · Beta"),
        ..Default::default()
    };
    eframe::run_native(
        "Magic Circle Maker",
        options,
        Box::new(move |cc| Ok(Box::new(app::MakerApp::new(cc, smoke)))),
    )
}
