# Plan: Reevaluación de Items Archify Ignorados para V2/V3

> **Contexto:** Archify report (score 54/D+) marcó 12 items como "ignore/defer" con justificación. V2 (NAS, PostgreSQL, Kafka, E2E, multi-usuario) y V3 (escala/HA) cambian la premisa: muchos ahora aplican.

---

## 1. Mapeo de Items a Fases V2/V3

| Item Archify | Estado Original | ¿Aplica en V2? | ¿Aplica en V3? | Fase Sugerida |
|--------------|-----------------|----------------|----------------|---------------|
| **Authorization (RBAC)** | Grade D- — "wait until multi-tenant" | ✅ **SÍ** — carpetas compartidas / multi-usuario | ✅ | V2 |
| **Authenticity (MFA)** | Grade D- — "wait until user demand" | ✅ **SÍ** — E2E encryption exige auth fuerte | ✅ | V2 |
| **Privacy** | Grade F — "no PII beyond credentials" | ⚠️ **CONDICIONAL** — si V2 añade cuentas/perfil | ✅ | V2/V3 |
| **Compliance** | Grade F — "no regulatory req identified" | ⚠️ **CONDICIONAL** — si target enterprise/GDPR | ✅ | V2/V3 |
| **Governance** | Grade F — "branch protection can wait until team grows" | ✅ **SÍ** — CI/CD hardening, branch rules | ✅ | V2 |
| **Business Continuity** | N/S — "not scored, no DR planning" | ⚠️ **CONDICIONAL** — si deploy multi-instancia/producción | ✅ | V3 |
| **Backward Compatibility** | N/A — "not applicable" | ✅ **SÍ** — API versioning para clientes V1/V2 | ✅ | V2 |
| **Non-repudiation** | Grade F — "not needed for file sync" | ⚠️ **CONDICIONAL** — auditoría empresarial | ✅ | V3 |
| **Elasticity** | N/A — "not applicable" | ⚠️ **CONDICIONAL** — auto-scaling si Kafka/workers | ✅ | V3/Phase 4 |
| **Redundancy** | N/A — "not applicable" | ⚠️ **CONDICIONAL** — HA PostgreSQL/NAS | ✅ | V3 |
| **Responsiveness** | N/A — "CLI/desktop focus" | ⚠️ **CONDICIONAL** — web dashboard responsive | ❌ | — |
| **Internationalization** | N/A — "not required" | ❌ **NO** — fuera de scope V2/V3 | ❌ | — |

---

## 2. Decisiones a Resolver (Preguntas al Usuario)

### 2.1 Multi-usuario / Compartición (Desbloquea RBAC, Privacy, Compliance)
- **Pregunta:** ¿V2 incluirá carpetas compartidas entre usuarios (invitar, permisos read/write)?
- **Recomendación:** **Sí** — es el paso natural tras "single user V1". Permite RBAC granular (owner/editor/viewer) y activa Privacy/Compliance.
- **Impacto:** Requiere modelo de datos `User`, `FolderShare`, `Permission`; endpoint `POST /shares`, `GET /shares`; UI web para invitar.

### 2.2 Autenticación Fuerte / MFA (Desbloquea Authenticity)
- **Pregunta:** ¿MFA (TOTP/WebAuthn) obligatorio u opcional en V2?
- **Recomendación:** **Opcional en V2, obligatorio para cuentas con E2E** — implementar TOTP primero, WebAuthn en V3.
- **Impacto:** `Authenticator` trait, tabla `mfa_credentials`, endpoint `/auth/mfa/*`, recovery codes.

### 2.3 Encriptación E2E (Desbloquea Non-repudiation, Privacy)
- **Pregunta:** ¿E2E por archivo (clave derivada de passphrase usuario) o por carpeta compartida (clave por share)?
- **Recomendación:** **Por carpeta (share key)** — compatible con sharing; clave de usuario deriva clave de carpetas propias.
- **Impacto:** `syncfiles-crypto` crate nuevo; `FileEntry` añade `encrypted: bool`, `key_id`; cliente cifra/descifra; server nunca ve plaintext.

### 2.4 API Versioning (Desbloquea Backward Compatibility)
- **Pregunta:** ¿Estrategia de versioning: URL (`/api/v2/`) o header (`Accept: application/vnd.syncfiles.v2+json`)?
- **Recomendación:** **URL v2** — más simple para clientes; `/api/v1/` congelado, `/api/v2/` para E2E, shares, WebSocket.
- **Impacto:** Router versionado, migración gradual, `v1` deprecated tras 2 releases.

### 2.5 Enterprise / Compliance Target (Desbloquea Compliance, Business Continuity, Non-repudiation)
- **Pregunta:** ¿V2 apunta a clientes empresariales (GDPR, SOC2, auditoría)?
- **Recomendación:** **MVP enterprise en V3** — V2 prepara auditoría (log inmutable, export), V3 añade GDPR (derecho al olvido, portabilidad), SOC2 controls.
- **Impacto:** `AuditLog` append-only, `DataExport` job, `RetentionPolicy`, `GDPRController`.

