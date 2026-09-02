# Modelo de Encriptación

## 1. Objetivo y alcance

Este documento define la estrategia de cifrado para SyncFiles en V1 y su preparación para V2.

La clave de decisión arquitectónica es esta:
- V1 no exige encriptación end-to-end del contenido de archivos,
- V1 sí exige cifrado de la comunicación y protección de metadatos operativos,
- la arquitectura debe estar preparada para introducir E2E más adelante sin reescribir la base funcional.

## 2. Principios de diseño

- La confidencialidad no debe depender de la intuición ni de la configuración manual del usuario.
- Los datos en tránsito deben ir protegidos por TLS 1.3 en todo caso.
- Los metadatos operativos pueden ser visibles en V1, pero deben tratarse con control de acceso.
- Los archivos en reposo deben cifrarse al menos a nivel de disco o almacenamiento del backend.
- La evolución a E2E debe ser posible sin romper el flujo de sincronización ni la experiencia del cliente.

## 3. Política de encriptación por versión

### 3.1 V1

En V1 se acepta lo siguiente:
- cifrado de transporte obligatorio con TLS 1.3,
- protección de almacenamiento en backend con cifrado a nivel de disco o servicio,
- metadatos de sincronización y auditoría visibles para el servidor,
- no se exige E2E del contenido,
- las claves de almacenamiento pueden estar bajo control del servidor o de un servicio de KMS interno.

### 3.2 V2

En V2 se incorpora la siguiente evolución:
- cifrado end-to-end del contenido de archivos,
- claves generadas por usuario o dispositivo y no solo por el backend,
- separación estricta entre metadatos operativos y metadatos sensibles,
- gestión de revocación, recuperación de claves y rotación segura,
- almacenamiento de claves con hardware o KMS gestionado por el usuario.

## 4. Algoritmos recomendados

### 4.1 Transporte

- Protocolo: TLS 1.3
- Ciphersuites permitidos: TLS_AES_256_GCM_SHA384 y TLS_CHACHA20_POLY1305_SHA256
- Objetivo: autenticidad, confidencialidad e integridad del canal

### 4.2 Archivos en reposo (V1)

- Algoritmo recomendado: AES-256-GCM
- Uso: cifrado del contenido del archivo o de los chunks antes de guardarlos en almacenamiento local o en Nas
- Principio: autenticación integrada con GCM para detectar corrupción o alteración
- Alternativa viable: ChaCha20-Poly1305 para dispositivos móviles con menos soporte de AES nativo

### 4.3 Derivación de claves

- Para V1: derivación con HKDF o KDF compatible con soporte del entorno operativo
- Para V2: derivación de claves por usuario/devices con HKDF sobre material maestro seguro
- Semilla de clave: generada con RNG criptográfico de alta calidad
- Rotación: cada archivo o cada lote puede tener clave separada en evolución futura

### 4.4 Hashes e integridad

- SHA-256 para hashes de archivo y comprobación general
- El cliente y el servidor deben validar `checksum` antes y después de sincronizar
- Los chunks o partes fragmentadas deben llevar su propia suma de verificación cuando se transfieran

## 5. Estructura de claves

En V1 la estructura debe ser simple y operable:
- una clave maestra de servicio para cifrado del almacenamiento operativo,
- claves de sesión para autenticar y proteger la comunicación,
- metadatos de sincronización sin cifrar, pero con control de acceso,
- no se usa una capa E2E para contenido.

En V2 la estructura debe evolucionar a:
- clave maestra del usuario,
- claves por dispositivo,
- claves por archivo o por lote,
- clave de wrapping para almacenamiento seguro,
- separación clara entre metadatos legibles y metadatos sensibles.

## 6. Política de almacenamiento seguro

- Los datos en reposo deben permanecer cifrados aunque el disco o el almacenamiento se vea comprometido.
- Las copias de seguridad deben cifrarse en reposo con la misma política del almacenamiento principal.
- Las claves deben protegerse en un almacenamiento seguro del sistema operativo o en un KMS controlado por la plataforma.
- El servidor debe registrar acceso y rotación sin exponer el valor de la clave en logs.

## 7. Consideraciones de seguridad funcional

- La sincronización no debe requerir que el servidor pueda leer el contenido del archivo para funcionar en V1.
- Sin embargo, V1 acepta que la infraestructura pueda acceder a contenido en reposo si el sistema operativo y la política de despliegue lo permiten.
- La separación de responsabilidades debe prepararse para V2: metadatos operativos y contenido serán tratados por capas distintas.

## 8. Criterios de aceptación

El diseño de encriptación se considera correcto si:
- toda comunicación usa TLS 1.3,
- el almacenamiento persistente no se deja sin cifrado,
- la arquitectura soporta V1 sin exigir E2E,
- el sistema guarda la base para una migración a E2E clara y no improvisada,
- la gestión de claves se puede rotar y auditar sin romper la operación normal.

## 9. Recomendación final

Para V1, la recomendación práctica es:
- TLS 1.3 + AES-256-GCM para reposo + checksum por archivo
- sin E2E obligatorio,
- con base preparada para migración segura a V2.

Esto ofrece un equilibrio correcto entre seguridad real, simplicidad operativa y capacidad de evolución.
