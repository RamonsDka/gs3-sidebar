# Seguridad

## Qué toca la barra

| Elemento | Detalle |
| --- | --- |
| Lectura | Sesión de Pi, git, árbol del proyecto, Engram (SQLite en solo lectura) |
| Escritura | Solo su propio `gs3-hud.json` y, si usás la voz, `~/.config/gs3/discord-token.json` (permisos `0600`) |
| Control de paneles | Un único `pane.focus` y la tecla fija `F9` hacia el panel de Pi |
| Procesos | Vesktop (engranaje) y Flameshot (botón de captura) |
| Red | La barra no abre conexiones externas por sí misma. Solo habla con el demonio local de Engram (`http://127.0.0.1:7437`, solo lectura) y con sockets locales (herdr, Vesktop). La tarjeta *Pull requests* ejecuta `gh`, que sí consulta GitHub con tu sesión de `gh` |

La barra nunca muestra valores de configuración de MCP, ni lee archivos con nombre de secreto (`.env*`, `*token*`, `*.pem`, `id_rsa`…).

## Cómo reportar una vulnerabilidad

No abras un issue público. Usá **[Report a vulnerability](https://github.com/RamonsDka/gs3-sidebar/security/advisories/new)** (aviso privado de GitHub). Incluí versión, pasos para reproducir e impacto. Se responde lo antes posible; es un proyecto personal, sin plazos garantizados.

## Si compartiste tu token de voz

Borralo y volvé a autorizar: `rm ~/.config/gs3/discord-token.json`.
