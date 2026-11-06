# Prompts para instalar y ejecutar por comandos

Bloques listos para copiar, en el orden en que se usan. Cada uno dice qué hace y cómo saber que salió bien. Todo es para **Linux**; en otros sistemas ver [Windows](../docs/windows.md) y [OpenCode](../docs/opencode.md).

## Camino rápido

```sh
git clone https://github.com/RamonsDka/gs3-sidebar.git
cd gs3-sidebar
./install.sh --dry-run     # mira qué haría, sin tocar nada
./install.sh               # instala de verdad
```

Esperado: `install.sh` termina con el doctor en verde y el código de salida es `0`.

## 1 · Comprobar requisitos

```sh
scripts/check-deps.sh
```

Obligatorios: `herdr`, `jq`, `cargo` y un compilador C. Si falta Rust:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Para `jq` en Debian/Ubuntu/Pop!_OS: `sudo apt install jq build-essential`.

## 2 · Instalar

| Quiero… | Comando |
| --- | --- |
| Instalación normal | `./install.sh` |
| Sin preguntas | `./install.sh -y` |
| Solo la barra, sin extensiones de Pi | `./install.sh --no-extensions` |
| Usar un binario ya compilado | `./install.sh --binary /ruta/a/gs3-hud` |
| Instalar en otra carpeta | `./install.sh --prefix "$HOME/apps/gs3"` |
| Otra carpeta de agente de Pi | `./install.sh --agent-dir /ruta/al/agente` |
| Ver qué haría | `./install.sh --dry-run` |

Si `~/.local/bin` no está en tu `PATH`:

```sh
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc && . ~/.bashrc
command -v gs3 gs3-hud
```

Esperado: dos rutas dentro de `~/.local/bin`.

## 3 · Paso a paso, sin `install.sh`

Cada script se puede correr solo:

```sh
scripts/check-deps.sh
scripts/build-hud.sh          # compila e instala gs3-hud
scripts/install-launcher.sh   # instala gs3 y crea ~/.config/gs3/
scripts/install-extensions.sh # copia las extensiones a ~/.pi/agent/extensions/
scripts/doctor.sh
```

Compilar a mano, sin instalar:

```sh
cd gs3/hud
CARGO_TARGET_DIR=../../target cargo build --release
```

## 4 · Ejecutar

```sh
herdr                       # abrí herdr y una terminal dentro
herdr integration install pi   # una sola vez: informa a herdr de la sesión de Pi
gs3                         # divide el panel: Pi a la izquierda, la barra a la derecha
```

Dentro de Pi, `/reload` carga las extensiones. Atajos de la barra: `1`–`6` pestañas, `r` refresca, `q` sale.

Otras formas de arrancar:

| Situación | Comando |
| --- | --- |
| Usar otro agente en el panel grande | `GS3_SHELL=pi gs3` |
| Más ancho para la barra | `GS3_RATIO=0.65 gs3` |
| Barra a mano sobre un panel existente | `herdr pane list` y luego `gs3-hud --pane <pane_id>` |
| Ver todas las pestañas como texto | `gs3-hud --pane <pane_id> --once` |

## 5 · Verificar

```sh
scripts/doctor.sh     # exit 0 = sano
gs3-hud --help
```

Dentro de herdr, `doctor.sh` también renderiza la barra contra tu panel real.

## 6 · Tests (opcional, para contribuir)

```sh
CARGO_TARGET_DIR="$PWD/target" cargo test --release --manifest-path gs3/hud/Cargo.toml

# Extensiones: necesitan la carpeta del paquete de Pi
export PI_PKG_ROOT="$(dirname "$(dirname "$(readlink -f "$(command -v pi)")")")"   # ajustala si no coincide
node --test extensions/*/*.test.mjs
```

Sin `PI_PKG_ROOT`, los tests de integración de las extensiones dicen `SKIP` y no fallan.

## 7 · Desinstalar

```sh
./uninstall.sh                  # quita comandos y extensiones; conserva tu configuración
./uninstall.sh --purge-config   # además borra ~/.config/gs3 (incluye el token de Vesktop, si lo hubo)
```

## Si algo falla

| Síntoma | Probá |
| --- | --- |
| `herdr not found` | Instalá herdr y reabrí la terminal |
| La barra sale vacía al instante | `gs3-hud` sale en silencio si el panel no existe: revisá `herdr pane list` |
| `gs3: no shell to run` | `GS3_SHELL=pi gs3` o instalá Pi |
| La compilación falla sin red | Ya hay `Cargo.lock`: `cargo build --release --locked` |

Más detalle en [docs/install.md](../docs/install.md). Para hacerlo con ayuda de una IA: [ai-agent.md](ai-agent.md).
