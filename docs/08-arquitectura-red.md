# Arquitectura de Red y Comunicaciones

## 1. Objetivo y alcance

Esta capa define cómo los clientes se conectan al servidor de SyncFiles, cómo se transportan los metadatos y archivos, qué políticas de reintento y seguridad aplica la red y cómo se preparan estos mecanismos para la evolución a V2.

La decisión de V1 es deliberada y consistente con la arquitectura general:
- clientes y servidor interactúan en un modelo cliente-servidor simple,
- la sincronización usa REST/JSON sobre HTTPS,
- la coordinación se hace con polling,
- no se exige WebSocket ni gRPC en V1,
- el NAS y la infraestructura distribuida no forman parte del MVP.

## 2. Principios de diseño

- El canal de comunicaciones es siempre autenticado y cifrado.
- El servidor es la única autoridad operativa del estado.
- El cliente debe tolerar fallos transitorios sin perder la intención de la operación.
- Las operaciones deben ser idempotentes a nivel de request.
- La red no debe ofrecer más complejidad de la que el MVP necesita.
- La arquitectura debe dejar margen para V2 sin romper V1.

## 3. Topología de red de V1

### 3.1 Modelo operativo

La topología de V1 es una arquitectura centralizada con una sola instancia operativa del backend:

- cada cliente se conecta al mismo endpoint del servidor,
- el servidor expone un API REST/JSON,
- el cliente sincroniza mediante polling de cambios,
- los archivos se transfieren como payloads HTTP normalizados,
- la coordinación de conflictos y metadatos ocurre en el servidor.

### 3.2 Diagrama lógico

Cliente Desktop / Android
      |  HTTPS + TLS 1.3 + REST/JSON
      v
Servidor SyncFiles (single-node)
      |  SQLite + almacenamiento local
      v
Archivos + metadatos + cola + auditoría

### 3.3 Suposiciones de red

- Las sesiones se gestionan con tokens o cookies de sesión autorizados por el backend.
- Los clientes deben poder operar en redes domésticas y móviles con latencias moderadas.
- Se acepta un modelo de sincronización con reintentos y backoff.
- No se exige comunicación directa entre dispositivos.
- El servidor es el único punto de coordinación.

## 4. Protocolos y transporte

### 4.1 Protocolo principal

- Protocolo: HTTP/1.1 o HTTP/2 sobre TLS 1.3
- Encapsulado de datos: JSON por API, binario para archivos
- Autenticación: token de sesión o bearer token con validación server-side
- Integridad: checksum por archivo y por chunk cuando aplique
- Política de compresión: opcional y controlada; no obligatorio en V1

### 4.2 Endpoints funcionales

Los clientes contactan con endpoints dedicados a:
- autenticación y sesión,
- lista de archivos y metadatos,
- pull de cambios,
- upload de archivos,
- delete/rename/move/copy,
- resolución de conflictos,
- auditoría y estado del sistema.

### 4.3 Formato de request/response

- JSON para metadatos y estado.
- Content-Type: application/json para payloads de control.
- Multipart o streaming para transferencias de archivos por tamaño.
- Estructuras con `idempotency_key` para evitar efectos dobles en reintentos.

## 5. Flujo de sincronización sobre la red

### 5.1 Login y sesión

1. El cliente presenta credenciales del usuario.
2. El servidor valida identidad y emite una sesión vinculada a dispositivo.
3. El servidor devuelve un token y un `device_id` válido.
4. El cliente almacena la sesión en un almacén seguro local.
5. Las peticiones posteriores usan el token y la identidad del dispositivo.

### 5.2 Polling de cambios

- El cliente consulta periódicamente al servidor por deltas.
- El servidor responde con metadatos actualizados desde la última versión conocida.
- La estrategia de polling puede ser fija o adaptativa.
- El polling se acopla a la cola local de cambios para minimizar saturación y latencia.

### 5.3 Upload local

1. El cliente detecta un cambio local y lo encola.
2. Persiste la operación antes de enviarla.
3. Envía el archivo o el delta con el `idempotency_key`.
4. El servidor valida la operación, checksum y permisos.
5. El servidor acepta, rechaza o declara conflicto.
6. El cliente actualiza el estado local según la respuesta.

### 5.4 Download remoto

1. El cliente solicita la lista de cambios pendientes.
2. El servidor devuelve archivos y metadatos relevantes.
3. El cliente descarga el contenido, valida checksum y aplica la operación local.
4. El sistema actualiza la marca `synced_at` y el estado local.

### 5.5 Manejo de conflictos

En caso de modificación simultánea:
- el servidor compara `modified_at` y `checksum`,
- si hay conflicto real, el sistema no sobrescribe silenciosamente,
- el cliente recibe una señal de conflicto con información del archivo alternativo,
- el usuario decide si mantener local, remoto o conservar ambos precedentes.

## 6. Política de tolerancia a fallos de red

### 6.1 Reintentos

Los fallos transitorios se manejan con:
- backoff exponencial,
- límite de reintentos por operación,
- `idempotency_key` para evitar duplicados,
- cola persistida en disco antes del envío.

### 6.2 Errores previstos

- 401/403: sesión inválida o permisos insuficientes.
- 408/429: timeout o rate limit.
- 409: conflicto de actualización.
- 5xx: fallo temporal del backend.
- corte de red: se conserva el estado y la intención en cola.

### 6.3 Política de consistencia

La regla general es:
- nunca asumir que una operación fallida está completa,
- si no existe confirmación del servidor, el cliente mantiene el estado en pendiente,
- la operación se reintenta o se declara explícitamente fallida,
- el usuario no ve un estado de sincronización falso.

## 7. Seguridad de la capa de red

- TLS 1.3 obligatorio.
- Validación estricta de certificados.
- Tokens con expiración y revocación.
- Sesión vinculada a `device_id` y `user_id`.
- Logs mínimos para diagnóstico sin exponer contenidos sensibles.
- No se permite comunicación sin autenticación ni sin TLS.

## 8. Evolución hacia V2

La evolución planeada no cambia la base central del diseño, pero habilita un patrón más avanzado:

- NAS como capa de almacenamiento persistente.
- PostgreSQL como base documental/relacional con mejores garantías.
- Kafka como bus de eventos para flujo asíncrono.
- WebSocket o gRPC como canales de tiempo real en fases posteriores.
- E2E como capa adicional sobre archivos y metadatos sensibles.

La transición a V2 debe hacerse con la base funcional de V1 estabilizada, sin reescribir el contrato básico de sincronización.

## 9. Criterios de aceptación

Se considera correcto el diseño de red de V1 cuando:
- todos los clientes se conectan con TLS 1.3,
- la sincronización funciona con polling sin requerir WebSocket,
- la cola local es persistente y recuperable tras reinicio,
- reintentos y conflictos no generan duplicados ni estados inconsistentes,
- la arquitectura admite una evolución limpia a NAS + PostgreSQL + Kafka sin reescribir el modelo core.
