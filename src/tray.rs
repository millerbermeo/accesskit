//! Icono de bandeja del sistema: submenú por métrica (mostrar/ocultar y
//! color), arranque automático y salir.
//!
//! egui/eframe usan winit y no corren un loop de GTK; en Linux la bandeja
//! (vía libappindicator) sí lo necesita, así que corre en su propio hilo.
//! Los cambios hechos acá se escriben en `SharedState` (config compartida
//! con el hilo de la app) para que se apliquen en caliente.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::Color32;
use tray_icon::menu::{
    CheckMenuItem, Icon as MenuIcon, IconMenuItem, Menu, MenuEvent, MenuId, MenuItem,
    PredefinedMenuItem, Submenu,
};
use tray_icon::{Icon as TrayIcon, TrayIconBuilder};

use crate::config::{parse_hex_color, MetricKind, Orientation};
use crate::icon;
use crate::shared::SharedState;

/// Paleta curada de colores para el arco de progreso de cada métrica.
const PALETTE: [(&str, &str); 8] = [
    ("Rojo", "#E63027"),
    ("Naranja", "#FF8C42"),
    ("Amarillo", "#FFC857"),
    ("Verde", "#4CAF50"),
    ("Turquesa", "#2EC4B6"),
    ("Azul", "#4361EE"),
    ("Violeta", "#9B5DE5"),
    ("Rosa", "#F15BB5"),
];

const METRIC_KINDS: [MetricKind; 4] = [
    MetricKind::Cpu,
    MetricKind::Ram,
    MetricKind::Disk,
    MetricKind::Gpu,
];

fn display_name(kind: MetricKind) -> &'static str {
    match kind {
        MetricKind::Cpu => "CPU",
        MetricKind::Ram => "RAM",
        MetricKind::Disk => "Disco",
        MetricKind::Gpu => "GPU",
    }
}

/// Qué hacer cuando llega el evento de un item de menú.
enum Action {
    ToggleAutostart(CheckMenuItem),
    ToggleShow(MetricKind, CheckMenuItem),
    /// Metrica, hex elegido, y todos los swatches hermanos (para refrescar
    /// su check visual tras el cambio).
    SetColor(MetricKind, &'static str, Vec<(&'static str, IconMenuItem)>),
    /// Nueva orientación, su propio item (para reafirmar el check) y el
    /// item de la otra orientación (para desmarcarlo, efecto radio-button).
    SetOrientation(Orientation, CheckMenuItem, CheckMenuItem),
    Quit,
}

/// Ancho mínimo (en caracteres) de un item de menú. Los menús nativos se
/// ajustan al contenido, así que las etiquetas cortas ("RAM", "Mostrar",
/// nombres de colores) dejaban el menú muy angosto; rellenar con espacios
/// a la derecha lo ensancha sin depender de una API de ancho fijo.
const MIN_LABEL_WIDTH: usize = 22;

fn pad(label: &str) -> String {
    let len = label.chars().count();
    if len >= MIN_LABEL_WIDTH {
        label.to_string()
    } else {
        format!("{label}{}", " ".repeat(MIN_LABEL_WIDTH - len))
    }
}

fn autostart_desktop_path() -> PathBuf {
    let base = env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env::var("HOME").unwrap_or_default()).join(".config"));
    base.join("autostart/halo.desktop")
}

fn is_autostart_enabled() -> bool {
    autostart_desktop_path().exists()
}

/// Crea o borra el `.desktop` en `~/.config/autostart/`, que es lo que
/// persiste la opción entre reinicios (XDG Autostart).
fn set_autostart(enabled: bool) {
    let path = autostart_desktop_path();
    if enabled {
        let Ok(exe) = env::current_exe() else {
            eprintln!("halo: no se pudo resolver la ruta del ejecutable para autostart");
            return;
        };
        if let Some(parent) = path.parent()
            && let Err(err) = fs::create_dir_all(parent)
        {
            eprintln!("halo: no se pudo crear {}: {err}", parent.display());
            return;
        }
        let contents = format!(
            "[Desktop Entry]\nType=Application\nName=halo\nExec={}\nX-GNOME-Autostart-enabled=true\nNoDisplay=true\n",
            exe.display()
        );
        if let Err(err) = fs::write(&path, contents) {
            eprintln!("halo: no se pudo escribir {}: {err}", path.display());
        }
    } else if let Err(err) = fs::remove_file(&path)
        && err.kind() != std::io::ErrorKind::NotFound
    {
        eprintln!("halo: no se pudo borrar {}: {err}", path.display());
    }
}

