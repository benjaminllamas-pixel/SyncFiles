#!/usr/bin/env bash
set -euo pipefail

ACTION="${SF_DEPLOY_ACTION:-dry-run}"
PROJECT="${GCP_PROJECT:-}"
REGION="${GCP_REGION:-us-central1}"
SERVICE="${GCP_SERVICE:-syncfiles}"
IMAGE="${GCP_IMAGE:-}"
DISK_SIZE="${GCP_DISK_SIZE_GB:-10}"
ADMIN_TOKEN="${SF_ADMIN_TOKEN:-}"
USER_EMAIL="${SF_USER_0_EMAIL:-admin@syncfiles.local}"
USER_PASSWORD="${SF_USER_0_PASSWORD:-}"
USER_ID="${SF_USER_0_ID:-user-001}"

if [[ "$ACTION" != "dry-run" && "$ACTION" != "apply" ]]; then
    printf 'SF_DEPLOY_ACTION debe ser dry-run o apply.\n' >&2
    exit 2
fi
if [[ -z "$PROJECT" || -z "$IMAGE" ]]; then
    printf 'GCP_PROJECT e GCP_IMAGE son obligatorios.\n' >&2
    exit 2
fi
if [[ -z "$ADMIN_TOKEN" || -z "$USER_PASSWORD" ]]; then
    printf 'SF_ADMIN_TOKEN y SF_USER_0_PASSWORD son obligatorios.\n' >&2
    exit 2
fi

print_cmd() {
    printf '%s\n' "$*"
}

if [[ "$ACTION" == "dry-run" ]]; then
    printf 'Dry-run GCP SyncFiles\n'
    print_cmd "gcloud config set project $PROJECT"
    print_cmd "gcloud run deploy $SERVICE --region $REGION --image $IMAGE --allow-unauthenticated --concurrency 1"
    print_cmd "  --add-volume name=data,type=gce-persistent-disk,pd-size=${DISK_SIZE}GB,pd-type=pd-balanced"
    print_cmd "  --add-volume-mount volume=data,mount-path=/data"
    print_cmd "  --set-env-vars SF_BIND_ADDRESS=0.0.0.0:8080,SF_DATABASE_URL=sqlite:/data/db/syncfiles.db,SF_STORAGE_ROOT=/data/storage,SF_MIGRATIONS_DIR=/app/migrations,SF_STATIC_DIR=/app/static,BACKUP_DIR=/data/backups,SF_ADMIN_TOKEN=<oculto>,SF_USER_0_EMAIL=$USER_EMAIL,SF_USER_0_ID=$USER_ID"
    print_cmd "gcloud run jobs deploy ${SERVICE}-backup --region $REGION --image $IMAGE --task-timeout 10m --command /bin/bash --args /scripts/backup.sh"
    print_cmd "  --add-volume name=data,type=gce-persistent-disk,pd-size=${DISK_SIZE}GB,pd-type=pd-balanced"
    print_cmd "  --add-volume-mount volume=data,mount-path=/data"
    printf 'El volumen persistente admite un solo writer; mantenga concurrency=1. Configure Cloud Scheduler para ejecutar el job de backup.\n'
    exit 0
fi

if ! command -v gcloud >/dev/null 2>&1; then
    printf 'gcloud no está instalado.\n' >&2
    exit 1
fi

gcloud config set project "$PROJECT"
gcloud run deploy "$SERVICE" \
    --region "$REGION" \
    --image "$IMAGE" \
    --allow-unauthenticated \
    --concurrency 1 \
    --add-volume "name=data,type=gce-persistent-disk,pd-size=${DISK_SIZE}GB,pd-type=pd-balanced" \
    --add-volume-mount "volume=data,mount-path=/data" \
    --set-env-vars "SF_BIND_ADDRESS=0.0.0.0:8080,SF_DATABASE_URL=sqlite:/data/db/syncfiles.db,SF_STORAGE_ROOT=/data/storage,SF_MIGRATIONS_DIR=/app/migrations,SF_STATIC_DIR=/app/static,BACKUP_DIR=/data/backups,SF_ADMIN_TOKEN=$ADMIN_TOKEN,SF_USER_0_EMAIL=$USER_EMAIL,SF_USER_0_ID=$USER_ID"

gcloud run jobs deploy "${SERVICE}-backup" \
    --region "$REGION" \
    --image "$IMAGE" \
    --task-timeout 10m \
    --command /bin/bash \
    --args /scripts/backup.sh \
    --add-volume "name=data,type=gce-persistent-disk,pd-size=${DISK_SIZE}GB,pd-type=pd-balanced" \
    --add-volume-mount "volume=data,mount-path=/data"

printf 'Despliegue GCP completado. Programe ${SERVICE}-backup con Cloud Scheduler.\n'
