# Audionáutica — arquitectura futura (notas)

Estas notas **no** están implementadas. Existen para no pintar el dominio contra una pared.

## SonoBus Jam Capture

Capturar stems de participantes (Felipe / Cata / Daniver / Master) con el mismo origen de sync.

`AudioAsset` ya reserva:

- `sourceType = SONOBUS_STEM`
- `participant`
- `sync_group`
- `timeline_offset_seconds`

Dirección preferida: Arrangement View de Ableton como timeline canónica. Session View puede representar la misma toma como una Scene con clips alineados. Sprint 1 no habla con SonoBus.

## Ableton Bridge / Max for Live

Un dispositivo M4L o un puente OSC/MIDI podría:

- anunciar consolidates en caliente
- empujar assets de la biblioteca de vuelta al set
- marcar el origin clip

El adapter actual es de **archivo**, no de Live API. Mantener esa separación.

## Project Publish / Pull y versioning

Más adelante: publicar un proyecto, pull del último, historial, locks, branches/experiments, colaboradores. Eso vive al lado de `Project`, no dentro de `AudioAsset`. No mezclar versionado de set con hash de loop.

## Dropbox API / Google Drive API

Hoy el provider es una carpeta local sincronizada. El trait `StorageProvider::put_relative` debe poder implementarse con:

- `DropboxApiProvider`
- `GoogleDriveApiProvider`
- `S3Provider` / `R2Provider`
- `AudionauticaCloudProvider`

sin cambiar `AudioAsset`. `asset_storage_locations` ya modela N copias por asset.

## Community / Loop Hunts

Assets públicos, remixes, provenance, licencias, `creator`, `visibility`, `parent_id`. Guardarlos como columnas o JSON de metadata cuando llegue el momento; no construir la red social ahora.

Loop Hunts = captura de field recordings / texturas como contribución a una biblioteca compartida. El `sourceType` `FIELD_RECORDING` / `COMMUNITY_UPLOAD` ya existe en el enum.

## iPad generative instrument

Cliente que consulta `GET /assets` filtrando por BPM, category, year, project, creator, similarity, usage, y carga samples para granular / sequencing / multitouch.

Por eso el dominio no depende de Tauri ni de paths nativos. Una API futura serializa `AudioAsset` tal cual.

## Audio embeddings, BPM/key detection

- `detectedBpm` ya está en el schema (null).
- Embeddings y key pueden vivir en tablas laterales (`asset_features`) para no inflar el row principal.
- No acoplar clasificación automática a las carpetas físicas.

## Cloud backend / auth / users

Fuera de Sprint 1. SQLite local es la fuente de verdad del músico en su máquina. Un backend futuro replica assets e identidades; no al revés.
