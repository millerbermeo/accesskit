# halo

Widgets circulares y transparentes para monitorizar el sistema en Linux (X11 y Wayland). Cada métrica habilitada (CPU, RAM, y próximamente GPU, disco y red) se muestra como un anillo de progreso flotante, sin bordes ni barra de título, que puede arrastrarse libremente sobre el escritorio.

## ¿Para qué sirve?

Es un monitor de recursos minimalista pensado para tenerlo siempre visible encima de otras ventanas (`always_on_top`), sin ocupar espacio en la barra de tareas. En vez de un panel con texto, muestra un anillo animado que se llena según el porcentaje de uso de cada métrica.

## ¿Cómo funciona?

- **Motor gráfico**: [`egui`](https://github.com/emilk/egui) + [`eframe`](https://github.com/emilk/egui) con backend `glow` (OpenGL), elegido por soportar transparencia real en X11/Wayland con tiempos de compilación bajos.
- **Métricas**: obtenidas con [`sysinfo`](https://crates.io/crates/sysinfo), refrescadas cada segundo (`REFRESH_INTERVAL`).
- **Ventanas**: cada métrica habilitada vive en su propia ventana sin decoraciones, transparente y sin foco de teclado (`with_active(false)`), para no interferir con el trabajo normal.
- **Animación**: al cambiar el valor de una métrica, el anillo se anima con un *ease-out* cúbico durante 400 ms en vez de saltar bruscamente.
- **Posiciones**: al arrastrar un widget, su nueva posición se guarda automáticamente (con un debounce de 1s) en el archivo de configuración.
- **Repintado eficiente**: mientras no hay animación en curso, la app "duerme" y solo despierta para el siguiente refresco de métricas — no consume CPU de forma constante.
- **Icono de bandeja**: al arrancar aparece un icono en la bandeja del sistema (área de notificaciones). Un click despliega un menú con:
  - **Iniciar con el sistema** (checkbox): activa/desactiva el arranque automático al iniciar sesión, escribiendo/borrando `~/.config/autostart/halo.desktop` (estándar XDG Autostart). El estado se lee de ese archivo al abrir el menú, así que persiste entre reinicios sin configuración adicional.
  - **Salir**: cierra la aplicación (las ventanas no tienen barra ni botón de cerrar).

### Estructura del código

| Archivo | Responsabilidad |
|---|---|
| `src/main.rs` | Arranque, ventana raíz, resolución de la ruta de configuración |
| `src/config.rs` | Carga/guardado de `config.toml`, colores, tema |
| `src/app.rs` | Lógica de la app: refresco, animación, arrastre, guardado de posiciones |
| `src/widgets/` | Dibujo del anillo de progreso (`CircularProgress`) |
| `src/metrics/` | Lectura de cada métrica (`cpu.rs`, `memory.rs`; `gpu.rs`, `disk.rs`, `network.rs` reservados para el futuro) |

### Configuración

Al primer arranque se crea automáticamente en `~/.config/halo/config.toml` (o `$XDG_CONFIG_HOME/halo/config.toml` si esa variable está definida). Ejemplo:

```toml
[widget]
size = 140.0
opacity = 0.9
always_on_top = true
show_label = true

[cpu]
enabled = true
position = [100.0, 100.0]

[ram]
enabled = true
position = [260.0, 100.0]
```

Los cambios en el archivo se aplican al reiniciar la aplicación. Las posiciones se actualizan solas al arrastrar los widgets.

## Instalación

### Rápida (recomendada)

Descarga el instalador y ejecútalo con `bash`:

```bash
curl -fsSL https://raw.githubusercontent.com/millerbermeo/accesskit/main/install.sh | bash
```

Esto hace, en orden:

1. Comprueba si tienes `git` y `cargo` (Rust). Si falta Rust, lo instala con [`rustup`](https://rustup.rs).
2. Descarga el código fuente en `~/.local/share/halo/src` (o actualiza si ya existe: `git pull`).
3. Compila en modo `release` con `cargo build --release`.
4. Copia el binario resultante a `~/.local/bin/halo`.

Al terminar, si `~/.local/bin` no está en tu `PATH`, el script te dice exactamente qué línea agregar a tu `~/.bashrc`/`~/.zshrc`.

Para ejecutarlo:

```bash
halo
```

Volver a correr el mismo `curl | bash` en el futuro descarga los últimos cambios del repositorio, recompila e instala la versión actualizada.

### Manual

```bash
git clone https://github.com/millerbermeo/accesskit.git
cd accesskit
cargo build --release
./target/release/halo
```

### Requisitos

- Rust (se instala solo si falta, vía `rustup`).
- Linux con X11 o Wayland.
- `git`.
- Libs de desarrollo para el icono de bandeja: `libgtk-3-dev`, `libxdo-dev`, `libayatana-appindicator3-dev` (o `libappindicator3-dev`). `install.sh` las instala automáticamente en distros basadas en Debian/Ubuntu (pide `sudo`); en otras distros hay que instalarlas a mano antes de compilar.
- Para que el icono de bandeja se vea en **GNOME** hace falta la extensión "AppIndicator and KStatusNotifierItem Support" (en Ubuntu viene preinstalada y activa por defecto).

## Desinstalar

```bash
rm ~/.local/bin/halo
rm -rf ~/.local/share/halo
rm -rf ~/.config/halo
```
