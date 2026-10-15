#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod canvas;
mod design;
mod settings;
fn main() -> eframe::Result {
    let open = std::env::args_os()
        .skip(1)
        .find(|a| !a.to_string_lossy().starts_with("--"))
        .map(std::path::PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1380., 880.])
            .with_min_inner_size([960., 600.])
            .with_icon({
                let image = image::load_from_memory(include_bytes!(
                    "../../../resources/branding/jenga.png"
                ))
                .expect("app icon")
                .into_rgba8();
                eframe::egui::IconData {
                    width: image.width(),
                    height: image.height(),
                    rgba: image.into_raw(),
                }
            }),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Jenga",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, open)))),
    )
}
