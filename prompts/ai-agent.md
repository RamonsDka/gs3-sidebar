# Prompts para instalar dándole instrucciones a una IA

Pegale estos textos a tu agente de código (Pi, OpenCode, Claude Code, Codex…) **con el repositorio ya clonado como directorio de trabajo**. Están escritos para que el agente investigue primero, haga cambios acotados y te muestre la evidencia.

> [!IMPORTANT]
> Estos prompts le piden a la IA ejecutar comandos en tu equipo. Pedile siempre un `--dry-run` primero, leé lo que propone y no le des permisos fuera de la carpeta del repo y de `~/.local`, `~/.config/gs3` y `~/.pi/agent/extensions`.

## Cómo usarlos

1. `git clone https://github.com/RamonsDka/gs3-sidebar.git && cd gs3-sidebar`
2. Abrí tu agente en esa carpeta.
3. Pegá el prompt que corresponda.
4. Revisá el plan que te devuelve antes de aprobar.

## Instalar (Linux)

```text
Estás en el repositorio gs3-sidebar. Quiero instalar la barra lateral gs3 en este equipo Linux.

Reglas:
- Leé primero README.md, docs/install.md y scripts/check-deps.sh. No inventes comandos: usá solo los de esos archivos.
- No modifiques nada fuera de: este repositorio, ~/.local/bin, ~/.config/gs3 y ~/.pi/agent/extensions.
- No uses sudo sin preguntarme antes. Si falta un paquete, decime cuál y el comando para instalarlo, y esperá.

Pasos:
1. Ejecutá scripts/check-deps.sh y resumime qué falta.
2. Ejecutá ./install.sh --dry-run y mostrame qué haría.
3. Cuando yo diga "dale", ejecutá ./install.sh -y.
4. Ejecutá scripts/doctor.sh y mostrame la salida completa y el código de salida.

Al terminar decime, en una tabla: qué se instaló y dónde, qué falló o se omitió, y qué tengo que hacer yo a mano (por ejemplo abrir herdr y ejecutar `herdr integration install pi`).
```

Cómo saber que salió bien: la IA te muestra `doctor.sh` con código de salida `0` y las rutas `~/.local/bin/gs3` y `~/.local/bin/gs3-hud`.

## Diagnosticar una instalación que no anda

```text
La barra gs3 no se ve o muestra datos vacíos. Diagnosticalo sin cambiar nada todavía.

1. Leé docs/install.md (sección "Si algo falla") y gs3/README.md (tabla de situaciones).
2. Ejecutá solo comandos de lectura: scripts/doctor.sh, `command -v gs3 gs3-hud herdr jq`, `herdr pane list`, `gs3-hud --help`.
3. Decime cuál de estas causas es y con qué evidencia: herdr no está en el PATH; gs3-hud no instalado; Pi no corre en el panel enlazado; la integración de herdr con Pi no está instalada (`herdr integration status`); ~/.config/gs3 no existe.
4. Proponé el arreglo mínimo y esperá mi aprobación para aplicarlo.
```

## Personalizar

```text
Quiero cambiar la barra gs3. Antes de editar, leé skills/gs3-sidebar/SKILL.md completo y respetá sus reglas: la barra es de SOLO LECTURA, con únicas excepciones el atajo F9 al selector de modelos, guardar su propia configuración, el RPC de voz de Vesktop y el botón de Flameshot.

Cambio que quiero: <describilo aquí, por ejemplo "agregar una tarjeta con la rama remota en la pestaña GIT">.

Hacelo así:
1. Mostrame los archivos que vas a tocar y por qué (gs3/hud/src/…).
2. Escribí primero un test que falle y mostrame que falla.
3. Implementalo con el mínimo de código y mostrame que el test pasa.
4. Ejecutá `CARGO_TARGET_DIR="$PWD/target" cargo test --release --manifest-path gs3/hud/Cargo.toml` y mostrame el resultado.
5. No agregues escritura, envío de teclas ni red a la barra.
```

## Adaptar a OpenCode

```text
Estás en el repositorio gs3-sidebar. Quiero que la barra lateral gs3 muestre también sesiones de OpenCode. Esto NO está implementado ni probado.

Contexto que tenés que confirmar leyendo, no asumiendo:
- docs/opencode.md explica qué se puede reutilizar y qué falta.
- La barra obtiene de herdr la ruta del JSONL de sesión de Pi (campo agent_session de `herdr pane get`) y lo parsea en gs3/hud/src/sources.rs. OpenCode guarda sus sesiones en SQLite.

Hacé esto en orden, parando para mostrarme el resultado de cada paso:
1. Con herdr abierto y OpenCode corriendo en un panel, ejecutá `herdr pane list` y mostrame el campo agent_session de ese panel. No sigas hasta que sepamos qué informa realmente.
2. Localizá la base de OpenCode (~/.local/share/opencode/) y mostrame su esquema con `sqlite3 -readonly <db> .schema`. NUNCA la abras con escritura.
3. Proponeme un diseño mínimo: una función que lea la sesión de OpenCode y devuelva la misma estructura que hoy produce el lector de Pi, para no tocar la interfaz.
4. Leé skills/gs3-sidebar/SKILL.md y respetá sus reglas (solo lectura, no mostrar valores de configuración).
5. Con mi aprobación, implementalo test-first y ejecutá cargo test.

No inventes tablas, columnas ni comandos: si algo no lo podés comprobar, decímelo.
```

## Adaptar a Windows

```text
Estás en el repositorio gs3-sidebar, en Windows. Leé docs/windows.md. Quiero evaluar si gs3-hud puede compilar de forma nativa.

1. Ejecutá `cargo build --release --manifest-path gs3/hud/Cargo.toml` y mostrame los errores tal cual.
2. Listá los usos de `std::os::unix`, `/proc` y `flatpak` en gs3/hud/src y proponé un plan para aislarlos con #[cfg(unix)] / #[cfg(windows)].
3. No empieces a portar hasta que yo apruebe el plan. Si preferís WSL2, decímelo y explicá por qué.
```

## Qué no pedirle a la IA

- Que **silencie o ensordezca** tu Vesktop real para "probar" los botones de voz.
- Que suba tu `~/.config/gs3/discord-token.json` o cualquier `.env`.
- Que "arregle" tests de integración borrándolos: sin `PI_PKG_ROOT` simplemente se saltan.

Volver a [prompts por comandos](cli.md) · [README](../README.md).
