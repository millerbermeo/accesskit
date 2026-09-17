//! Widget circular de progreso (solo pintado, sin estado).

use eframe::egui::{
    Align2, Color32, FontId, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2,
};

use crate::config::Theme;

/// Anillo circular grueso con el porcentaje grande centrado y el nombre de la
/// métrica debajo. El arco empieza en la parte superior y avanza en sentido
/// horario representando exactamente el porcentaje.
pub struct CircularProgress<'a> {
    /// Porcentaje a representar (0.0–100.0).
    value: f32,
    label: &'a str,
    theme: &'a Theme,
    /// Color del arco de progreso, propio de cada métrica (no viene del tema).
    progress_color: Color32,
    size: f32,
    opacity: f32,
    show_label: bool,
}

impl<'a> CircularProgress<'a> {
    pub fn new(
        value: f32,
        label: &'a str,
        theme: &'a Theme,
        progress_color: Color32,
        size: f32,
        opacity: f32,
        show_label: bool,
    ) -> Self {
        Self {
            value: value.clamp(0.0, 100.0),
            label,
            theme,
            progress_color,
            size,
            opacity: opacity.clamp(0.0, 1.0),
            show_label,
        }
    }

    /// Reserva el espacio del widget y lo pinta. Devuelve la respuesta de
    /// arrastre para que la ventana pueda moverse.
    pub fn show(&self, ui: &mut Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(self.size), Sense::drag());
        self.paint(ui, rect);
        response
    }

    fn paint(&self, ui: &mut Ui, rect: Rect) {
        let painter = ui.painter();
        let center = rect.center();
        // Margen entre el círculo y el borde de la ventana: si el diámetro
        // coincidiera exactamente con el tamaño de la ventana, el suavizado
        // del borde quedaría cortado en seco justo en los puntos de
        // tangencia (arriba/abajo/lados), y el círculo se ve "achatado" ahí.
        const EDGE_MARGIN: f32 = 3.0;
        let radius = self.size * 0.5 - EDGE_MARGIN;
        let ring_width = self.theme.ring_width.clamp(1.0, radius);
        let ring_radius = radius - ring_width * 0.5;

        // Fondo del widget y anillo base.
        painter.circle_filled(center, radius, self.apply_opacity(self.theme.background));
        painter.circle_stroke(
            center,
            ring_radius,
            Stroke::new(ring_width, self.apply_opacity(self.theme.ring_background)),
        );

        // Arco de progreso: empieza arriba (-90°) y avanza en sentido horario.
        let fraction = self.value / 100.0;
        if fraction > 0.0 {
            let start_angle = -std::f32::consts::FRAC_PI_2;
            let sweep = fraction * std::f32::consts::TAU;
            let segments = ((ring_radius * sweep / 3.0) as usize).clamp(16, 256);
            let points: Vec<Pos2> = (0..=segments)
                .map(|i| {
                    let angle = start_angle + sweep * i as f32 / segments as f32;
                    center + ring_radius * Vec2::angled(angle)
                })
                .collect();
            let color = self.apply_opacity(self.progress_color);
            painter.add(Shape::line(points.clone(), Stroke::new(ring_width, color)));
            // Extremos redondeados del arco.
            painter.circle_filled(points[0], ring_width * 0.5, color);
            painter.circle_filled(*points.last().unwrap(), ring_width * 0.5, color);
        }

        // Porcentaje grande centrado.
        painter.text(
            center,
            Align2::CENTER_CENTER,
            format!("{:.0}%", self.value),
            FontId::proportional(self.size * 0.26),
            self.apply_opacity(self.theme.text),
        );

        // Nombre de la métrica debajo del porcentaje.
        if self.show_label {
            painter.text(
                center + Vec2::new(0.0, self.size * 0.19),
                Align2::CENTER_CENTER,
                self.label,
                FontId::proportional(self.size * 0.10),
                self.apply_opacity(self.theme.text),
            );
        }
    }

    fn apply_opacity(&self, color: Color32) -> Color32 {
        Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            (color.a() as f32 * self.opacity) as u8,
        )
    }
}
