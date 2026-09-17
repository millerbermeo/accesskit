//! Configuración de halo cargada y guardada en TOML.

use std::fs;
use std::path::Path;

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

/// Color de progreso por defecto (rojo).
pub const DEFAULT_PROGRESS_COLOR: &str = "#E63027";
pub const DEFAULT_BACKGROUND_COLOR: &str = "#202020";
pub const DEFAULT_TEXT_COLOR: &str = "#FFFFFF";
pub const DEFAULT_RING_BACKGROUND_COLOR: &str = "#3A3A3A";

/// Configuración raíz del archivo `config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: ThemeConfig,
    pub widget: WidgetConfig,
    pub cpu: MetricConfig,
    pub ram: MetricConfig,
    pub gpu: MetricConfig,
    pub disk: MetricConfig,
    pub network: MetricConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: ThemeConfig::default(),
            widget: WidgetConfig::default(),
            cpu: MetricConfig {
                enabled: true,
                position: None,
                color: None,
            },
            ram: MetricConfig {
                enabled: true,
                position: None,
                color: None,
            },
            gpu: MetricConfig {
                enabled: true,
                position: None,
                color: None,
            },
            disk: MetricConfig {
                enabled: true,
                position: None,
                color: None,
            },
            network: MetricConfig::default(),
        }
    }
}

impl Config {
    /// Configuración de una métrica concreta.
    pub fn metric(&self, kind: MetricKind) -> &MetricConfig {
        match kind {
            MetricKind::Cpu => &self.cpu,
            MetricKind::Ram => &self.ram,
            MetricKind::Disk => &self.disk,
            MetricKind::Gpu => &self.gpu,
        }
    }

    /// Configuración mutable de una métrica concreta.
    pub fn metric_mut(&mut self, kind: MetricKind) -> &mut MetricConfig {
        match kind {
            MetricKind::Cpu => &mut self.cpu,
            MetricKind::Ram => &mut self.ram,
            MetricKind::Disk => &mut self.disk,
            MetricKind::Gpu => &mut self.gpu,
        }
    }

    /// Métricas habilitadas, en orden fijo.
    pub fn enabled_kinds(&self) -> Vec<MetricKind> {
        [MetricKind::Cpu, MetricKind::Ram, MetricKind::Disk, MetricKind::Gpu]
            .into_iter()
            .filter(|kind| self.metric(*kind).enabled)
            .collect()
    }

    /// Color del arco de progreso de una métrica: el suyo propio si tiene uno
    /// configurado, o si no el color de progreso del tema.
    pub fn metric_color(&self, kind: MetricKind) -> Color32 {
        self.metric(kind)
            .color
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or_else(|| self.theme.resolve().progress)
    }

    /// Carga la configuración desde `path`; si no existe, lo crea con los
    /// valores por defecto.
    pub fn load_or_create(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(contents) => toml::from_str(&contents).unwrap_or_else(|err| {
                eprintln!(
                    "halo: {} no es válido ({err}); se usan los valores por defecto",
                    path.display()
                );
                Self::default()
            }),
            Err(_) => {
                let config = Self::default();
                if let Err(err) = config.save(path) {
                    eprintln!("halo: no se pudo crear {}: {err}", path.display());
                }
                config
            }
        }
    }

    /// Guarda la configuración en `path`.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        fs::write(path, contents)
    }
}

/// Apariencia configurable (colores en texto y grosor del anillo).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub progress_color: String,
    pub background_color: String,
    pub text_color: String,
    pub ring_background_color: String,
    pub ring_width: f32,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            progress_color: DEFAULT_PROGRESS_COLOR.to_owned(),
            background_color: DEFAULT_BACKGROUND_COLOR.to_owned(),
            text_color: DEFAULT_TEXT_COLOR.to_owned(),
            ring_background_color: DEFAULT_RING_BACKGROUND_COLOR.to_owned(),
            ring_width: 3.6,
        }
    }
}

impl ThemeConfig {
    /// Convierte los colores de texto en colores listos para pintar.
    /// Si un valor no es válido se usa el color por defecto.
    pub fn resolve(&self) -> Theme {
        let fallback = |default: &str| parse_hex_color(default).unwrap();
        Theme {
            progress: parse_hex_color(&self.progress_color)
                .unwrap_or_else(|| fallback(DEFAULT_PROGRESS_COLOR)),
            background: parse_hex_color(&self.background_color)
                .unwrap_or_else(|| fallback(DEFAULT_BACKGROUND_COLOR)),
            text: parse_hex_color(&self.text_color)
                .unwrap_or_else(|| fallback(DEFAULT_TEXT_COLOR)),
            ring_background: parse_hex_color(&self.ring_background_color)
                .unwrap_or_else(|| fallback(DEFAULT_RING_BACKGROUND_COLOR)),
            ring_width: self.ring_width.max(1.0),
        }
    }
}

/// Tema con los colores ya parseados, listo para pintar.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub progress: Color32,
    pub background: Color32,
    pub text: Color32,
    pub ring_background: Color32,
    pub ring_width: f32,
}

/// Opciones comunes de todos los widgets.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct WidgetConfig {
    pub size: f32,
    pub opacity: f32,
    pub always_on_top: bool,
    pub show_label: bool,
    /// Los widgets siempre se mueven juntos, en fila u en columna según esto.
    pub orientation: Orientation,
}

impl Default for WidgetConfig {
    fn default() -> Self {
        Self {
            size: 57.96, // 50.4 + 15%
            opacity: 0.90,
            always_on_top: true,
            show_label: true,
            orientation: Orientation::Horizontal,
        }
    }
}

/// Disposición del grupo de widgets: en fila (uno al lado del otro) o en
/// columna (uno debajo del otro).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    Horizontal,
    Vertical,
}

impl Default for Orientation {
    fn default() -> Self {
        Self::Horizontal
    }
}

/// Activación, posición y color de una métrica concreta.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MetricConfig {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<[f32; 2]>,
    /// Color del arco de progreso, en "#RRGGBB". `None` usa `theme.progress_color`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// Identificador tipado de cada métrica soportada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricKind {
    Cpu,
    Ram,
    Disk,
    Gpu,
}

impl MetricKind {
    /// Identificador estable usado como id de ventana.
    pub fn id(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Ram => "ram",
            Self::Disk => "disk",
            Self::Gpu => "gpu",
        }
    }
}

/// Parsea un color en formato "#RRGGBB" o "#RRGGBBAA".
pub fn parse_hex_color(text: &str) -> Option<Color32> {
    let hex = text.trim().strip_prefix('#')?;
    let channel = |range: std::ops::Range<usize>| u8::from_str_radix(hex.get(range)?, 16).ok();
    match hex.len() {
        6 => Some(Color32::from_rgb(
            channel(0..2)?,
            channel(2..4)?,
            channel(4..6)?,
        )),
        8 => Some(Color32::from_rgba_unmultiplied(
            channel(0..2)?,
            channel(2..4)?,
            channel(4..6)?,
            channel(6..8)?,
        )),
        _ => None,
    }
}
