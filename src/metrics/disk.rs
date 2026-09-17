//! Uso del disco principal (punto de montaje "/").

use std::path::Path;

use sysinfo::{Disks, System};

use super::Metric;

/// Porcentaje de espacio usado en el disco raíz ("/"), o en el primero
/// disponible si no se encuentra ese punto de montaje.
pub struct DiskMetric {
    disks: Disks,
}

impl DiskMetric {
    pub fn new() -> Self {
        Self {
            disks: Disks::new_with_refreshed_list(),
        }
    }
}

impl Metric for DiskMetric {
    fn label(&self) -> &'static str {
        "DISK"
    }

    fn update(&mut self, _system: &mut System) -> f32 {
        self.disks.refresh(false);
        let root = self
            .disks
            .list()
            .iter()
            .find(|disk| disk.mount_point() == Path::new("/"))
            .or_else(|| self.disks.list().first());
        let Some(disk) = root else {
            return 0.0;
        };
        let total = disk.total_space();
        if total == 0 {
            return 0.0;
        }
        let used = total.saturating_sub(disk.available_space());
        used as f32 / total as f32 * 100.0
    }
}
