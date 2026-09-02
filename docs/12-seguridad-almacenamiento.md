# Seguridad de Almacenamiento

## 1. Descripción

Este documento define cómo se protege la información que queda almacenada en los dispositivos del cliente y en el backend de SyncFiles.

En V1, la prioridad es la seguridad operativa sin introducir la complejidad del cifrado E2E del contenido. La idea es proteger los datos en reposo, limitar el acceso no autorizado y dejar el camino abierto para V2.

## 2. Principios

- Cifrado obligatorio en reposo para archivos persistentes.
- No tolerar almacenamiento sin validación de integridad.
- Separación entre metadatos operativos y datos sensibles.
- Control de acceso por usuario y dispositivo.
- Auditable, recuperable y capaz de soportar restauración sin riesgo de corrupción.

## 3. Almacenamiento del cliente

En el cliente V1:
- el directorio sincronizado debe vivir en un almacenamiento local normal,
- la aplicación debe garantizar persistencia de metadatos y cola,
- los archivos temporales y los backups de conflicto deben protegerse con permisos del sistema operativo,
- los cambios de conflicto deben conservar una copia alternativa con nombre diferenciado antes del reemplazo.

Se recomienda:
- permisos de acceso restringidos por cuenta del sistema,
- archivos temporales en un subdirectorio aislado,
- limpieza de archivos fallidos y de caché no válidos.

## 4. Almacenamiento del servidor

En V1, el servidor usa SQLite para metadatos y almacenamiento local para archivos.

El almacenamiento debe cumplir:
- volumen de archivo separado de la base de datos,
- control de acceso por usuario,
- checksum almacenado junto a la entrada del archivo,
- registros de borrado, renovación y conflicto,
- rotación y retención de logs operativos.

## 5. Cifrado en reposo

Se recomienda:
- AES-256-GCM para cifrado de archivos o chunks,
- TLS 1.3 para transporte,
- cifrado del volumen o del almacenamiento si el entorno lo permite,
- rotación y respaldo de claves en un KMS o almacenamiento seguro del sistema.

La regla clave es que ni el cliente ni la infraestructura deben asumir que un archivo almacenado es seguro solo porque está en un disco local o en un NAS con permisos básicos.

## 6. Metadatos y seguridad operativa

Los metadatos operativos de V1 pueden ser visibles para el servidor, pero deben protegerse con:
- acceso restringido,
- validación por sesión,
- trazabilidad por usuario y dispositivo,
- mínimo necesario para operar.

Esto prepara el camino a V2, donde metadatos sensibles deben separarse aún más y compactarse con capas de cifrado adicionales.

## 7. Recuperación y restauración

- Las copias de seguridad deben ser independientes del almacenamiento activo.
- Cada operación debe poder ser reconstruida a partir del estado de metadatos y la cola local/servidor.
- Los conflictos no deben destruir la alternativa antes de la confirmación del usuario.
- La restauración de un archivo debe validar checksum y revisar el último estado sincronizado.

## 8. Criterios de aceptación

El diseño de almacenamiento se considera correcto si:
- los archivos y metadatos no quedan expuestos sin protección,
- la persistencia local y del servidor soporta reintentos y recuperación,
- los conflictos conservan la versión alternativa,
- la arquitectura puede evolucionar a V2 sin reenfocar la capa de almacenamiento desde cero.

