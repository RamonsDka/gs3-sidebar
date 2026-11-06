# Copiar comandos

Extensión local para Pi / Gentle Shell. Ejecutá `/reload` una vez después de instalarla.

En modo pantalla completa, los bloques de comandos de la última respuesta aparecen sobre el editor con el icono `⧉` al lado de cada comando, sin etiquetas ni números. Hacé clic en el icono para copiar el bloque completo. La vista previa se acorta al ancho disponible; el texto copiado no se acorta.

- `/copiar-comando`: copia el único bloque o permite elegir si hay varios.
- `/copiar-comando 2`: copia el segundo bloque.
- Nunca ejecuta comandos ni los inserta en el editor.
- Conserva saltos de línea, comillas e indentación; omite las cercas Markdown y los prefijos `$ ` al inicio de línea. No agrega un salto de línea final.
- Incluye bloques sin etiqueta o con etiqueta bash, sh, shell, zsh, fish, console, powershell, pwsh, cmd o bat. Usá bloques separados para comandos que quieras copiar por separado. No mezcles salida de terminal con comandos en un mismo bloque: se copia todo el contenido.
- En modo terminal normal, usá `/copiar-comando`: los clics pertenecen al terminal.
- Una respuesta nueva reemplaza los botones de la anterior. `/reload` reconstruye los botones desde la última respuesta de la rama actual.

Verificación: `node --test extensions/copy-commands/copy-commands.test.mjs` (desde la raíz del repo).
