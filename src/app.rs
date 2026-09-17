//! Aplicación principal: ventanas de widgets, animación y refresco de métricas.

use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Context, Pos2, Vec2, ViewportBuilder, ViewportCommand, ViewportId, WindowLevel,
};
use sysinfo::System;

use crate::config::{MetricKind, Orientation, WidgetConfig};
use crate::metrics::{self, Metric};
use crate::shared::SharedState;
use crate::widgets::CircularProgress;

/// Intervalo entre lecturas de métricas.
const REFRESH_INTERVAL: Duration = Duration::from_secs(1);
/// Duración de la animación cuando cambia un porcentaje.
const ANIMATION_DURATION: Duration = Duration::from_millis(400);
/// Periodo de repintado mientras hay una animación activa (~60 FPS reales;
/// egui descuenta su `predicted_dt` de este delay).
const FRAME_DURATION: Duration = Duration::from_millis(20);
/// Suelo del repintado en reposo: delays menores caen por debajo del
/// `predicted_dt` de egui y generarían un bucle de repintados inmediatos.
const MIN_IDLE_DELAY: Duration = Duration::from_millis(50);
/// Espera mínima entre guardados de posiciones en disco.
const SAVE_DEBOUNCE: Duration = Duration::from_secs(1);
/// Separación de los bordes de la pantalla al ubicar el grupo por primera
/// vez (instalación nueva, sin posiciones guardadas todavía).
const SCREEN_MARGIN: f32 = 24.0;
/// Separación entre widgets consecutivos del grupo.
const GROUP_GAP: f32 = 12.0;

pub struct HaloApp {
    shared: Arc<SharedState>,
    system: System,
    widgets: Vec<WidgetState>,
    last_refresh: Instant,
    /// `true` mientras algún widget esté animando (lo decide el último frame).
    animating: bool,
    positions_dirty: bool,
    last_save: Instant,
    /// Orientación del frame anterior, para detectar el cambio y realinear
    /// justo en ese momento (no en cada frame).
    last_orientation: Orientation,
    /// `true` si algún widget tiene el botón del ratón apretado sobre él,
    /// calculado en el `ui()` de este frame para pacer el `logic()` del
    /// próximo. No puede leerse desde `logic()` directamente: `ctx.input()`
    /// ahí refleja el viewport ROOT (la ventana fantasma invisible), no las
    /// ventanas de los widgets, así que siempre daría `false`.
    dragging: bool,
    /// `true` hasta que el widget ancla reciba una posición inicial: en una
    /// instalación nueva (sin `config.toml` previo) se resuelve una sola vez
    /// contra el tamaño de pantalla; si ya había posiciones guardadas, queda
    /// en `false` desde el arranque y no hace nada.
    initial_placement_pending: bool,
}

/// Estado de un widget: su métrica, la animación y la posición de su ventana.
struct WidgetState {
    kind: MetricKind,
    metric: Box<dyn Metric>,
    /// Valor actualmente mostrado (animado).
    current: f32,
    /// Valor objetivo al que tiende la animación.
    target: f32,
    anim_from: f32,
    anim_start: Instant,
    /// Última posición conocida de la ventana (coordenadas de pantalla).
    position: Option<Pos2>,
    /// `true` si `position` fue cambiada por código (agrupado) y su ventana
    /// todavía no fue movida ahí; se reafirma una vez y se limpia.
    needs_apply: bool,
    /// Posición reportada por el gestor de ventanas en el frame anterior,
    /// mientras hay un arrastre nativo en curso (ver `ViewportCommand::StartDrag`
    /// en `show_widget`). Sirve para calcular cuánto se movió este frame y
    /// aplicar el mismo delta al resto del grupo.
    drag_prev_outer: Option<Pos2>,
}

impl WidgetState {
    fn new(kind: MetricKind, position: Option<[f32; 2]>) -> Self {
        Self {
            kind,
            metric: metrics::create_metric(kind),
            current: 0.0,
            target: 0.0,
            anim_from: 0.0,
            anim_start: Instant::now() - ANIMATION_DURATION,
            position: position.map(|[x, y]| Pos2::new(x, y)),
            needs_apply: false,
            drag_prev_outer: None,
        }
    }

    fn set_target(&mut self, target: f32) {
        self.anim_from = self.current;
        self.target = target;
        self.anim_start = Instant::now();
    }

    /// Avanza la animación (ease-out cúbico). Devuelve `true` si sigue activa.
    fn tick_animation(&mut self) -> bool {
        let t =
            (self.anim_start.elapsed().as_secs_f32() / ANIMATION_DURATION.as_secs_f32()).min(1.0);
        let eased = 1.0 - (1.0 - t).powi(3);
        self.current = self.anim_from + (self.target - self.anim_from) * eased;
        t < 1.0
    }
}

