//! halo — widgets circulares de monitorización del sistema para Linux.

mod app;
mod config;
mod metrics;
mod tray;
mod widgets;

use std::env;
use std::path::PathBuf;

use eframe::egui::{ViewportBuilder, WindowLevel};

use app::HaloApp;
use config::Config;

/// Ruta del config: `$XDG_CONFIG_HOME/halo/config.toml` o `~/.config/halo/config.toml`.
/// Si no hay `HOME`, cae en `config.toml` junto al directorio de trabajo actual.
fn config_path() -> PathBuf {
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("halo/config.toml");
    }
    if let Ok(home) = env::var("HOME") {
        return PathBuf::from(home).join(".config/halo/config.toml");
    }
    PathBuf::from("config.toml")
}

/// Fuerza a winit a usar el backend X11 (vía XWayland si la sesión es
/// Wayland) quitando `WAYLAND_DISPLAY` del entorno antes de que arranque.
/// El "always on top" de halo depende de EWMH (`_NET_WM_STATE_ABOVE`), que
/// los compositores Wayland no exponen a apps normales; Mutter y KWin sí lo
/// aplican a las ventanas X11 que gestionan vía XWayland. No cambia nada de
/// la sesión de escritorio del usuario, solo cómo se conecta este proceso.
fn force_x11_backend() {
    // SAFETY: se llama al inicio de main(), antes de crear cualquier hilo
    // (tray::spawn) o abrir una conexión de pantalla.
    unsafe {
        env::remove_var("WAYLAND_DISPLAY");
    }
}

/// Desatacha el proceso de la terminal (fork + setsid + stdio a `/dev/null`),
/// para que `halo` alcance como comando único y sobreviva al cierre de la
/// terminal. Debe llamarse antes de crear hilos o conexiones (X11, GTK), ya
/// que solo el hilo que llama a `fork()` sobrevive en el hijo.
/// `HALO_FOREGROUND=1` lo desactiva (útil para depurar con salida visible).
fn daemonize() {
    if env::var_os("HALO_FOREGROUND").is_some() {
        return;
    }
    // nochdir=1: conserva el directorio de trabajo. noclose=0: redirige
    // stdin/stdout/stderr a /dev/null.
    unsafe {
        libc::daemon(1, 0);
    }
}

fn main() -> eframe::Result<()> {
    force_x11_backend();
    daemonize();
    tray::spawn();

    let config_path = config_path();
    let config = Config::load_or_create(&config_path);

    // El primer widget habilitado se pinta en la ventana raíz.
    let enabled = config.enabled_kinds();
    let Some(&root_kind) = enabled.first() else {
        eprintln!(
            "halo: no hay ninguna métrica habilitada en {}",
            config_path.display()
        );
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
