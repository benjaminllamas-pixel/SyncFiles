# Infraestructura NAS (Network Attached Storage)

## 1. Objetivo

Este documento define la capa de almacenamiento compartido que queda planeada para V2 y que no forma parte de la base operativa de V1.

La adopción de NAS en SyncFiles responde a la necesidad de:
- almacenamiento persistente centralizado,
- mayor capacidad y crecimiento en volumen,
- organización y acceso más controlado para varios dispositivos,
- preparación para una infraestructura más robusta y escalable.

## 2. Principio de diseño

La arquitectura de V1 usa almacenamiento local por diseño. El NAS se incorpora como evolución posterior, con la intención de mantener el mismo contrato funcional.

El NAS no debe ser un reemplazo improvisado del almacenamiento local. Debe ser una capa que:
- preserve la abstracción de `StorageProvider`,
- soporte acceso controlado desde el backend,
- permita migración segura y reversión controlada,
- mantenga checksum, integridad y auditoría.

## 3. Modelo operativo esperado en V2

### 3.1 Topología

- servidor principal con acceso al NAS,
- almacenamiento compartido como capa de persistencia central,
- SQLite o PostgreSQL como base de metadatos según la fase de evolución,
- fallback temporal hacia almacenamiento local en caso de degradación del NAS.

### 3.2 Flujos operativos

- el backend accede al NAS a través del `StorageGateway`,
- las rutas físicas quedan encapsuladas bajo una capa de abstracción,
- los clientes no interactúan directamente con el NAS,
- la consistencia y la integridad se validan con checksum y auditoría.

## 4. Hardware recomendado

Se recomienda un NAS basado en los siguientes criterios:
- RAID 1 o RAID 5 según volumen y tolerancia al fallo,
- unidades SSD NVMe o HDD enterprise según costo y velocidad,
- UPS con protección de energía,
- monitorización de salud del disco, temperatura y estado de RAID,
- red local con alta disponibilidad y rendimiento estable.

## 5. Seguridad del NAS

- acceso exclusivo desde el servidor o desde red privada controlada,
- autenticación con credenciales del servicio y no de usuario final,
- almacenamiento bajo control del backend,
- cifrado de reposo si el entorno lo permite,
- control de permisos y auditoría de acceso por usuario y acción.

## 6. Fallback y recuperación

En una infraestructura con NAS:
- si el NAS falla, el sistema debe caer a almacenamiento local temporal,
- la operación debe quedar marcada como pendiente o reintentos,
- la reconciliación posterior debe hacerse de forma controlada,
- no se debe aceptar una situación en la que el sistema “pierde” un archivo sin trazabilidad.

## 7. Requisitos de migración

La migración LocalDisk → NAS debe realizarse en fases:
1. validación del layout físico,
2. pruebas de checksum y consistencia,
3. compatibilidad de metadatos,
4. prueba de failover,
5. switchover controlado,
6. revalidación posterior con auditoría.

## 8. Criterios de entrada a V2

El NAS se considera apropiado cuando:
- la V1 ya está estabilizada,
- el uso real requiere almacenamiento compartido o más capacidad,
- la base de metadatos está lista para una evolución a PostgreSQL,
- el sistema ya tiene una política de conflicto y reintentos bien validada.

## 9. Recomendación final

NAS no es un requisito previo para V1. Es una evolución natural y ordenada que se activa cuando el producto ya está completo y se requiere capacidad, resiliencia y diseño más avanzado. La clave es que el sistema ya debe haber definido una abstracción clara para que la transición sea segura y compatible.