impl HaloApp {
    pub fn new(shared: Arc<SharedState>) -> Self {
        let mut system = System::new();
        // Primera lectura: sysinfo necesita dos refrescos para calcular el uso de CPU.
        system.refresh_cpu_usage();
        system.refresh_memory();

        let (widgets, last_orientation): (Vec<WidgetState>, Orientation) = {
            let config = shared.config.lock().unwrap();
            let widgets = config
                .enabled_kinds()
                .into_iter()
                .map(|kind| WidgetState::new(kind, config.metric(kind).position))
                .collect();
            (widgets, config.widget.orientation)
        };

        // Instalación nueva: ningún widget trae posición guardada todavía.
        // Si al menos uno ya tiene una (config existente), no se toca nada.
        let initial_placement_pending = widgets.iter().all(|w| w.position.is_none());

        Self {
            shared,
            system,
            widgets,
            last_refresh: Instant::now(),
            animating: true, // Animación de entrada desde 0%.
            positions_dirty: false,
            last_save: Instant::now(),
            last_orientation,
            dragging: false,
            initial_placement_pending,
        }
    }

    fn refresh_metrics(&mut self) {
        for widget in &mut self.widgets {
            let value = widget.metric.update(&mut self.system).clamp(0.0, 100.0);
            widget.set_target(value);
        }
    }

    /// Añade o quita widgets para que coincidan con las métricas habilitadas
    /// en la configuración compartida (el menú de bandeja puede haberla
    /// cambiado desde el último frame), y realinea el grupo si cambió el
    /// conjunto de widgets visibles o la orientación elegida.
    fn sync(&mut self) {
        let (enabled, orientation, size) = {
            let config = self.shared.config.lock().unwrap();
            (
                config.enabled_kinds(),
                config.widget.orientation,
                config.widget.size,
            )
        };

        let before_len = self.widgets.len();
        self.widgets.retain(|widget| enabled.contains(&widget.kind));
        let mut set_changed = before_len != self.widgets.len();

        for kind in enabled {
            if self.widgets.iter().any(|widget| widget.kind == kind) {
                continue;
            }
            let position = self.shared.config.lock().unwrap().metric(kind).position;
            self.widgets.push(WidgetState::new(kind, position));
            set_changed = true;
        }

        let orientation_changed = orientation != self.last_orientation;
        self.last_orientation = orientation;

        if set_changed || orientation_changed {
            self.align_group(size, orientation);
        }
    }

    /// Fija la posición inicial de un widget que todavía no tiene una
    /// guardada (primera vez que aparece), sin propagarla al resto del
    /// grupo: es solo para recordar dónde lo puso el gestor de ventanas.
    fn bootstrap_position(&mut self, index: usize, position: Pos2) {
        let widget = &mut self.widgets[index];
        if widget.position.is_some() {
            return;
        }
        widget.position = Some(position);
        let kind = widget.kind;
        self.shared.config.lock().unwrap().metric_mut(kind).position = Some([position.x, position.y]);
        self.positions_dirty = true;
    }

    /// Desplaza todos los widgets por el mismo delta (arrastre en grupo).
    /// Como las ventanas no tienen decoraciones, moverlas depende de este
    /// comando explícito — salvo la que originó el delta (`skip_index`):
    /// esa la está moviendo el gestor de ventanas de forma nativa (ver
    /// `show_widget`), así que mandarle también un `OuterPosition` pelearía
    /// con ese movimiento y la haría vibrar.
    fn apply_group_delta(&mut self, delta: Vec2, skip_index: Option<usize>) {
        if delta.length_sq() == 0.0 {
            return;
        }
        let mut config = self.shared.config.lock().unwrap();
        for (i, widget) in self.widgets.iter_mut().enumerate() {
            let Some(pos) = widget.position else {
                continue;
            };
            let moved = pos + delta;
            widget.position = Some(moved);
            if Some(i) != skip_index {
                widget.needs_apply = true;
            }
            config.metric_mut(widget.kind).position = Some([moved.x, moved.y]);
        }
        drop(config);
        self.positions_dirty = true;
    }

    /// Alinea todos los widgets en fila (horizontal) o columna (vertical)
    /// junto al primero, que no se mueve.
    fn align_group(&mut self, size: f32, orientation: Orientation) {
        let anchor = self
            .widgets
            .first()
            .and_then(|w| w.position)
            .unwrap_or(Pos2::new(100.0, 100.0));

        let mut config = self.shared.config.lock().unwrap();
        for (i, widget) in self.widgets.iter_mut().enumerate().skip(1) {
            let offset = i as f32 * (size + GROUP_GAP);
            let target = match orientation {
                Orientation::Horizontal => anchor + Vec2::new(offset, 0.0),
                Orientation::Vertical => anchor + Vec2::new(0.0, offset),
            };
            widget.position = Some(target);
            widget.needs_apply = true;
            config.metric_mut(widget.kind).position = Some([target.x, target.y]);
        }
        drop(config);
        self.positions_dirty = true;
    }

