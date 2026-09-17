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

### 7.1 Responsables por Riesgo (Owner)

| Riesgo | Owner | Role |
|--------|-------|------|
| RT-01 (Pérdida de Datos) | Lead Ops | Operaciones / SRE |
| RT-02 (Vulnerabilidad Encriptación) | Security Lead | Seguridad |
| RT-03 (Sincronización Ineficiente) | Tech Lead | Backend |
| RT-04 (Escalabilidad Insuficiente) | CTO | Arquitectura |
| RS-01 (Acceso No Autorizado) | Security Lead | Seguridad |
| RS-02 (Ataques MITM) | Security Lead | Seguridad |
| RS-03 (Exposición de Metadatos) | Security Lead | Seguridad |
| RO-01 (Disponibilidad Servicio) | Lead Ops | SRE / Infra |
| RO-02 (Complejidad Operaciones) | Lead Ops | SRE / Docs |
| RO-03 (Retrasos Kafka) | CTO | Arquitectura |
| RC-01 (Incumplimiento normativo) | CTO | Compliance |

### 7.2 Umbrales de Activación y Severidad (SEV)

| SEV | Condición | SLA de respuesta | SLA de resolución |
|-----|-----------|-------------------|-------------------|
| **SEV1** | Server caído; pérdida de datos sin backup disponible; breach de seguridad activo | 15 min | 4 h (post-mortem ≤ 24 h) |
| **SEV2** | >5% de errores 5xx en 10 min; sync_rate < 50% del baseline; backup fallido 24h | 30 min | 4 h |
| **SEV3** | >10% de warnings en logs; rate limit al 50%; un solo usuario afectado | 2 h | 24 h |

**Umbrales numéricos:**
- `sync_failed_operations_total` crece > 200% en 5 min → SEV2
- `/health/ready` responde 503 > 30 s → SEV1
- `active_sessions_count` anómalo (10× baseline) → SEV3 (posible sesión colgada)
- `conflict_rate` > 5% → SEV3
- `retry_rate` > 10% → SEV3

### 7.3 Procedimientos de Contención por SEV

#### SEV1: Contener → Comunicar → Recuperar → Post-mortem (≤ 24 h)

1. **Contener:**
   - Identificar alcance: ¿server caído? ¿data loss? ¿breach?
   - Si breach: desconectar endpoint afectado, bloquear IP sospechosa.
   - Si data loss: detener escrituras (`SF_READONLY=1`).
2. **Comunicar:**
   - Notificar al equipo de emergencia (Slack #incidents).
   - Si afecta a usuarios: publicar status page (GitHub Discussions).
3. **Recuperar:**
   - Restore desde backup más reciente (ver §7.4).
   - Verificar integridad con `PRAGMA integrity_check`.
   - Reiniciar server, validar `/health/ready`.
4. **Post-mortem (≤ 24 h):**
   - Documentar causa raíz, timeline, acciones correctivas.
   - Publicar en `docs/postmortems/YYYY-MM-DD-sev1.md`.

#### SEV2: Diagnosticar → Mitigar → Documentar (≤ 4 h)

1. **Diagnosticar:** revisar logs estructurados (NDJSON) con `request_id` correlation.
2. **Mitigar:** revisar rate limiter, limpiar sync_queue, forzar re-sync.
3. **Documentar:** actualizar runbook si la mitigación es novedosa.

#### SEV3: Registrar → Priorizar

1. **Registrar** en backlog de tech debt.
2. **Priorizar** en el planning semanal.

### 7.4 Procedimiento de Backup/Restore (referencia: docs/36)

- **Backup diario:** `scripts/backup.sh` (cron 2 AM). Ver `docs/36-backup-y-restauracion.md`.
- **Backup manual:** `POST /admin/backup` (protegido por `SF_ADMIN_TOKEN`).
- **Restore:** procedimiento en `docs/36-backup-y-restauracion.md` §6.
- **Test E2E:** `scripts/e2e-backup-restore.sh` (ejecutar mensualmente).

### 7.5 Contactos y Escalamiento

| Rol | Contacto | Canales |
|-----|----------|---------|
| Lead Ops (SRE) | `<leadops@syncfiles.local>` | Slack #sre, SMS 24/7 |
| Security Lead | `<security@syncfiles.local>` | Slack #security, email prioritario |
| CTO | `<cto@syncfiles.local>` | Slack #cto, llamada si SEV1 |
| On-call rotativo | Ver `docs/oncall.md` | Página 24/7 via PagerDuty |
| Usuarios / Soporte | `<support@syncfiles.local>` | GitHub Issues, email |

**Escalamiento SEV1:** Lead Ops → CTO (5 min) → llamada de emergencia.
**Escalamiento SEV2:** Lead Ops → Tech Lead (30 min).
**Escalamiento SEV3:** Registro en backlog, prioridad en planning.

### 7.6 Métricas de Efectividad de Mitigaciones

| Métrica | Target | Medición |
|---------|--------|----------|
| MTTR SEV1 | < 4 h | Desde detección hasta servicio restaurado |
| MTTR SEV2 | < 4 h | Desde detección hasta mitigación |
| Backup success rate | 99.5% | Fracción de backups exitosos / total |
| Restore test pass rate | 100% | E2E restore test (mensual) |
| Meantime to detection (MTTD) | < 5 min | Tiempo desde incidente hasta alerta |
| Post-mortem completion | ≤ 24 h (SEV1) | Fracción de post-mortems dentro de SLA |

## 8. Revisión y Mejora Continua
- Revisión mensual de matriz de riesgos.
- Actualización tras incidentes relevantes.
- Simulacros trimestrales (fallo NAS, replay de eventos, pérdida de conectividad).
