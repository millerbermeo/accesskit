//! halo — widgets circulares de monitorización del sistema para Linux.

mod app;
mod config;
mod metrics;
mod widgets;

use std::path::PathBuf;

use eframe::egui::{ViewportBuilder, WindowLevel};

use app::HaloApp;
use config::Config;

const CONFIG_PATH: &str = "config.toml";

fn main() -> eframe::Result<()> {
    let config_path = PathBuf::from(CONFIG_PATH);
    let config = Config::load_or_create(&config_path);

    // El primer widget habilitado se pinta en la ventana raíz.
    let enabled = config.enabled_kinds();
    let Some(&root_kind) = enabled.first() else {
        eprintln!("halo: no hay ninguna métrica habilitada en {CONFIG_PATH}");
        std::process::exit(1);
    };

    let widget = config.widget;
    let mut viewport = ViewportBuilder::default()
        .with_title("halo")
        .with_inner_size([widget.size, widget.size])
        .with_min_inner_size([widget.size, widget.size])
        .with_max_inner_size([widget.size, widget.size])
        .with_resizable(false)
        .with_decorations(false)
        .with_transparent(true)
        .with_taskbar(false)
        .with_active(false)
        .with_window_level(if widget.always_on_top {
            WindowLevel::AlwaysOnTop
        } else {
            WindowLevel::Normal
        });
    if let Some([x, y]) = config.metric(root_kind).position {
        viewport = viewport.with_position([x, y]);
    }

    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        persist_window: false, // Las posiciones las gestiona config.toml.
        ..Default::default()
    };

    eframe::run_native(
        "halo",
        options,
        Box::new(move |_cc| Ok(Box::new(HaloApp::new(config, config_path)))),
    )
}
