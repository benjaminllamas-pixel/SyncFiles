# Configuración Inicial (Onboarding)

## 1. Objetivo

Este documento define el flujo de configuración inicial del cliente y del servidor de SyncFiles para V1.

La idea es simplificar la puesta en marcha y cerrar la brecha entre instalación, autenticación y primer sincronización, sin introducir complejidad accidental ni dependencias de infraestructura que no existan en V1.

## 2. Principios de onboarding

- toda configuración debe ser reproducible,
- la primera sincronización debe ser determinista,
- el cliente no debe requerir un entorno de red complejo para funcionar,
- el usuario debe reconocer claramente qué se está sincronizando y cuál es el estado actual,
- la configuración debe dejar espacio para la evolución a V2 sin reescribir el flujo.

## 3. Flujo de configuración del cliente

### 3.1 Primer arranque

Al abrir la app por primera vez, el cliente debe:
1. detectar el sistema operativo y la plataforma,
2. verificar que el entorno cumple requisitos mínimos,
3. comprobar que existe un directorio de sincronización válido,
4. crear la base SQLite local para metadatos,
5. generar o recuperar un `device_id` estable,
6. pedir credenciales del usuario y/o token de sesión,
7. validar la conexión con el backend,
8. ejecutar una primera sincronización de prueba o estado inicial.

### 3.2 Selección del directorio sincronizado

El usuario debe elegir:
- una carpeta local dentro de su perfil o de un directorio de trabajo,
- una ruta segura y accesible por el cliente,
- una política de sincronización clara para archivos ya existentes.

El sistema debe ofrecer una opción de:
- sincronizar todo desde cero,
- importar una estructura existente,
- conservar archivos ya presentes y tratar diferencias reales como conflicto.

### 3.3 Perfil de sesión

Durante la primera configuración se deben registrar:
- `user_id`,
- `device_id`,
- nombre del dispositivo,
- sistema operativo y versión,
- nombre del directorio sincronizado,
- nivel de seguridad (por defecto: V1 standard),
- token o sesión activa.

## 4. Configuración del servidor

### 4.1 Requisitos mínimos de infraestructura

En V1 el entorno debe tener:
- una máquina o servicio para el backend,
- SQLite operando con acceso local o protegido,
- almacenamiento local para archivos,
- certificado TLS 1.3 válido,
- acceso a una red donde los clientes puedan conectarse de forma estable.

### 4.2 Variables de configuración

El servidor debe disponer de un conjunto mínimo de configuración:
- `APP_ENV`
- `DB_PATH`
- `FILE_STORAGE_PATH`
- `TLS_CERT_PATH`
- `TLS_KEY_PATH`
- `SESSION_TTL_SECONDS`
- `MAX_RETRY_ATTEMPTS`
- `POLLING_INTERVAL_SECONDS`
- `AUDIT_RETENTION_DAYS`

### 4.3 Autenticación inicial

El usuario final no se registra desde el cliente en V1. El sistema asume:
- cuenta ya creada en backend,
- credenciales administradas por el operador o por un flujo de provisioning externo,
- un dispositivo de confianza asociado a la cuenta durante el onboarding.

## 5. Primer sincronización

Al finalizar la configuración inicial:
1. el cliente hace `GET /api/v1/files/changes` para sincronizar el estado remoto,
2. se crea la tabla o estado local de archivos,
3. se permite la primera sincronización no destructiva,
4. el sistema deja un paquete de estado inicial auditable,
5. la app pasa a estado “ready” y entra en polling normal.

## 6. Manejo de situaciones no normales

### 6.1 Ruta inválida o no accesible

El cliente debe:
- mostrar error claro,
- permitir elegir otra carpeta,
- no iniciar sincronización si la ruta provoca riesgo de corrupción.

### 6.2 Certificado TLS inválido

El cliente debe bloquear actividades remotas y pedir reconfiguración del entorno.

### 6.3 Sesión expirada

Debe solicitar re-login sin perder el estado local de cola y de metadatos.

## 7. Configuración para Android y desktop

La experiencia de onboarding debe ser consistente en todas las plataformas:
- mismo principio de ruta local sincronizada,
- mismo flujo de sesión,
- mismo manejo de conflicto,
- mismo estado visible para el usuario.

La diferencia está en cómo cada plataforma gestiona:
- permisos del sistema,
- almacenamiento seguro local,
- notificaciones,
- persistencia de sesión.

## 8. Criterios de aceptación

La configuración inicial se considera correcta si:
- el usuario puede instalar y configurar la app sin ayuda técnica,
- el cliente valida la conexión con el backend antes de comenzar a escribir,
- la primera sincronización no borra ni sobreescribe archivos sin consentimiento,
- la app puede recuperarse tras reinicio o sesión vencida,
- el entorno es reproducible y documentado para soporte técnico.

## 9. Recomendación final

El onboarding de V1 debe ser corto, seguro y visible. Si el usuario sabe qué cambió, qué está pendiente y qué se está sincronizando, la ganancia operativa es mucho mayor que cualquier complejidad extra en el wizard. La base del onboarding debe permitir V2 sin reescribir el proceso central.