### 2.6 Governance & CI/CD Hardening (Desbloquea Governance)
- **Pregunta:** ¿Reglas de branch protection + required reviews + status checks obligatorios en `main`?
- **Recomendación:** **Sí inmediato** — `main` protegido, PR require 1 review + CI pass + clippy pass; `develop` branch para features.
- **Impacto:** GitHub branch protection rules, CODEOWNERS, dependabot/renovate.

### 2.7 HA / DR / Elasticity (Desbloquea Business Continuity, Redundancy, Elasticity)
- **Pregunta:** ¿V3 requiere HA activo-activo (múltiples server pods + PostgreSQL Patroni + NAS replicado)?
- **Recomendación:** **PostgreSQL Patroni + S3-compatible NAS replicado** — worker Kafka stateless escala horizontal; server stateless tras E2E (clave en cliente).
- **Impacto:** Docker/K8s manifests, Helm chart, health/readiness probes ya existen, `leader election` para workers.

---

## 3. Plan de Acción por Fases

### V2 (Core: NAS, PostgreSQL, Kafka, E2E, Multi-usuario)
| Tarea | Dependencia | Owner Sugerido |
|-------|-------------|----------------|
| A1. Modelo multi-usuario + shares + RBAC | 2.1 Sí | Backend |
| A2. E2E encryption (share-key + user-key) | 2.3 Por carpeta | Crypto + Client |
| A3. MFA TOTP (opcional) | 2.2 Opcional | Auth |
| A4. API v2 router + versioning | 2.4 URL v2 | Backend |
| A5. Branch protection + dependabot | 2.6 Inmediato | DevOps |
| A6. Audit log inmutable + export JSON | 2.5 MVP enterprise | Backend |

### V3 (Enterprise/HA/Scale)
| Tarea | Dependencia | Owner Sugerido |
|-------|-------------|----------------|
| B1. PostgreSQL Patroni HA + replication | 2.7 HA | DevOps/DB |
| B2. NAS S3-replicated + failover | 2.7 HA | DevOps/Infra |
| B3. GDPR: right to erasure, portability | 2.5 Enterprise | Backend/Legal |
| B4. WebAuthn MFA + recovery codes | 2.2 MFA | Auth |
| B5. Auto-scaling workers (KEDA/K8s HPA) | 2.7 Elasticity | DevOps |
| B6. Audit log inmutable (WORM/S3 Object Lock) | 2.5 Compliance | Backend |
| B7. API v2 → v3 migration path | 2.4 Backward compat | Backend |

---

## 4. Validación y Criterios de Salida

| Ítem | Criterio | Métrica |
|------|----------|---------|
| RBAC | Invitar usuario, asignar rol, revocar funciona E2E | 100% tests `shares/*` pass |
| MFA | TOTP login + recovery codes | 100% tests `auth/mfa/*` pass |
| E2E | Server nunca ve plaintext; cliente cifra/descifra round-trip | `e2e-e2e-encryption.sh` 100% |
| API v2 | Cliente V1 sigue funcionando; V2 usa nuevos endpoints | `cargo test --features v1_compat` |
| Branch protection | PR sin review/CI fail bloqueado en `main` | GitHub status check |
| HA PostgreSQL | Failover < 30s sin pérdida datos | `pgbench` + chaos test |
| GDPR export/erase | `GET /gdpr/export`, `DELETE /gdpr/erase` en < 5s | E2E test |

---

## 5. Riesgos y Mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigación |
|--------|--------------|---------|------------|
| Scope creep V2 (multi-usuario + E2E + NAS simultáneo) | Alta | Retraso V2 | **Separar**: V2.0 = NAS + PostgreSQL + Kafka; V2.1 = E2E + multi-usuario |
| E2E rompe conflict resolution (server no ve contenido) | Media | Data loss | **Design**: conflict resolution client-side con claves derivadas; server solo metadata |
| PostgreSQL migration rompe clientes V1 | Alta | Breaking change | **Migration tool** offline + dual-write transitorio; feature flag `use_pg` |
| MFA recovery codes perdidos = account lockout | Media | Soporte | **Recovery codes** obligatorios al activar MFA; admin override en V3 |

---

## 6. Preguntas Abiertas (Resolver Antes de Implementar)

1. **Multi-usuario en V2: ¿Sí/No?** (define RBAC, Privacy, Compliance, Shares)
2. **E2E scope: ¿por archivo o por carpeta/share?** (define crypto model, key rotation)
3. **MFA: ¿TOTP only o TOTP + WebAuthn en V2?** (WebAuthn añade dependencia `webauthn-rs`)
4. **API versioning: ¿URL path o header?** (afecta todos los clientes)
5. **Enterprise target V2 vs V3?** (define si Compliance/BC/Non-repudiation entran en V2)
6. **HA V3: ¿Kubernetes o VMs + Patroni?** (define infra, tooling, skills)

---

## 7. Próximos Pasos Inmediatos

1. **Resolver preguntas 1-4 arriba** (bloquean diseño V2)
2. **Crear issue GitHub por cada decisión** con `design` label
3. **Actualizar `docs/49-roadmap.md`** con decisiones tomadas
4. **Crear plan detallado V2.0** (`1789448560089-v2-core-plan.md`) una vez resueltas 1-4
5. **Implementar Governance (A5) ya** — no bloquea nada, mejora inmediato

---

*Plan generado para discusión. Ejecutar `plan_exit` cuando preguntas 1-4 estén resueltas.*