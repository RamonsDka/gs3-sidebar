# Instalación (Linux)

> Probado en **Pop!_OS con COSMIC**, Wayland, terminal Ghostty. Otras distribuciones de Linux deberían funcionar si cumplen los requisitos, pero no se probaron.

## Camino rápido

```sh
git clone https://github.com/RamonsDka/gs3-sidebar.git
cd gs3-sidebar
./install.sh
```

Luego abrí una terminal **dentro de herdr** y ejecutá `gs3`. En Pi, `/reload` carga las extensiones.

## Requisitos

| Herramienta | Para qué | Obligatoria |
| --- | --- | --- |
| [herdr](https://herdr.dev) | El multiplexor donde vive la barra | Sí |
| `jq` | El lanzador lee el JSON de herdr | Sí |
| `cargo` (Rust) | Compilar `gs3-hud` (no hace falta si pasás `--binary`) | Sí, salvo `--binary` |
| Compilador C (`build-essential`) | SQLite viene incluido y se compila | Con `cargo` |
| Pi / Gentle Shell | El agente que se monitorea | Para ver datos |
| `flatpak` + Flameshot | Botón de captura (solo Flatpak `org.flameshot.Flameshot`) | No |
| Vesktop ≥ 1.6.9 | Botones de voz (Discord oficial no está soportado) | No |
| `gh` | Pestaña GIT: pull requests | No |
| `node` | Solo para correr los tests de las extensiones | No |

`scripts/check-deps.sh` verifica todo esto y te dice qué falta.

## Qué hace `install.sh`

Ejecuta, en orden, los scripts de `scripts/`. Podés correr cada uno por separado.

| Paso | Script | Resultado |
| --- | --- | --- |
| 1 | `check-deps.sh` | Comprueba herramientas y plataforma |
| 2 | `build-hud.sh` | Compila `gs3-hud` e instala `~/.local/bin/gs3-hud` |
| 3 | `install-launcher.sh` | Instala `~/.local/bin/gs3` y crea `~/.config/gs3/` |
| 4 | `install-extensions.sh` | Copia las extensiones a `~/.pi/agent/extensions/` |
| 5 | `doctor.sh` | Verifica la instalación |

Si ya tenías una extensión con el mismo nombre y distinta, **no se borra**: se renombra a `<nombre>.bak.<fecha>`.

### Opciones

| Opción | Efecto |
| --- | --- |
| `--dry-run` | Muestra qué haría, sin cambiar nada |
| `--prefix <dir>` | Dónde van `bin/gs3` y `bin/gs3-hud` (por defecto `~/.local`) |
| `--agent-dir <dir>` | Carpeta de Pi; las extensiones van en `<dir>/extensions` (por defecto `~/.pi/agent`, o `PI_CODING_AGENT_DIR`) |
| `--config-dir <dir>` | Configuración de la barra (por defecto `~/.config/gs3`) |
| `--binary <archivo>` | Usa un `gs3-hud` ya compilado en vez de compilar |
| `--no-extensions` | Instala solo la barra |
| `-y`, `--yes` | No pide confirmación |

Si `~/.local/bin` no está en tu `PATH`, el instalador te lo avisa. Agregalo con `export PATH="$HOME/.local/bin:$PATH"`.

## Primer uso

1. Abrí herdr y una terminal dentro de él.
2. Ejecutá `gs3`. El panel se divide: Pi a la izquierda, la barra a la derecha.
3. Escribí algo a Pi: la barra pasa de `idle` a `working` y empieza a mostrar tokens y costo.

Fuera de herdr, `gs3` crea un espacio de trabajo nuevo y se conecta a él.

### El estado del agente

Para que herdr conozca la sesión de Pi (y la barra la siga al hacer `/new` o `/resume`), instalá su integración una vez:

```sh
herdr integration install pi
```

## Verificar y desinstalar

```sh
scripts/doctor.sh          # sano = código de salida 0
./uninstall.sh             # quita comandos y extensiones; conserva tu configuración
./uninstall.sh --purge-config   # además borra ~/.config/gs3
```

Para que `doctor.sh` renderice la barra contra un panel real, ejecutalo dentro de herdr; fuera, solo comprueba que el programa arranca.

## Si algo falla

| Síntoma | Causa probable | Solución |
| --- | --- | --- |
| `gs3: herdr not found` | herdr no está en el `PATH` | Instalalo desde herdr.dev |
| `gs3: no shell to run` | No hay `gentle-shell2` ni `pi` | Instalá Pi o definí `GS3_SHELL=<comando>` |
| Pi corre sin barra, con aviso | `gs3-hud` no está instalado | `scripts/build-hud.sh` |
| La barra dice `conectado` pero sin datos | Pi no está en el panel enlazado | Lanzalo con `gs3`, no a mano |
| `/model-picker` no existe | Extensiones sin cargar | `/reload` en Pi |
| Los tests de extensiones dicen SKIP | Falta `PI_PKG_ROOT` | Ver [CONTRIBUTING](../CONTRIBUTING.md) |

Siguiente paso: [configuración](configuration.md).
