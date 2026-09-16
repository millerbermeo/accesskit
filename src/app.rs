//! Aplicación principal: ventanas de widgets, animación y refresco de métricas.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Context, Pos2, ViewportBuilder, ViewportCommand, ViewportId, WindowLevel,
};
use sysinfo::System;

use crate::config::{Config, MetricKind, Theme, WidgetConfig};
use crate::metrics::{self, Metric};
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

pub struct HaloApp {
    config: Config,
    config_path: PathBuf,
    theme: Theme,
    system: System,
    widgets: Vec<WidgetState>,
    last_refresh: Instant,
    /// `true` mientras algún widget esté animando (lo decide el último frame).
    animating: bool,
    positions_dirty: bool,
    last_save: Instant,
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
    pub fn new(config: Config, config_path: PathBuf) -> Self {
        let mut system = System::new();
        // Primera lectura: sysinfo necesita dos refrescos para calcular el uso de CPU.
        system.refresh_cpu_usage();
        system.refresh_memory();

        let widgets = config
            .enabled_kinds()
            .into_iter()
            .map(|kind| WidgetState::new(kind, config.metric(kind).position))
            .collect();

        Self {
            theme: config.theme.resolve(),
            config,
            config_path,
            system,
            widgets,
            last_refresh: Instant::now(),
            animating: true, // Animación de entrada desde 0%.
            positions_dirty: false,
            last_save: Instant::now(),
        }
    }

    fn refresh_metrics(&mut self) {
        for widget in &mut self.widgets {
            let value = widget.metric.update(&mut self.system).clamp(0.0, 100.0);
            widget.set_target(value);
        }
    }

    /// Registra la posición de una ventana y marca la config como modificada.
    fn update_position(&mut self, index: usize, position: Pos2) {
        let widget = &mut self.widgets[index];
        if widget.position.is_some_and(|p| p.distance(position) < 1.0) {
            return;
        }
        widget.position = Some(position);
        self.config.metric_mut(widget.kind).position = Some([position.x, position.y]);
        self.positions_dirty = true;
    }

    fn maybe_save_positions(&mut self) {
        if !self.positions_dirty || self.last_save.elapsed() < SAVE_DEBOUNCE {
            return;
        }
        match self.config.save(&self.config_path) {
            Ok(()) => self.positions_dirty = false,
            Err(err) => eprintln!("halo: no se pudo guardar {}: {err}", self.config_path.display()),
        }
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

    /// Pinta el widget `index` en su propia ventana. Devuelve `true` si está animando.
    fn show_widget(&mut self, ctx: &Context, index: usize) -> bool {
        let theme = self.theme;
        let widget_cfg = self.config.widget;
        let label = self.widgets[index].metric.label();
        let position = self.widgets[index].position;
        let builder = Self::viewport_builder(widget_cfg, label, position);

        let widget = &mut self.widgets[index];
        let mut animating = false;
        let mut new_position = None;
        ctx.show_viewport_immediate(
            ViewportId::from_hash_of(widget.kind.id()),
            builder,
            |viewport_ui, _class| {
                animating = widget.tick_animation();

                let response = CircularProgress::new(
                    widget.current,
                    label,
                    &theme,
                    widget_cfg.size,
                    widget_cfg.opacity,
                    widget_cfg.show_label,
                )
                .show(viewport_ui);
                if response.drag_started() {
                    viewport_ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
                }

                new_position = viewport_ui
                    .ctx()
                    .input(|input| input.viewport().outer_rect)
                    .map(|rect| rect.min);
            },
        );

        if let Some(position) = new_position {
            self.update_position(index, position);
        }
        animating
    }
}

impl eframe::App for HaloApp {
    /// Lógica sin pintado: refresco de métricas, guardado y ritmo de repintado.
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if self.last_refresh.elapsed() >= REFRESH_INTERVAL {
            self.refresh_metrics();
            self.last_refresh = Instant::now();
            self.animating = true; // Los nuevos valores arrancan una animación.
        }

        self.maybe_save_positions();

        // Repintado bajo demanda: ~60 FPS solo mientras anima; si no, se duerme
        // hasta el próximo refresco de métricas para no consumir CPU.
        if self.animating {
            ctx.request_repaint_after(FRAME_DURATION);
        } else {
            let remaining = REFRESH_INTERVAL.saturating_sub(self.last_refresh.elapsed());
            ctx.request_repaint_after(remaining.clamp(MIN_IDLE_DELAY, REFRESH_INTERVAL));
        }
    }

    /// Pintado del widget raíz y apertura de las ventanas de los demás.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let theme = self.theme;
        let widget_cfg = self.config.widget;
        let mut animating = false;

        // El primer widget habilitado usa la ventana raíz. A diferencia de las
        // ventanas secundarias (cuyo ViewportBuilder se reaplica en cada frame
        // vía show_viewport_immediate), la raíz solo recibe su ViewportBuilder
        // una vez al arrancar, y algunos gestores de ventanas (p. ej. Mutter en
        // X11) descartan el nivel "always on top" pedido antes del primer mapeo.
        // Se reafirma en cada frame para que quede fijado igual que las demás.
        ui.ctx()
            .send_viewport_cmd(ViewportCommand::WindowLevel(if widget_cfg.always_on_top {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            }));
        {
            let widget = &mut self.widgets[0];
            animating |= widget.tick_animation();
            let response = CircularProgress::new(
                widget.current,
                widget.metric.label(),
                &theme,
                widget_cfg.size,
                widget_cfg.opacity,
                widget_cfg.show_label,
            )
            .show(ui);
            if response.drag_started() {
                ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
            }
        }
        if let Some(position) = ui
            .ctx()
            .input(|input| input.viewport().outer_rect)
            .map(|rect| rect.min)
        {
            self.update_position(0, position);
        }

        // El resto de widgets viven en sus propias ventanas.
        let ctx = ui.ctx().clone();
        for index in 1..self.widgets.len() {
            animating |= self.show_widget(&ctx, index);
        }

        self.animating = animating;
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4] // Ventana totalmente transparente.
    }
}
