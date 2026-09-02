# Análisis de Riesgos

## 1. Objetivo
Identificar, priorizar y mitigar riesgos técnicos, de seguridad y operacionales de SyncFiles.

## 2. Criterios de Evaluación
- **Severidad:** Crítica / Alta / Media / Baja
- **Probabilidad:** Alta / Media / Baja
- **Prioridad:** combinación de severidad y probabilidad.

## 3. Riesgos Técnicos
### RT-01: Pérdida de Datos
**Severidad:** Crítica  
**Probabilidad:** Media  
**Mitigación:** backup redundante en NAS, versionado, validación por checksum.

### RT-02: Vulnerabilidad de Encriptación
**Severidad:** Crítica  
**Probabilidad:** Baja  
**Mitigación:** auditoría externa, librerías probadas, rotación y actualización de algoritmos.

### RT-03: Sincronización Ineficiente
**Severidad:** Alta  
**Probabilidad:** Media  
**Mitigación:** delta sync, compresión, sincronización incremental y batching.

### RT-04: Escalabilidad Insuficiente
**Severidad:** Alta  
**Probabilidad:** Media  
**Mitigación:** escalado horizontal, colas/eventos con Kafka, particionado de carga.

## 4. Riesgos de Seguridad
### RS-01: Acceso No Autorizado
**Severidad:** Crítica  
**Probabilidad:** Media  
**Mitigación:** MFA, rate limiting, políticas de sesión, monitoreo de anomalías.

### RS-02: Ataques MITM
**Severidad:** Alta  
**Probabilidad:** Baja  
**Mitigación:** TLS 1.3, pinning de certificados, validación estricta de integridad.

### RS-03: Exposición de Metadatos Sensibles
**Severidad:** Alta  
**Probabilidad:** Media  
**Mitigación:** minimización de metadatos, cifrado de campos sensibles, políticas de retención.

## 5. Riesgos Operacionales
### RO-01: Disponibilidad del Servicio
**Severidad:** Alta  
**Probabilidad:** Media  
**Mitigación:** infraestructura redundante, failover automático, pruebas de recuperación.

### RO-02: Complejidad de Operaciones
**Severidad:** Media  
**Probabilidad:** Alta  
**Mitigación:** documentación viva, alertas accionables, runbooks.

### RO-03: Retrasos o Saturación en Kafka
**Severidad:** Alta  
**Probabilidad:** Media  
**Mitigación:** particionado adecuado, control de lag, DLQ, autoscaling de consumidores.

## 6. Riesgos de Cumplimiento
### RC-01: Incumplimiento normativo (privacidad/datos)
**Severidad:** Alta  
**Probabilidad:** Media  
**Mitigación:** clasificación de datos, trazabilidad, controles de acceso y retención regulada.

## 7. Plan de Respuesta
- Definir responsables por riesgo (owner).
- Umbrales de activación y escalamiento.
- Procedimientos de contención, recuperación y postmortem.
- Métricas de efectividad de mitigaciones.

## 8. Revisión y Mejora Continua
- Revisión mensual de matriz de riesgos.
- Actualización tras incidentes relevantes.
- Simulacros trimestrales (fallo NAS, replay de eventos, pérdida de conectividad).
