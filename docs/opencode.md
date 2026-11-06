# OpenCode

> [!WARNING]
> **Nada de esto está probado.** La barra se hizo para Pi sobre herdr. Esta guía explica qué parte se puede reutilizar tal cual, qué habría que escribir de nuevo y cómo empezar. Los comandos de herdr y de OpenCode salen de su documentación oficial; el código nuevo que se describe **no existe todavía**. No hay capturas de OpenCode porque nunca se corrió.

## Respuesta corta

Con OpenCode en un panel de herdr, **la mitad de la barra funcionaría casi sin cambios** y la otra mitad necesita un lector de sesión nuevo.

| Pestaña / tarjeta | ¿Depende de la sesión de Pi? | Con OpenCode |
| --- | --- | --- |
| **GIT** (grafo, ramas, diff, PR) | No: usa el directorio del panel | Debería funcionar |
| **TREE** (árbol del proyecto) | No | Debería funcionar |
| **MEM**: Engram, Stack | No (Engram se lee aparte) | Debería funcionar |
| **CONFIG** | No | Debería funcionar |
| **MAIN**: estado `idle/working/blocked` | Sí, vía herdr | Funciona si la integración de herdr informa el estado |
| **MAIN**: tokens, contexto, costo, tareas, modelo | **Sí: lee el JSONL de sesión de Pi** | **Hay que escribir un lector nuevo** |
| **MEM**: tools, MCP · **PROF** | **Sí** (sesión de Pi, `settings.json`, `agents/*.md`) | **Hay que escribir un lector nuevo** |
| Botón del modelo (`F9`) | Sí: depende de la extensión model-picker de Pi | No aplica: es una extensión de Pi |
| Voz y captura | No | Igual que en Linux |

## Por qué ocurre

La barra no le pregunta nada a Pi. Pide a herdr la información del panel y herdr responde con la **ruta del archivo de sesión**:

```json
"agent_session": { "agent": "pi", "kind": "path", "value": "…/sessions/…/<fecha>_<id>.jsonl" }
```

La barra abre ese archivo `.jsonl` y lee solo los bytes nuevos (inotify): de ahí salen tokens, costo, modelo, tareas, llamadas a herramientas y MCP. Eso es lo específico de Pi.

OpenCode guarda sus sesiones de otra forma: una base **SQLite** (`opencode.db`, en `~/.local/share/opencode/`), con tablas como `session`, `message`, `part` y `todo`, y la reporta a herdr por su propia integración (que informa el *id* de sesión). Hasta donde se pudo comprobar, no entrega una ruta a un JSONL. Por eso las tarjetas de MAIN que cuentan tokens necesitan otra fuente.

## Paso 1: lo que sí se puede probar ya

```sh
# 1. Instalá la barra como en Linux
./install.sh --no-extensions        # las extensiones son de Pi, no sirven aquí

# 2. Que herdr conozca a OpenCode (la carpeta de configuración debe existir antes)
herdr integration install opencode
#    -> instala plugins en ~/.config/opencode/plugins/  (reiniciá OpenCode)

# 3. Abrí herdr, entrá a tu proyecto y lanzá OpenCode en un panel
opencode

# 4. En otro panel, enlazá la barra al panel de OpenCode
herdr pane list                      # copiá el pane_id del panel de OpenCode
gs3-hud --pane <ese_pane_id> --cwd "$PWD"
```

> `gs3` (el comando) abre **gs2/Pi** en el panel grande. Para OpenCode usá `GS3_SHELL=opencode gs3`, o lanzá `gs3-hud` a mano como arriba.

Con eso deberías ver GIT, TREE, MEM (Engram, versiones) y CONFIG. En MAIN, el estado y las tarjetas de tokens pueden aparecer vacías o en cero: es lo esperado.

Herdr reconoce OpenCode (≥ 1.18.29) y su integración informa también el estado `idle` / `working` / `blocked`. En **Windows** esa integración se reportó como no soportada ([issue #1531 de herdr](https://github.com/ogulcancelik/herdr/issues/1531)), así que desde Windows no cuentes con ella.

## Paso 2: tokens y tareas (requiere código nuevo)

Hay que enseñarle a la barra a leer SQLite de OpenCode. El camino más corto:

1. **Ubicá los datos.** La base está en `~/.local/share/opencode/opencode.db` (el nombre cambia en builds de desarrollo). Ábrila en **solo lectura**, como ya hace la barra con Engram (`rusqlite` ya es dependencia).
2. **Mapeá la sesión.** La integración de herdr informa el *id* de sesión de OpenCode; con él consultás `message` / `part` / `todo`. Confirmá antes el formato real con `herdr pane get <pane_id>` y con `sqlite3 -readonly opencode.db .schema`: el esquema cambia entre versiones.
3. **Normalizá.** En `gs3/hud/src/sources.rs` la sesión de Pi se resume en una estructura `Session` (modelo, esfuerzo, tokens de entrada/salida, costo, tareas). Un lector nuevo, `opencode_session(db, id) -> Session`, alimenta el mismo estado y **la interfaz no cambia**.
4. **Elegí la fuente.** Si `agent_session.kind == "path"` y termina en `.jsonl`, usá el lector de Pi; si es un id de OpenCode, usá el nuevo.
5. **Refresco.** SQLite no avisa de los cambios por inotify con la misma limpieza que un archivo que crece. Lo más simple y seguro: releer al terminar cada turno (herdr ya emite `pane.agent_status_changed`) y con la tecla `r`.

Respetá las reglas de la barra: solo lectura, sin mostrar valores de configuración, sin escribir en bases ajenas. Están en [`skills/gs3-sidebar/SKILL.md`](../skills/gs3-sidebar/SKILL.md), pensado para que un agente haga este cambio con seguridad ([prompt listo en `prompts/ai-agent.md`](../prompts/ai-agent.md#adaptar-a-opencode)).

## Paso 3: extensiones

Las extensiones de este repo usan la API de Pi y **no sirven en OpenCode**. OpenCode carga plugins de JavaScript/TypeScript desde `~/.config/opencode/plugins/` o `.opencode/plugins/`, con otra API. Para equivalentes:

| Extensión de Pi | Equivalente en OpenCode |
| --- | --- |
| model-picker | Probablemente innecesaria (OpenCode tiene su propio manejo de modelos, no verificado aquí); revisá su documentación antes de portar nada |
| copy-commands, message-frames | Dependen de las clases internas de la interfaz de Pi; habría que reescribirlas desde cero |
| herdr-workspace-title | Es la más portable: solo usa el CLI de herdr; habría que cambiar el *hook* que detecta el primer pedido |

## Lista de comprobación

- [ ] `herdr integration status` lista OpenCode como instalado
- [ ] `gs3-hud --pane <id> --once` muestra GIT, TREE y MEM
- [ ] El estado del panel cambia de `idle` a `working` al enviar un mensaje a OpenCode
- [ ] (Paso 2) Los tokens de MAIN dejan de estar en cero

Volver al [README](../README.md) · [Windows](windows.md).
