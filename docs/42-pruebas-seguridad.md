# Pruebas de Seguridad

## 1. Objetivo

Las pruebas de seguridad de SyncFiles deben validar que la plataforma no solo sincroniza archivos, sino que lo hace con autenticación válida, integridad de datos, protección de sesión y manejo seguro de conflictos y metadatos.

En V1 el enfoque es práctico y concentrado:
- autenticación y autorización,
- protección de transporte,
- límites y control de abuso,
- validación de integridad de archivos,
- manejo seguro de sesión, tokens y dispositivos.

En V2 la batería se amplia para incluir E2E, gestión de claves, NAS y topología distribuida.

## 2. Criterios de seguridad clave

### 2.1 Autenticación

- credenciales precreadas y restringidas,
- sesiones con expiración,
- `device_id` vinculado a la sesión,
- rechazo de sesiones revocadas o expiradas,
- logs mínimos y auditables.

### 2.2 Autorización

- cada request debe validar `user_id`, `device_id` y permisos,
- no se permite acceso cruzado entre usuarios,
- no se permiten rutas no normalizadas ni traversal paths,
- los empleados del sistema no deben tener acceso directo a archivos sensibles sin justificación.

### 2.3 Integridad

- checksum obligatorio antes y después del sync,
- validación del contenido en upload y download,
- rechazo de archivos corruptos o no esperados,
- bloqueo de operaciones que no puedan verificarse.

## 3. Tipos de pruebas de seguridad

### 3.1 Pruebas de autenticación

- sesión expirada,
- token robado o reutilizado,
- login con usuario inexistente,
- intento de acceso desde dispositivo sin permiso,
- abuso de reintentos y fuerza bruta.

### 3.2 Pruebas de autorización

- acceso a archivos ajenos,
- bypass por rutas relativas,
- manipulación de `file_id` o `path_hash`,
- uso de secuencias de estado no válidas.

### 3.3 Pruebas de transporte y red

- downgrade de TLS,
- certificado inválido,
- conexión sin HTTPS,
- ataque de replay con tokens antiguos,
- timeouts, errores 401/403/409 y rate limit.

### 3.4 Pruebas de almacenamiento

- escritura de archivos fuera del directorio sincronizado,
- corrupción del metadata SQLite,
- doble ejecución de una operación con la misma `idempotency_key`,
- recuperación tras pérdida de cola o fallo de disco.

## 4. Revisión criptográfica

Se recomienda realizar una revisión de la política de cifrado de la siguiente forma:
- verificar TLS 1.3 obligatorio,
- validar algoritmo de cifrado para reposo,
- revisar derivados de clave y rotación,
- verificar que los hashes de archivo no estén expuestos en logs sensibles,
- asegurar que la estrategia de V1 no impida la evolución a E2E en V2.

## 5. Matriz de escenarios de seguridad

| Área | Escenario | Esperado |
|---|---|---|
| Auth | Token expirado | Rechazo y reautenticación |
| Auth | Dispositivo no permitido | Error 403 y bloqueo de operación |
| Integrity | Checksum no coincide | Rechazo y marca de error |
| Storage | Archivo fuera de ruta | Error y validación de ruta |
| API | Replay de request | Rechazo por idempotencia o sesión inválida |
| Sessions | Duplicidad de sesión | Validación por device_id + revocación |
| Conflict | Sobrescritura silenciosa | Bloqueado, requiere confirmación |

## 6. Requisitos mínimos de auditoría

Debe estar disponible una auditoría mínima con:
- `user_id`, `device_id`, `timestamp`, `evento`, `resultado`, `motivo`, `ip` si aplica,
- eventos de login, logout, sesión revocada, conflicto, rechazo por checksum y fallos de auth,
- retención mínima operativa y segura.

## 7. Revisión de seguridad para release

Antes de cada release o cierre de versión:
- ejecuta pruebas de seguridad automatizadas,
- valida dependencias y librerías con escaneo de vulnerabilidades,
- revisa configuración de TLS y almacenamiento,
- comprueba que los logs no contienen datos sensibles,
- valida que no hay acceso directo a archivos en almacenamiento no autorizado.

## 8. Criterios de aceptación

La versión V1 se considera segura si:
- las sesiones no pueden reutilizarse ni sobrevivir a la expiración,
- los archivos corruptos se detectan antes del commit final,
- la API no permite acceso cruzado de usuarios o dispositivos,
- los conflictos no producen sobrescrituras silenciosas,
- la arquitectura permite evolucionar a E2E sin cambiar la seguridad base del sistema.

## 9. Recomendación final

Para V1, la seguridad debe ser “sólida y simple”: sesiones, TLS, checksum, control de acceso y no sobrescritura silenciosa. Eso aporta protección real sin crear una base de seguridad demasiado compleja para el MVP. V2 puede añadir E2E y gestión avanzada de claves, pero no debe depender de reescribir la capa base de seguridad actual.
