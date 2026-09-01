# Pokedex Audionautica 1.0.4

Importación automática de loops compartidos desde Drive/Dropbox a la biblioteca local.

## Windows

- Instala `Pokedex Audionautica_1.0.4_x64-setup.exe` (NSIS) o el MSI equivalente.
- **Signing:** unsigned.

## Novedades

- **Importación desde espejos:** copia loops que tu amigo farmeó al Drive compartido hacia tu biblioteca local y LOOP POKEDEX.
- **Automático al abrir:** al iniciar la app revisa Drive/Dropbox y solo importa lo que falta (no rehashea ni recopia lo que ya tienes).
- **Botón manual:** "Importar ahora desde Drive/Dropbox" para forzar una revisión cuando Drive termine de bajar archivos nuevos.
- **Proyecto Biblioteca compartida:** loops importados del espejo aparecen agrupados bajo ese proyecto.

## Known limitations

Sin APIs de nube: Drive/Dropbox son carpetas locales sincronizadas. Los archivos deben estar descargados en tu PC (Disponible sin conexión) antes de importar.