    /// Ubica el grupo en la esquina inferior derecha de la pantalla, con
    /// margen de los bordes, en vez de dejar que el gestor de ventanas
    /// elija dónde poner cada ventana nueva (normalmente el centro o la
    /// esquina superior izquierda). Solo actúa una vez, en la primera
    /// instalación; `align_group` ya se encarga de acomodar al resto del
    /// grupo junto al ancla.
    fn place_group_bottom_right(&mut self, ctx: &Context, size: f32, orientation: Orientation) {
        if !self.initial_placement_pending {
            return;
        }
        let Some(monitor_size) = ctx.input(|i| i.viewport().monitor_size) else {
            return; // Sin info de monitor todavía; se reintenta el próximo frame.
        };
        if monitor_size.x <= 1.0 || monitor_size.y <= 1.0 {
            return;
        }
        if self.widgets.is_empty() {
            self.initial_placement_pending = false;
            return;
        }

        // El ancla es el primero del grupo y `align_group` extiende al
        // resto hacia la derecha (horizontal) o hacia abajo (vertical); para
        // que el GRUPO ENTERO termine pegado a la esquina inferior derecha
        // (no solo el ancla, que dejaría a los demás fuera de pantalla), se
        // retrocede el ancla el ancho total del grupo menos un widget.
        let span = (self.widgets.len() - 1) as f32 * (size + GROUP_GAP);
        let (anchor_x, anchor_y) = match orientation {
            Orientation::Horizontal => (
                (monitor_size.x - size - SCREEN_MARGIN - span).max(SCREEN_MARGIN),
                (monitor_size.y - size - SCREEN_MARGIN).max(SCREEN_MARGIN),
            ),
            Orientation::Vertical => (
                (monitor_size.x - size - SCREEN_MARGIN).max(SCREEN_MARGIN),
                (monitor_size.y - size - SCREEN_MARGIN - span).max(SCREEN_MARGIN),
            ),
        };
        let position = Pos2::new(anchor_x, anchor_y);
        let kind = self.widgets[0].kind;
        self.widgets[0].position = Some(position);
        self.shared.config.lock().unwrap().metric_mut(kind).position = Some([position.x, position.y]);
        self.align_group(size, orientation);
        self.positions_dirty = true;
        self.initial_placement_pending = false;
    }

    fn maybe_save_positions(&mut self) {
        if !self.positions_dirty || self.last_save.elapsed() < SAVE_DEBOUNCE {
            return;
        }
        self.shared.save();
        self.positions_dirty = false;
        self.last_save = Instant::now();
    }

    fn viewport_builder(
        widget: WidgetConfig,
        title: &str,
        position: Option<Pos2>,
    ) -> ViewportBuilder {
        let mut builder = ViewportBuilder::default()
            .with_title(title)
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
        if let Some(position) = position {
            builder = builder.with_position([position.x, position.y]);
        }
        builder
    }