fn swatch_label(name: &str, selected: bool) -> String {
    if selected {
        format!("✓ {name}")
    } else {
        name.to_string()
    }
}

/// Si `configured` es `None`, el color activo es el del tema por defecto.
fn is_selected(configured: Option<&str>, theme_default: &str, hex: &str) -> bool {
    match configured {
        Some(c) => c.eq_ignore_ascii_case(hex),
        None => theme_default.eq_ignore_ascii_case(hex),
    }
}

/// Icono de swatch: un círculo relleno del color dado, para mostrar en la
/// paleta del menú.
fn swatch_icon(hex: &str) -> MenuIcon {
    const SIZE: u32 = 16;
    let color = parse_hex_color(hex).unwrap_or(Color32::WHITE);
    let center = SIZE as f32 / 2.0;
    let radius = center - 1.0;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            let coverage = (radius + 0.5 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
            rgba.extend_from_slice(&[color.r(), color.g(), color.b(), (255.0 * coverage) as u8]);
        }
    }
    MenuIcon::from_rgba(rgba, SIZE, SIZE).expect("halo: swatch de color inválido")
}

/// Construye el submenú de una métrica ("Mostrar" + paleta de colores) y
/// registra sus acciones.
fn build_metric_submenu(
    kind: MetricKind,
    shared: &SharedState,
    actions: &mut HashMap<MenuId, Action>,
) -> Submenu {
    let (enabled, configured_color, theme_default) = {
        let config = shared.config.lock().unwrap();
        let metric = config.metric(kind);
        (
            metric.enabled,
            metric.color.clone(),
            config.theme.progress_color.clone(),
        )
    };

    let submenu = Submenu::new(display_name(kind), true);

    let show_item = CheckMenuItem::new(pad("Mostrar"), true, enabled, None);
    let _ = submenu.append(&show_item);
    actions.insert(show_item.id().clone(), Action::ToggleShow(kind, show_item));

    let _ = submenu.append(&PredefinedMenuItem::separator());

    let mut swatches = Vec::with_capacity(PALETTE.len());
    for (name, hex) in PALETTE {
        let selected = is_selected(configured_color.as_deref(), &theme_default, hex);
        let item = IconMenuItem::new(
            pad(&swatch_label(name, selected)),
            true,
            Some(swatch_icon(hex)),
            None,
        );
        let _ = submenu.append(&item);
        swatches.push((hex, item));
    }
    for (hex, item) in &swatches {
        actions.insert(
            item.id().clone(),
            Action::SetColor(kind, hex, swatches.clone()),
        );
    }

    submenu
}

