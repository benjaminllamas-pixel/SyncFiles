# CI/CD Pipeline

## 1. Objetivo

El pipeline de CI/CD de SyncFiles debe permitir entregas rápidas, seguras y reproducibles del software, sin comprometer la estabilidad del sistema ni la confianza del usuario.

En V1, el pipeline debe ser limpio y alineado con la fase de diseño-implementación:
- validación automática por cada cambio,
- pruebas de integración y seguridad,
- release controlado con rollback,
- despliegue simple del backend y los clientes.

## 2. Git workflow recomendado

- rama `main` como rama de integración estable,
- ramas de feature para cambios funcionales,
- ramas de release para cerrar una versión candidata,
- tags para versiones oficiales,
- merge solo mediante validación y revisión.

## 3. Pipeline de CI

### 3.1 Stage 1: validate

- checkout
- instalación de dependencias
- lint / type-check / build
- validación de formato y estructuras relevantes

### 3.2 Stage 2: unit tests

- pruebas unitarias de sincronización,
- validación de path normalization,
- lógica de conflicto,
- checksum, session y queue behavior.

### 3.3 Stage 3: integration tests

- servidor + SQLite + cliente mock,
- escenario completo de upload/download,
- polling de cambios,
- conflicto con resolución por usuario,
- reintentos y recuperación ante fallo de red.

### 3.4 Stage 4: security checks

- escaneo de dependencias,
- revisión de configuración TLS,
- validación de secretos y variables sensibles,
- pruebas automatizadas de sesión y autorización.

## 4. Pipeline de CD

### 4.1 Release candidates

Los cambios para release deben pasar por:
- build reproducible,
- pruebas de integración,
- pruebas de seguridad,
- pruebas de rendimiento mínimas,
- aprobación manual o por equipo técnico.

### 4.2 Deploy de backend

- despliegue controlado con entorno de staging,
- validación de API,
- smoke tests,
- promesa a producción solo si pasa control de calidad.

### 4.3 Deploy de clientes

- generar artefactos firmados,
- publicar en canal apropiado,
- verificar instalación y auto-update,
- activar rollout gradual si el producto así lo requiere.

## 5. Rollback y recuperación

- cada release debe tener un rollback documentado,
- no se debe liberar una versión sin backup de configuración o artefactos,
- un problema crítico debe revertirse por versionado, no por ad hoc fixes,
- los cambios operativos deben registrarse con trazabilidad.

## 6. Observabilidad del pipeline

- métricas del tiempo de build y test,
- tasa de fallos por etapa,
- registro de artefactos,
- revisión de logs para cada release,
- alertas si una etapa crítica tarda o falla repetidamente.

## 7. Reglas de calidad y gobernanza

- todo PR requiere validación de CI antes de merge,
- no se envía a producción un release sin prueba de humo,
- no se aceptan cambios de seguridad sin revisión y evidencia,
- cualquier cambio de comportamiento relevante debe incluir pruebas de regresión.

## 8. Criterios de aceptación

El pipeline de CI/CD es correcto si:
- cada cambio se valida automáticamente,
- la calidad del software se mantiene estable,
- los releases se pueden desplegar y revertir sin caos,
- la arquitectura y las pruebas permiten evolución segura a V2.

## 9. Recomendación final

La mejor estrategia para SyncFiles es un pipeline simple, transparente y automatizado: CI fuerte, CD controlado y rollback documentado. Eso permite mantener V1 estable, entregar valor con seguridad y preparar la evolución a V2 sin crear fricción innecesaria en el equipo o en la operación.
