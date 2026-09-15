//! Uso global de CPU.

use sysinfo::System;

use super::Metric;

/// Porcentaje de uso global de CPU (media de todos los núcleos).
pub struct CpuMetric;

impl CpuMetric {
    pub fn new() -> Self {
        Self
    }
}

impl Metric for CpuMetric {
    fn label(&self) -> &'static str {
        "CPU"
    }

    fn update(&mut self, system: &mut System) -> f32 {
        system.refresh_cpu_usage();
        system.global_cpu_usage()
    }
}
