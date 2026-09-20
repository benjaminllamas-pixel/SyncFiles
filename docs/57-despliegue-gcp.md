# Despliegue mínimo en Google Cloud Run

## Alcance V1

Cloud Run es stateless. Para V1 se monta un **Persistent Disk** en `/data` y se fuerza `concurrency=1`, porque SQLite no admite varios writers. El servicio expone HTTP; la autenticación de SyncFiles protege la API, pero se recomienda restringir la URL con IAP, Load Balancer o una VPN para producción.

## Requisitos

- Proyecto GCP y región seleccionados.
- Imagen publicada en Artifact Registry, por ejemplo `us-central1-docker.pkg.dev/PROYECTO/syncfiles/server:v1`.
- Permiso para crear Cloud Run services/jobs y Persistent Disk.
- `SF_ADMIN_TOKEN`, `SF_USER_0_EMAIL`, `SF_USER_0_PASSWORD` y `SF_USER_0_ID`.
- Revisar precios de Cloud Run, Persistent Disk y tráfico antes de desplegar; no se asume free tier.

## Despliegue

```bash
export GCP_PROJECT=mi-proyecto
export GCP_REGION=us-central1
export GCP_SERVICE=syncfiles
export GCP_IMAGE=us-central1-docker.pkg.dev/mi-proyecto/syncfiles/server:v1
export SF_ADMIN_TOKEN="$(openssl rand -hex 32)"
export SF_USER_0_EMAIL="admin@example.com"
export SF_USER_0_PASSWORD="$(openssl rand -base64 32)"
export SF_USER_0_ID="user-001"

scripts/deploy-gcp.sh
SF_DEPLOY_ACTION=apply scripts/deploy-gcp.sh
```

El script crea/actualiza el servicio y un Cloud Run Job `syncfiles-backup` que ejecuta `/scripts/backup.sh` sobre el mismo volumen.

## Backup programado

Cree un Cloud Scheduler job HTTP que ejecute el job de Cloud Run o invoque el endpoint `/admin/backup` con el token administrativo. El backup queda en `/data/backups`; para copia offsite, copie ese directorio a Cloud Storage con un proceso autorizado y verifique el SHA-256 antes de borrar la copia local.

## Validación

```bash
SERVICE_URL="$(gcloud run services describe "$GCP_SERVICE" --region "$GCP_REGION" --format='value(status.url)')"
curl -fsS "$SERVICE_URL/health/live"
curl -fsS "$SERVICE_URL/health/ready"
```

## Limitaciones

- No es HA: un solo writer y un solo volumen.
- Cloud Run puede escalar a cero; la primera solicitud puede tener cold start.
- No se implementa dual-write ni PostgreSQL en V1.
- El script no crea red, IAM, IAP ni Cloud Scheduler; son decisiones del operador.
