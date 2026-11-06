# Windows

> [!WARNING]
> **Nada de esto está probado.** La barra se desarrolló y verificó solo en Linux (Pop!_OS + COSMIC). Esta guía sale de leer el código y la documentación de herdr; es un mapa de lo que habría que hacer, no un procedimiento garantizado. Si lo probás, contanos cómo te fue en un issue.

## Respuesta corta

| Opción | Esfuerzo | Qué esperar |
| --- | --- | --- |
| **A. WSL2** (recomendada) | Bajo | Se instala como en Linux dentro de WSL, con las limitaciones de abajo |
| **B. Windows nativo** | Alto: hay que modificar el código | `gs3-hud` **no compila hoy** en Windows |

## Opción A: WSL2

Herdr, Pi y `gs3` corren dentro de la distribución Linux; la terminal de Windows solo muestra la salida.

1. Instalá WSL2 con una distribución (Ubuntu, por ejemplo) y abrila.
2. Dentro de WSL instalá `herdr`, `jq`, Rust y Pi, y seguí [docs/install.md](install.md) tal cual.
3. Abrí herdr dentro de WSL y ejecutá `gs3`.

Qué funciona y qué no (esperable, sin verificar):

| Función | Estado esperado |
| --- | --- |
| Barra, pestañas, git, árbol, memoria | Debería funcionar igual que en Linux |
| Extensiones de Pi | Deberían funcionar: viven en `~/.pi/agent/extensions/` de WSL |
| Botón de **voz** (Vesktop) | **No** funciona: busca el proceso de Vesktop en `/proc` y un socket de Unix de Linux; Vesktop en Windows queda fuera de WSL |
| Botón de **captura** | **No** funciona: lanza `flatpak run org.flameshot.Flameshot gui` |
| Clic en el modelo (`F9`) | Debería funcionar (todo ocurre dentro de herdr) |

Los botones que no funcionan fallan en silencio; el resto de la barra no se ve afectado.

## Opción B: Windows nativo

Herdr sí soporta Windows de forma nativa (ConPTY, con un canal por *named pipe* en lugar de socket Unix) y su integración con Pi está soportada ([documentación de herdr](https://herdr.dev/tui/docs/windows-beta/)). El problema está en `gs3-hud`.

### Lo que impide compilar hoy

Verificado en el código (`gs3/hud/src/`):

| Archivo | Uso exclusivo de Unix | Qué habría que hacer |
| --- | --- | --- |
| `sources.rs` | `std::os::unix::net::UnixStream` para hablar con herdr (líneas ~10, 202, 248) | Reemplazar por un cliente de *named pipe*. Herdr convierte `HERDR_SOCKET_PATH` en un nombre `\\.\pipe\…` (crate `interprocess`, `GenericNamespaced`) |
| `sources/voice.rs` | `UnixStream`, `/proc/net/unix`, `/proc/<pid>/comm`, permisos `0600` | Reescribir la detección de Vesktop y el transporte del RPC de Discord (en Windows el RPC de Discord suele ser un *named pipe*, no un socket Unix; verificalo antes) |
| `sources/screenshot.rs` | `flatpak`, `CommandExt::process_group` | Lanzar Flameshot nativo (`flameshot.exe gui`) |
| `gs3/bin/gs3` | Script de bash + `jq` | Reescribir en PowerShell, o ejecutarlo con Git Bash |

**Plan mínimo**, en este orden, para tener una barra útil sin voz ni captura:

1. Aislar el transporte de herdr detrás de una pequeña función `connect(socket)` y compilar el resto con `#[cfg(unix)]` / `#[cfg(windows)]`.
2. Desactivar `voice` y `screenshot` en Windows (`#[cfg(unix)]`), para que no estorben.
3. Implementar el *named pipe* para el paso 1.
4. Escribir un lanzador `gs3.ps1` equivalente a `gs3/bin/gs3`.
5. Comprobar con `cargo build --release --target x86_64-pc-windows-msvc`.

Las dependencias restantes (`crossterm`, `ratatui`, `notify`, `rusqlite` con SQLite incluido) son multiplataforma.

### Lo que ya está previsto

- `extensions/herdr-workspace-title.ts` ya contempla Windows: lee `%APPDATA%\herdr\session.json` y usa `windowsHide`.
- Pi y herdr guardan sus datos en rutas distintas; la barra obtiene la carpeta del agente a partir de la ruta de sesión que informa herdr, así que no depende de una ruta fija de Pi.

### Datos útiles de herdr en Windows

```powershell
# Instalar herdr (comando oficial)
powershell -ExecutionPolicy Bypass -c "irm https://herdr.dev/install.ps1 | iex"

# Registrar la integración con Pi (igual que en Linux)
herdr integration install pi
```

Limitaciones propias de herdr en Windows que conviene conocer: no existe `herdr terminal attach`, el cursor puede parpadear durante salida intensa y el cwd en vivo de PowerShell es parcial (`partial`). Ver [la página oficial](https://herdr.dev/tui/docs/windows-beta/).

## Antes de afirmar que funciona

- [ ] `cargo build --release` termina sin errores en Windows
- [ ] `gs3-hud --pane <id> --once` imprime las seis pestañas
- [ ] Al escribirle algo a Pi, el estado cambia de `idle` a `working`
- [ ] `/reload` carga las extensiones en Pi

Siguiente paso: [adaptar a OpenCode](opencode.md), o volver al [README](../README.md).
