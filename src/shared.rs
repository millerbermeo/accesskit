//! Estado compartido entre el hilo de la app (egui/eframe) y el hilo de la
//! bandeja del sistema (GTK): la configuración en memoria y el contexto de
//! egui, para que cambios hechos desde el menú de bandeja (mostrar/ocultar
//! una métrica, cambiarle el color) se reflejen en caliente.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui::{Context, ViewportId};

use crate::config::Config;

pub struct SharedState {
    pub config: Mutex<Config>,
    config_path: PathBuf,
    /// Se rellena en el primer frame de la app; permite despertarla desde
    /// el hilo de la bandeja tras un cambio.
    ctx: Mutex<Option<Context>>,
}

impl SharedState {
    pub fn new(config: Config, config_path: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            config: Mutex::new(config),
            config_path,
            ctx: Mutex::new(None),
        })
    }

    /// Registra (o reemplaza) el contexto de egui usado para pedir repintados.
    pub fn set_context(&self, ctx: Context) {
        *self.ctx.lock().unwrap() = Some(ctx);
    }

    /// Guarda la configuración actual en disco.
    pub fn save(&self) {
        let config = self.config.lock().unwrap();
        if let Err(err) = config.save(&self.config_path) {
            eprintln!("halo: no se pudo guardar {}: {err}", self.config_path.display());
        }
    }

    /// Pide un repintado inmediato (por ejemplo, tras un cambio desde la
    /// bandeja). Siempre apunta al viewport ROOT explícitamente: este método
    /// se llama desde el hilo de la bandeja (GTK), distinto del hilo de
    /// egui/eframe, y `ctx.request_repaint()` sin especificar viewport lee
    /// una pila de viewports compartida entre hilos que puede estar
    /// apuntando a otro viewport justo en ese instante (carrera). ROOT es
    /// además el único viewport que realmente dispara el repintado: los
    /// widgets son viewports inmediatos, anidados dentro del `ui()` de ROOT.
    pub fn request_repaint(&self) {
        if let Some(ctx) = self.ctx.lock().unwrap().as_ref() {
            ctx.request_repaint_of(ViewportId::ROOT);
        }
    }
}
