#!/usr/bin/env bash
set -euo pipefail

ACTION="${SF_DEPLOY_ACTION:-dry-run}"
REGION="${AWS_REGION:-us-east-1}"
CLUSTER="${AWS_CLUSTER:-syncfiles}"
SERVICE="${AWS_SERVICE:-syncfiles}"
IMAGE="${AWS_ECS_IMAGE:-}"
EFS_ID="${AWS_EFS_FILESYSTEM_ID:-}"
EFS_ACCESS_POINT="${AWS_EFS_ACCESS_POINT_ID:-}"
EXECUTION_ROLE="${AWS_EXECUTION_ROLE_ARN:-}"
TASK_ROLE="${AWS_TASK_ROLE_ARN:-}"
LOG_GROUP="${AWS_LOG_GROUP:-/ecs/syncfiles}"
ADMIN_TOKEN="${SF_ADMIN_TOKEN:-}"
USER_EMAIL="${SF_USER_0_EMAIL:-admin@syncfiles.local}"
USER_PASSWORD="${SF_USER_0_PASSWORD:-}"
USER_ID="${SF_USER_0_ID:-user-001}"

if [[ "$ACTION" != "dry-run" && "$ACTION" != "apply" ]]; then
    printf 'SF_DEPLOY_ACTION debe ser dry-run o apply.\n' >&2
    exit 2
fi
if [[ -z "$IMAGE" || -z "$EFS_ID" || -z "$EFS_ACCESS_POINT" || -z "$EXECUTION_ROLE" || -z "$TASK_ROLE" ]]; then
    printf 'Faltan variables obligatorias de ECS/EFS/roles.\n' >&2
    exit 2
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TEMPLATE="$ROOT/infra/aws-ecs-task-definition.json"
WORKDIR="$(mktemp -d "${TMPDIR:-/tmp}/syncfiles-aws.XXXXXX")"
trap 'rm -rf "$WORKDIR"' EXIT
RENDERED="$WORKDIR/task-definition.json"

python3 - "$TEMPLATE" "$RENDERED" "$IMAGE" "$EFS_ID" "$EFS_ACCESS_POINT" "$EXECUTION_ROLE" "$TASK_ROLE" "$LOG_GROUP" "$REGION" "$ADMIN_TOKEN" "$USER_EMAIL" "$USER_PASSWORD" "$USER_ID" <<'PY'
import json
import pathlib
import sys

template, output, image, efs, access_point, execution_role, task_role, log_group, region, admin_token, user_email, user_password, user_id = sys.argv[1:]
data = json.loads(pathlib.Path(template).read_text())
container = data["containerDefinitions"][0]
container["image"] = image
container["environment"] = [
    {"name": "SF_BIND_ADDRESS", "value": "0.0.0.0:8080"},
    {"name": "SF_MIGRATIONS_DIR", "value": "/app/migrations"},
    {"name": "SF_STATIC_DIR", "value": "/app/static"},
    {"name": "SF_DATABASE_URL", "value": "sqlite:/data/db/syncfiles.db"},
    {"name": "SF_STORAGE_ROOT", "value": "/data/storage"},
    {"name": "BACKUP_DIR", "value": "/data/backups"},
    {"name": "SF_ADMIN_TOKEN", "value": admin_token},
    {"name": "SF_USER_0_EMAIL", "value": user_email},
    {"name": "SF_USER_0_PASSWORD", "value": user_password},
    {"name": "SF_USER_0_ID", "value": user_id},
]
container["healthCheck"]["command"] = ["CMD-SHELL", "curl -sf http://localhost:8080/health/live || exit 1"]
container["logConfiguration"]["options"] = {
    "awslogs-group": log_group,
    "awslogs-region": region,
    "awslogs-stream-prefix": "syncfiles",
}
data["volumes"][0]["efsVolumeConfiguration"]["filesystemId"] = efs
data["volumes"][0]["efsVolumeConfiguration"]["authorizationConfig"]["accessPointId"] = access_point
data["executionRoleArn"] = execution_role
data["taskRoleArn"] = task_role
pathlib.Path(output).write_text(json.dumps(data, indent=2) + "\n")
PY

if [[ "$ACTION" == "dry-run" ]]; then
    printf 'Dry-run AWS ECS SyncFiles\n'
    printf 'Task definition renderizada: %s\n' "$RENDERED"
    printf 'aws ecs register-task-definition --cli-input-json file://%s\n' "$RENDERED"
    printf 'aws ecs update-service --cluster %s --service %s --force-new-deployment\n' "$CLUSTER" "$SERVICE"
    printf 'EFS debe montar /data y el servicio debe tener un writer; use una tarea programada para /scripts/backup.sh.\n'
    exit 0
fi

if ! command -v aws >/dev/null 2>&1; then
    printf 'aws CLI no está instalado.\n' >&2
    exit 1
fi

TASK_ARN="$(aws ecs register-task-definition --cli-input-json "file://$RENDERED" --query taskDefinition.taskDefinitionArn --output text)"
aws ecs update-service \
    --cluster "$CLUSTER" \
    --service "$SERVICE" \
    --task-definition "$TASK_ARN" \
    --force-new-deployment

printf 'Despliegue AWS ECS completado: %s\n' "$TASK_ARN"
