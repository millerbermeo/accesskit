//! Icono de bandeja del sistema: menú para activar/desactivar el arranque
//! automático de halo al iniciar sesión, y para salir de la aplicación.
//!
//! egui/eframe usan winit y no corren un loop de GTK; en Linux la bandeja
//! (vía libappindicator) sí lo necesita, así que corre en su propio hilo.

use std::env;
use std::fs;
use std::path::PathBuf;

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder};

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

/// Lanza el icono de bandeja en un hilo dedicado con su propio loop de GTK.
pub fn spawn() {
    std::thread::spawn(|| {
        gtk::init().expect("halo: no se pudo inicializar gtk para el icono de bandeja");

        let autostart_item =
            CheckMenuItem::new("Iniciar con el sistema", true, is_autostart_enabled(), None);
        let quit_item = MenuItem::new("Salir", true, None);

        let menu = Menu::new();
        let _ = menu.append(&autostart_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&quit_item);

        let autostart_id = autostart_item.id().clone();
        let quit_id = quit_item.id().clone();

        let _tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(icon())
            .with_tooltip("halo")
            .build()
            .expect("halo: no se pudo crear el icono de bandeja");

        let receiver = MenuEvent::receiver();
        loop {
            gtk::main_iteration_do(true);
            while let Ok(event) = receiver.try_recv() {
                if event.id == autostart_id {
                    set_autostart(autostart_item.is_checked());
                } else if event.id == quit_id {
                    std::process::exit(0);
                }
            }
        }
    });
}

/// Genera un icono circular simple (color del anillo de progreso) sin
/// depender de ningún archivo de imagen externo.
fn icon() -> Icon {
    const SIZE: u32 = 32;
    let center = SIZE as f32 / 2.0;
    let radius = center - 1.0;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            if (dx * dx + dy * dy).sqrt() <= radius {
                rgba.extend_from_slice(&[0xE6, 0x30, 0x27, 0xFF]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    Icon::from_rgba(rgba, SIZE, SIZE).expect("halo: icono de bandeja inválido")
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
