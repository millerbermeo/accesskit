//! Métricas del sistema expresadas como porcentaje (0.0–100.0).

pub mod cpu;
pub mod memory;

// Reservado para próximas versiones (aún sin implementar):
pub mod disk;
pub mod gpu;
pub mod network;

use sysinfo::System;

use crate::config::MetricKind;

/// Una métrica del sistema representable como porcentaje.
pub trait Metric {
    /// Nombre corto mostrado bajo el porcentaje ("CPU", "RAM", ...).
    fn label(&self) -> &'static str;

    /// Actualiza la métrica y devuelve su valor actual (0.0–100.0).
    fn update(&mut self, system: &mut System) -> f32;
}

/// Crea la implementación concreta de una métrica.
pub fn create_metric(kind: MetricKind) -> Box<dyn Metric> {
    match kind {
        MetricKind::Cpu => Box::new(cpu::CpuMetric::new()),
        MetricKind::Ram => Box::new(memory::MemoryMetric::new()),
    }
}
