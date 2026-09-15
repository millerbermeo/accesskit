//! Uso de memoria RAM.

use sysinfo::System;

use super::Metric;

/// Porcentaje de RAM usada respecto al total.
pub struct MemoryMetric;

impl MemoryMetric {
    pub fn new() -> Self {
        Self
    }
}

impl Metric for MemoryMetric {
    fn label(&self) -> &'static str {
        "RAM"
    }

    fn update(&mut self, system: &mut System) -> f32 {
        system.refresh_memory();
        let total = system.total_memory();
        if total == 0 {
            return 0.0;
        }
        system.used_memory() as f32 / total as f32 * 100.0
    }
}
