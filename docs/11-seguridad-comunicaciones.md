# Seguridad de Comunicaciones

## 1. Descripción

Esta capa define la protección de los datos en tránsito entre clientes y servidor de SyncFiles.

La política de V1 es estricta y simple:
- TLS 1.3 obligatorio,
- autenticación de sesión por token,
- validación de identidad del dispositivo,
- checksum y verificación de integridad,
- sin asumir E2E como mecanismo de transporte.

## 2. Principios

- La comunicación siempre debe ir cifrada.
- No se aceptan endpoints sin TLS ni sesiones no verificadas.
- Los clientes deben poder detectar errores de autenticación e interrumpir operaciones remotas antes de comprometer datos.
- La seguridad del canal protege la transmisión, pero no reemplaza la seguridad del almacenamiento ni la futura política E2E.

## 3. Requisitos mínimos de V1

- TLS 1.3 con validación de certificados.
- Sesiones con expiración y revocación.
- Tokens vinculados a `user_id` y `device_id`.
- Rechazo de conexiones sin autenticación.
- Protección contra abuso de rate limiting y reintentos masivos.

## 4. Flujo de autenticación

1. El cliente intenta iniciar sesión con credenciales precreadas.
2. El servidor valida identidad y devuelve una sesión válida.
3. La sesión se asocia a un dispositivo concreto.
4. Cada request posterior incluye el token de sesión.
5. Si la sesión expira o el servidor rechaza la credencial, la app bloquea operaciones remotas y exige re-autenticación.

## 5. Integridad y validación

- Cada archivo se valida con checksum antes de aceptar una operación remota.
- Si el checksum no coincide, la operación se rechaza o se marca como inconsistente.
- Los errores de red o timeout no deben considerarse confirmación de ejecución.
- La app debe operar con `idempotency_key` para prevenir ejecuciones duplicadas.

## 6. Recomendación para V2

En V2 se puede reforzar la seguridad con:
- E2E del contenido,
- claves por dispositivo y por archivo,
- rotación más avanzada de sesiones,
- autenticación más fuerte si se habilita acceso multiusuario o colaborativo.

La base de V1 debe preparar esta evolución sin cambiar el modelo cliente-servidor.
