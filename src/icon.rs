//! Icono de halo: carga el logo oficial (`logo.png`, embebido en el binario)
//! y lo escala al tamaño pedido (bandeja del sistema, etc).

use std::sync::OnceLock;

use image::imageops::FilterType;
use image::RgbaImage;

static LOGO_BYTES: &[u8] = include_bytes!("../logo.png");

fn logo() -> &'static RgbaImage {
    static LOGO: OnceLock<RgbaImage> = OnceLock::new();
    LOGO.get_or_init(|| {
        image::load_from_memory(LOGO_BYTES)
            .expect("halo: logo.png inválido o corrupto")
            .into_rgba8()
    })
}

/// Devuelve el logo de halo escalado a `size` x `size` píxeles, en RGBA
/// contiguo (fila por fila), listo para `TrayIcon::from_rgba` / `IconData`.
pub fn rgba(size: u32) -> Vec<u8> {
    let logo = logo();
    if logo.width() == size && logo.height() == size {
        return logo.as_raw().clone();
    }
    image::imageops::resize(logo, size, size, FilterType::Lanczos3).into_raw()
}
