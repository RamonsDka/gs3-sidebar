# Contribuir a gs3-sidebar

¡Gracias por querer mejorar la barra! Esta guía es corta a propósito.

## Antes de empezar

- La barra es **de solo lectura**. Las únicas excepciones están listadas en [`skills/gs3-sidebar/SKILL.md`](skills/gs3-sidebar/SKILL.md) (atajo `F9`, guardado de su propia configuración, RPC de voz de Vesktop, botón de Flameshot). Un cambio que escriba, envíe teclas o use la red fuera de esa lista no se acepta.
- Abrí un issue antes de un cambio grande, para acordar el enfoque.
- Probado solo en **Linux (Pop!_OS + COSMIC)**. Los aportes para Windows y OpenCode son bienvenidos; marcá claramente qué probaste y qué no.

## Preparar el entorno

```sh
git clone https://github.com/RamonsDka/gs3-sidebar.git
cd gs3-sidebar
scripts/check-deps.sh
```

## Checks antes de abrir un PR

| Qué cambiaste | Comando |
| --- | --- |
| Código de la barra (`gs3/hud/`) | `CARGO_TARGET_DIR="$PWD/target" cargo test --release --manifest-path gs3/hud/Cargo.toml` |
| Verificación completa de la barra (tests, guardas de solo lectura, catálogo, render) | `skills/gs3-sidebar/scripts/doctor.sh` |
| Extensiones de Pi | `PI_PKG_ROOT=<paquete de pi> node --test extensions/*/*.test.mjs` |
| Instalador o scripts | `bash -n install.sh uninstall.sh scripts/*.sh` y `./install.sh --dry-run` |

Sin `PI_PKG_ROOT`, los tests de integración de las extensiones se saltan (`SKIP`) en lugar de fallar.

## Reglas del código

- Test primero: un test que falle, después el cambio mínimo.
- Una tarjeta nueva en la interfaz debe aparecer también en `skills/gs3-sidebar/references/cards.md` (el doctor lo comprueba).
- Nunca muestres valores de configuración de MCP, tokens ni contenido de archivos con nombre de secreto.
- Ningún `sleep` para sondear: la barra se refresca por eventos.
- Documentá lo que cambies: ayuda del comando y los `docs/` correspondientes.

## Capturas

Si cambiás algo visible, actualizá la captura en `docs/assets/`. **Difuminá** nombres de usuario, rutas personales, listas de modelos o proveedores y cualquier texto privado antes de subirla. No publiques imágenes personales de fondo.

## Commits y PR

- Mensajes en formato [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:`…).
- Un PR por tema, idealmente de menos de ~400 líneas cambiadas.
- Completá la plantilla del PR.

## Licencia

Al contribuir aceptás que tu aporte se publique bajo la [licencia MIT](LICENSE).