    /// Pinta el widget `index` en su propia ventana. Devuelve si está
    /// animando y, si el usuario lo arrastró este frame, cuánto se movió
    /// (para desplazar al resto del grupo la misma cantidad).
    fn show_widget(&mut self, ctx: &Context, index: usize) -> (bool, Option<Vec2>, bool) {
        let kind = self.widgets[index].kind;
        let (widget_cfg, theme, progress_color) = {
            let config = self.shared.config.lock().unwrap();
            (config.widget, config.theme.resolve(), config.metric_color(kind))
        };
        let label = self.widgets[index].metric.label();
        let position = self.widgets[index].position;
        let builder = Self::viewport_builder(widget_cfg, label, position);

        let widget = &mut self.widgets[index];
        let mut animating = false;
        let mut drag_delta = None;
        let mut outer_position = None;
        let mut pointer_down = false;
        ctx.show_viewport_immediate(
            ViewportId::from_hash_of(widget.kind.id()),
            builder,
            |viewport_ui, _class| {
                // Algunos gestores de ventanas sueltan el nivel "always on
                // top" al hacer click sobre la ventana, así que se reafirma
                // en cada frame.
                viewport_ui
                    .ctx()
                    .send_viewport_cmd(ViewportCommand::WindowLevel(if widget_cfg.always_on_top {
                        WindowLevel::AlwaysOnTop
                    } else {
                        WindowLevel::Normal
                    }));

                // Reposicionamiento forzado (alinear grupo / seguir al
                // arrastre de otro widget); se manda una sola vez, no en
                // cada frame.
                if widget.needs_apply {
                    if let Some(pos) = widget.position {
                        viewport_ui.ctx().send_viewport_cmd(ViewportCommand::OuterPosition(pos));
                    }
                    widget.needs_apply = false;
                }

                animating = widget.tick_animation();

                let response = CircularProgress::new(
                    widget.current,
                    label,
                    &theme,
                    progress_color,
                    widget_cfg.size,
                    widget_cfg.opacity,
                    widget_cfg.show_label,
                )
                .show(viewport_ui);

                outer_position = viewport_ui
                    .ctx()
                    .input(|input| input.viewport().outer_rect)
                    .map(|rect| rect.min);

                // Al empezar a arrastrar, se le pide al gestor de ventanas
                // que mueva ESTA ventana de forma nativa (suave, sin
                // depender de nuestro ritmo de repintado ni de que los
                // eventos de ratón nos lleguen a tiempo). El resto del grupo
                // no puede moverse igual (el gestor tiene el puntero
                // agarrado para SU propio arrastre), así que las hermanas
                // siguen a mano (`apply_group_delta`) usando el delta real
                // que el gestor va reportando en `outer_rect` cuadro a
                // cuadro — eso sí sigue llegando durante el arrastre nativo,
                // a diferencia de los eventos de puntero.
                if response.drag_started() {
                    viewport_ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
                    widget.drag_prev_outer = outer_position.or(widget.position);
                }

                if response.dragged()
                    && let (Some(prev), Some(current)) = (widget.drag_prev_outer, outer_position)
                {
                    let delta = current - prev;
                    if delta != Vec2::ZERO {
                        drag_delta = Some(delta);
                    }
                    widget.drag_prev_outer = outer_position;
                }

                if response.drag_stopped() {
                    widget.drag_prev_outer = None;
                }

                // Estado del ratón *de este viewport concreto*: cada ventana
                // de widget solo recibe eventos cuando el puntero está sobre
                // ella, así que esto sí refleja si se está arrastrando (a
                // diferencia de leerlo desde el ROOT en `logic()`).
                pointer_down = viewport_ui.ctx().input(|input| input.pointer.any_down());
            },
        );

        // Solo para la primera aparición (sin posición guardada todavía):
        // recordar dónde lo puso el gestor de ventanas por defecto.
        if self.widgets[index].position.is_none()
            && let Some(pos) = outer_position
        {
            self.bootstrap_position(index, pos);
        }

        (animating, drag_delta, pointer_down)
    }
}

impl eframe::App for HaloApp {
    /// Lógica sin pintado: refresco de métricas, guardado y ritmo de repintado.
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.shared.set_context(ctx.clone());

        if self.last_refresh.elapsed() >= REFRESH_INTERVAL {
            self.refresh_metrics();
            self.last_refresh = Instant::now();
            self.animating = true; // Los nuevos valores arrancan una animación.
        }

        self.maybe_save_positions();

        // Repintado bajo demanda: ~60 FPS solo mientras anima; si no, se duerme
        // hasta el próximo refresco de métricas para no consumir CPU. Con el
        // botón del ratón pulsado sobre algún widget forzamos el ritmo rápido
        // también: las ventanas de widget son viewports inmediatos, así que
        // solo se repintan cuando repinta el ROOT (ver `HaloApp::dragging`);
        // sin esto, un arrastre que empieza fuera de una animación de
        // métrica queda a merced del repintado lento (hasta 1s) y se siente
        // trabado o "no agarra".
        if self.animating || self.dragging {
            ctx.request_repaint_after(FRAME_DURATION);
        } else {
            let remaining = REFRESH_INTERVAL.saturating_sub(self.last_refresh.elapsed());
            ctx.request_repaint_after(remaining.clamp(MIN_IDLE_DELAY, REFRESH_INTERVAL));
        }
    }

    /// Ventana raíz: no se pinta nada en ella (es una ventana fantasma, ver
    /// `main.rs`). Cada métrica vive en su propia ventana secundaria, creada
    /// y destruida dinámicamente según lo que esté habilitado en la config.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.sync();

        let ctx = ui.ctx().clone();

        if self.initial_placement_pending {
            let (size, orientation) = {
                let config = self.shared.config.lock().unwrap();
                (config.widget.size, config.widget.orientation)
            };
            self.place_group_bottom_right(&ctx, size, orientation);
        }

        let mut animating = false;
        let mut group_delta = None;
        let mut dragging = false;
        for index in 0..self.widgets.len() {
            let (widget_animating, delta, pointer_down) = self.show_widget(&ctx, index);
            animating |= widget_animating;
            dragging |= pointer_down;
            if let Some(delta) = delta {
                group_delta = Some((index, delta));
            }
        }
        if let Some((origin_index, delta)) = group_delta {
            self.apply_group_delta(delta, Some(origin_index));
        }
        self.animating = animating;
        self.dragging = dragging;
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4] // Ventana totalmente transparente.
    }
}
