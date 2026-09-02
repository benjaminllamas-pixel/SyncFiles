# Protocolo de Comunicación

## 1. Objetivo

Este documento define la comunicación oficial entre clientes y servidor de SyncFiles en V1.

La decisión final es coherente con la arquitectura general:
- V1 usa REST/JSON sobre HTTPS/TLS 1.3,
- la sincronización se basa en polling,
- no se requiere WebSocket ni gRPC en el MVP,
- el contenido y la semántica del archivo se modelan como metadatos y payloads HTTP,
- la evolución a protocolos más avanzados queda reservada para V2.

## 2. Opciones evaluadas

### 2.1 WebDAV
- Ventajas: estándar, compatible y ampliamente soportado.
- Desventajas: no está optimizado para sincronización de deltas ni para lógica de conflicto.
- Resultado: descartado para V1.

### 2.2 rsync
- Ventajas: muy eficiente para transferencia de archivos y checksum.
- Desventajas: no encaja bien con un servicio HTTP nativo y requiere más adaptación de flujos de control.
- Resultado: descartado como protocolo principal.

### 2.3 SFTP
- Ventajas: cifrado y compatibilidad con transferencia segura.
- Desventajas: no optimiza el patrón del producto para cambios incrementales ni para sincronización de estado de archivos.
- Resultado: descartado para V1.

### 2.4 Protocolo binario propio
- Ventajas: muy eficiente para sincronización en tiempo real si se requiere más adelante.
- Desventajas: más complejidad de implementación, más superficie de errores y menor interoperabilidad.
- Resultado: reserva para V2, no para MVP.

## 3. Decisión de V1

La decisión de V1 es:
- REST/JSON sobre HTTPS/TLS 1.3,
- polling de cambios como mecanismo primario,
- transferencias de archivos por HTTP multipart o streaming,
- `idempotency_key` en operaciones con efectos persistentes,
- API versionada por contrato y semver de servidor.

## 4. Modelo de comunicación

### 4.1 Endpoints principales
- `POST /api/v1/auth/login`
- `POST /api/v1/auth/logout`
- `GET /api/v1/files/changes`
- `POST /api/v1/files/upload`
- `POST /api/v1/files/download`
- `POST /api/v1/files/delete`
- `POST /api/v1/files/rename`
- `POST /api/v1/files/move`
- `POST /api/v1/files/copy`
- `POST /api/v1/conflicts/resolve`
- `GET /api/v1/session/status`

### 4.2 Contrato de mensajes

Los request y response deben seguir el siguiente patrón:
- `request_id` para trazabilidad,
- `device_id` para asociar la operación a un cliente,
- `user_id` para autorización,
- `idempotency_key` para evitar re-ejecución,
- `checksum` para validación del contenido,
- `modified_at` para comparación y conflicto.

### 4.3 Política de polling

- el cliente consulta el servidor con una frecuencia configurable,
- el servidor devuelve deltas desde la última versión conocida,
- la respuesta debe ser estable y determinista,
- si el cliente está inactivo, el polling debe bajar su frecuencia para ahorrar batería y red.

## 5. Seguridad del protocolo

- HTTPS con TLS 1.3 obligatorio,
- tokens de sesión con expiración y revocación,
- validación del dispositivo asociada a la sesión,
- rejection de requests sin token o con certificado inválido,
- checksum y validación de contenido antes de aceptar una escritura.

## 6. Evolución hacia V2

En V2 se puede evaluar una capa adicional de comunicación en tiempo real:
- WebSocket para eventos de sincronización,
- gRPC para tráfico interno y multiplexado eficiente,
- protocolo binario propio para optimización de archivos grandes o alto volumen.

Sin embargo, esa evolución no reemplaza el contrato básico de V1: la sincronización segura y consistente sigue siendo el objetivo central.

## 7. Decisión final

La arquitectura de V1 se fija en REST/JSON + polling por elección funcional y operativa. Es la opción más segura, más simple de depurar y mejor alineada con el MVP de SyncFiles. El protocolo binario propio queda como un paso posterior cuando el producto y su carga lo justifiquen.

