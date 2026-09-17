//! Icono de halo: un anillo de progreso parcial, igual al estilo de los
//! widgets, generado por código (sin depender de ningún archivo de imagen).

use std::f32::consts::{FRAC_PI_2, TAU};

const BACKGROUND: [u8; 4] = [0x20, 0x20, 0x20, 0xFF];
const RING_BACKGROUND: [u8; 4] = [0x3A, 0x3A, 0x3A, 0xFF];
const PROGRESS: [u8; 4] = [0xE6, 0x30, 0x27, 0xFF];
/// Fracción de arco pintada de color de progreso; puramente decorativo.
const PROGRESS_FRACTION: f32 = 0.7;

/// Genera un icono cuadrado de `size` x `size` píxeles en RGBA, con bordes
/// suavizados (antialiasing analítico por cobertura de distancia).
pub fn rgba(size: u32) -> Vec<u8> {
    let center = size as f32 / 2.0;
    let outer_r = center - 1.0;
    let ring_width = (size as f32 * 0.16).max(2.0);
    let inner_r = outer_r - ring_width;
    let start_angle = -FRAC_PI_2; // el arco arranca arriba, como en los widgets.
    let sweep = TAU * PROGRESS_FRACTION;

    let mut out = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let coverage = (outer_r + 0.5 - dist).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                continue; // queda transparente
            }

            let color = if dist >= inner_r {
                let mut angle = dy.atan2(dx) - start_angle;
                if angle < 0.0 {
                    angle += TAU;
                }
                if angle <= sweep {
                    PROGRESS
                } else {
                    RING_BACKGROUND
                }
            } else {
                BACKGROUND
            };

            let idx = ((y * size + x) * 4) as usize;
            out[idx] = color[0];
            out[idx + 1] = color[1];
            out[idx + 2] = color[2];
            out[idx + 3] = (color[3] as f32 * coverage).round() as u8;
        }
    }
    out
}
