# Pokedex Audionautica 1.0.6

Actualizaciones automáticas desde GitHub y mejoras de instalación.

## Windows

- Instala `Pokedex Audionautica_1.0.6_x64-setup.exe` (NSIS) o el MSI equivalente.
- **Actualiza la instalación existente** (mismo identificador `cl.audionautica.desktop`; no crea una app duplicada).
- **Signing:** unsigned.

## macOS

- Descarga el DMG universal o específico de tu arquitectura.
- Si Gatekeeper bloquea: **Ajustes del Sistema → Privacidad y seguridad → Abrir igual**.

## Novedades

- **Buscar actualizaciones:** al abrir la app se consulta GitHub Releases y avisa si hay una versión nueva.
- **Botón en el footer:** versión instalada + buscar/instalar actualización manualmente.
- **Instrucciones por plataforma:** SmartScreen (Windows) y Gatekeeper (macOS) en el diálogo de actualización.
- **Descarga directa:** el botón abre el instalador correcto (.msi / .dmg) desde la release de GitHub.

## Known limitations

- Sin code signing: Windows y macOS pueden mostrar advertencias al instalar.
- La primera vez con avisos de update requiere instalar manualmente esta versión (1.0.6); las siguientes pueden iniciarse desde la app.
