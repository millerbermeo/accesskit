//! Uso de GPU vía `nvidia-smi` (NVIDIA).

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use sysinfo::System;

use super::Metric;

/// Cada cuánto se relee `nvidia-smi` en el hilo de fondo.
const POLL_INTERVAL: Duration = Duration::from_secs(1);
/// Paso de espera entre chequeos de `running`, para que el hilo salga rápido
/// al deshabilitar la métrica en vez de tardar hasta `POLL_INTERVAL`.
const STOP_CHECK_STEP: Duration = Duration::from_millis(100);

/// Porcentaje de uso de la GPU, leído de `nvidia-smi` en un hilo de fondo
/// dedicado: el proceso puede tardar bastante en spawnear (GPU despertando
/// de un estado de bajo consumo, etc.), y hacerlo en el hilo de egui
/// congelaría toda la UI —incluido cualquier arrastre en curso— cada vez
/// que se refresca. `update()` solo lee el último valor cacheado.
pub struct GpuMetric {
    value: Arc<Mutex<f32>>,
    running: Arc<AtomicBool>,
}

impl GpuMetric {
    pub fn new() -> Self {
        let value = Arc::new(Mutex::new(0.0));
        let running = Arc::new(AtomicBool::new(true));

        let bg_value = Arc::clone(&value);
        let bg_running = Arc::clone(&running);
        thread::spawn(move || {
            let mut warned = false;
            while bg_running.load(Ordering::Relaxed) {
                let output = Command::new("nvidia-smi")
                    .args(["--query-gpu=utilization.gpu", "--format=csv,noheader,nounits"])
                    .output();
                match output {
                    Ok(output) if output.status.success() => {
                        if let Some(v) = String::from_utf8_lossy(&output.stdout)
                            .lines()
                            .next()
                            .and_then(|line| line.trim().parse::<f32>().ok())
                        {
                            *bg_value.lock().unwrap() = v;
                        }
                    }
                    _ => {
                        if !warned {
                            eprintln!("halo: no se pudo leer uso de GPU (¿falta nvidia-smi?)");
                            warned = true;
                        }
                    }
                }

                let mut waited = Duration::ZERO;
                while waited < POLL_INTERVAL && bg_running.load(Ordering::Relaxed) {
                    thread::sleep(STOP_CHECK_STEP);
                    waited += STOP_CHECK_STEP;
                }
            }
        });

        Self { value, running }
    }
}

impl Drop for GpuMetric {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

impl Metric for GpuMetric {
    fn label(&self) -> &'static str {
        "GPU"
    }

    fn update(&mut self, _system: &mut System) -> f32 {
        *self.value.lock().unwrap()
    }
}
