//! Uso de GPU vía `nvidia-smi` (NVIDIA).

use std::process::Command;

use sysinfo::System;

use super::Metric;

/// Porcentaje de uso de la GPU, leído de `nvidia-smi`. Si el comando no está
/// disponible (sin GPU NVIDIA o sin driver), reporta 0% y avisa una sola vez.
pub struct GpuMetric {
    warned: bool,
}

impl GpuMetric {
    pub fn new() -> Self {
        Self { warned: false }
    }
}

impl Metric for GpuMetric {
    fn label(&self) -> &'static str {
        "GPU"
    }

    fn update(&mut self, _system: &mut System) -> f32 {
        let output = Command::new("nvidia-smi")
            .args(["--query-gpu=utilization.gpu", "--format=csv,noheader,nounits"])
            .output();
        match output {
            Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .and_then(|line| line.trim().parse::<f32>().ok())
                .unwrap_or(0.0),
            _ => {
                if !self.warned {
                    eprintln!("halo: no se pudo leer uso de GPU (¿falta nvidia-smi?)");
                    self.warned = true;
                }
                0.0
            }
        }
    }
}
