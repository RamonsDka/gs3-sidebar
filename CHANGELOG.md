# Changelog

Formato basado en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/). Este proyecto usa [versionado semántico](https://semver.org/lang/es/).

## [Sin publicar]

## [0.1.0]

Primera publicación.

### Añadido

- `gs3-hud`: barra lateral de solo lectura en Rust con seis pestañas (MAIN, GIT, PROF, MEM, TREE, ⛭ CONFIG).
- Lanzador `gs3`: divide el panel de herdr (agente ~75 % / barra ~25 %).
- Botones de **voz** (micrófono, auriculares, engranaje) para Vesktop, con token privado `0600`.
- Botón de **captura** (Flameshot, solo Flatpak).
- Extensiones de Pi: `model-picker` (F9), `copy-commands`, `message-frames`, `herdr-workspace-title`.
- Instalador para Linux (`install.sh`, `uninstall.sh`, `scripts/`) con `--dry-run`.
- Documentación: instalación, pestañas, configuración, extensiones, Windows y OpenCode (guías sin probar), prompts para humanos y para IA.
- Skill `gs3-sidebar` con las reglas de seguridad y el catálogo de tarjetas.

### Limitaciones conocidas

- Probado solo en Pop!_OS + COSMIC (Wayland).
- Windows nativo: `gs3-hud` no compila (usa sockets Unix y `/proc`). Ver [docs/windows.md](docs/windows.md).
- OpenCode: sin lector de sesión propio. Ver [docs/opencode.md](docs/opencode.md).
- Flameshot fijo a `org.flameshot.Flameshot` (Flatpak); voz solo con Vesktop.

[Sin publicar]: https://github.com/RamonsDka/gs3-sidebar/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/RamonsDka/gs3-sidebar/releases/tag/v0.1.0