/// Lanza el icono de bandeja en un hilo dedicado con su propio loop de GTK.
pub fn spawn(shared: Arc<SharedState>) {
    std::thread::spawn(move || {
        gtk::init().expect("halo: no se pudo inicializar gtk para el icono de bandeja");

        let mut actions: HashMap<MenuId, Action> = HashMap::new();
        let menu = Menu::new();

        for kind in METRIC_KINDS {
            let submenu = build_metric_submenu(kind, &shared, &mut actions);
            let _ = menu.append(&submenu);
        }

        let _ = menu.append(&PredefinedMenuItem::separator());
        let orientation_now = shared.config.lock().unwrap().widget.orientation;
        let horizontal_item = CheckMenuItem::new(
            pad("Horizontal"),
            true,
            orientation_now == Orientation::Horizontal,
            None,
        );
        let vertical_item = CheckMenuItem::new(
            pad("Vertical"),
            true,
            orientation_now == Orientation::Vertical,
            None,
        );
        let _ = menu.append(&horizontal_item);
        let _ = menu.append(&vertical_item);
        actions.insert(
            horizontal_item.id().clone(),
            Action::SetOrientation(Orientation::Horizontal, horizontal_item.clone(), vertical_item.clone()),
        );
        actions.insert(
            vertical_item.id().clone(),
            Action::SetOrientation(Orientation::Vertical, vertical_item, horizontal_item),
        );

        let _ = menu.append(&PredefinedMenuItem::separator());
        let autostart_item = CheckMenuItem::new(
            pad("Iniciar con el sistema"),
            true,
            is_autostart_enabled(),
            None,
        );
        let _ = menu.append(&autostart_item);
        actions.insert(
            autostart_item.id().clone(),
            Action::ToggleAutostart(autostart_item),
        );

        let _ = menu.append(&PredefinedMenuItem::separator());
        let quit_item = MenuItem::new(pad("Salir"), true, None);
        let _ = menu.append(&quit_item);
        actions.insert(quit_item.id().clone(), Action::Quit);

        let _tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(tray_icon())
            .with_tooltip("halo")
            .build()
            .expect("halo: no se pudo crear el icono de bandeja");

        let receiver = MenuEvent::receiver();
        loop {
            gtk::main_iteration_do(true);
            while let Ok(event) = receiver.try_recv() {
                match actions.get(&event.id) {
                    Some(Action::ToggleAutostart(item)) => set_autostart(item.is_checked()),
                    Some(Action::SetOrientation(orientation, this_item, other_item)) => {
                        shared.config.lock().unwrap().widget.orientation = *orientation;
                        this_item.set_checked(true);
                        other_item.set_checked(false);
                        shared.save();
                        shared.request_repaint();
                    }
                    Some(Action::Quit) => std::process::exit(0),
                    Some(Action::ToggleShow(kind, item)) => {
                        let enabled = item.is_checked();
                        shared.config.lock().unwrap().metric_mut(*kind).enabled = enabled;
                        shared.save();
                        shared.request_repaint();
                    }
                    Some(Action::SetColor(kind, hex, siblings)) => {
                        shared.config.lock().unwrap().metric_mut(*kind).color =
                            Some((*hex).to_string());
                        shared.save();
                        shared.request_repaint();
                        for (sibling_hex, item) in siblings {
                            let name = PALETTE
                                .iter()
                                .find(|(_, h)| h == sibling_hex)
                                .map(|(name, _)| *name)
                                .unwrap_or("?");
                            item.set_text(pad(&swatch_label(name, sibling_hex == hex)));
                        }
                    }
                    None => {}
                }
            }
        }
    });
}

/// Icono de bandeja: el anillo de progreso de `crate::icon`, en alta
/// resolución para que se vea nítido en la bandeja del sistema.
fn tray_icon() -> TrayIcon {
    const SIZE: u32 = 64;
    TrayIcon::from_rgba(icon::rgba(SIZE), SIZE, SIZE).expect("halo: icono de bandeja inválido")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `set_autostart` debe crear y borrar el `.desktop`, y `is_autostart_enabled`
    /// debe reflejar ese estado — es lo que persiste la opción entre reinicios.
    #[test]
    fn autostart_round_trip() {
        let tmp_home = std::env::temp_dir().join("halo_autostart_test");
        let _ = fs::remove_dir_all(&tmp_home);
        // SAFETY: test de un solo hilo, no hay otro código leyendo esta env var a la vez.
        unsafe {
            env::set_var("XDG_CONFIG_HOME", tmp_home.join(".config"));
        }

        assert!(!is_autostart_enabled());

        set_autostart(true);
        assert!(is_autostart_enabled());
        let contents = fs::read_to_string(autostart_desktop_path()).unwrap();
        assert!(contents.contains("Exec="));
        assert!(contents.contains("X-GNOME-Autostart-enabled=true"));

        set_autostart(false);
        assert!(!is_autostart_enabled());

        let _ = fs::remove_dir_all(&tmp_home);
    }
}
