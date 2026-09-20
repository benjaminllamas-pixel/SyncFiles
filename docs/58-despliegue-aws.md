# Despliegue mínimo en AWS ECS/Fargate

## Alcance V1

ECS/Fargate no conserva el sistema de archivos de la tarea. El script usa **Amazon EFS** montado en `/data` para SQLite y storage. EFS debe configurarse para un único writer de SyncFiles; no se usa SQLite compartido entre varias tareas.

## Requisitos previos

- Cluster ECS Fargate, VPC/subnets y security groups.
- Servicio ECS llamado `syncfiles` con Load Balancer o acceso de red controlado.
- EFS filesystem y access point con cifrado en tránsito.
- Execution role y Task role con permisos para EFS, logs y secretos.
- Grupo de logs CloudWatch.
- Imagen en ECR.
- `awscli` configurada.
- Revisar costes de Fargate, EFS, ECR, CloudWatch y Load Balancer antes de desplegar.

## Variables

```bash
export AWS_REGION=us-east-1
export AWS_CLUSTER=syncfiles
export AWS_SERVICE=syncfiles
export AWS_ECS_IMAGE=123456789012.dkr.ecr.us-east-1.amazonaws.com/syncfiles:v1
export AWS_EFS_FILESYSTEM_ID=fs-0123456789abcdef0
export AWS_EFS_ACCESS_POINT_ID=fsap-0123456789abcdef0
export AWS_EXECUTION_ROLE_ARN=arn:aws:iam::123456789012:role/syncfiles-execution
export AWS_TASK_ROLE_ARN=arn:aws:iam::123456789012:role/syncfiles-task
export AWS_LOG_GROUP=/ecs/syncfiles
export SF_ADMIN_TOKEN="$(openssl rand -hex 32)"
export SF_USER_0_EMAIL="admin@example.com"
export SF_USER_0_PASSWORD="$(openssl rand -base64 32)"
export SF_USER_0_ID="user-001"
```

## Despliegue

```bash
scripts/deploy-aws.sh
SF_DEPLOY_ACTION=apply scripts/deploy-aws.sh
```

El modo `dry-run` renderiza y valida `infra/aws-ecs-task-definition.json` sin llamar a AWS. El modo `apply` registra la task definition y fuerza una nueva implementación del servicio existente.

## Backup

Ejecutar una tarea con override de comando:

```bash
aws ecs run-task \
  --cluster "$AWS_CLUSTER" \
  --task-definition syncfiles \
  --launch-type FARGATE \
  --network-configuration "awsvpcConfiguration={subnets=[subnet-...,],securityGroups=[sg-...]}" \
  --overrides '{"containerOverrides":[{"name":"syncfiles-server","command":["/bin/bash","/scripts/backup.sh"]}]}'
```

Copie `/data/backups` a S3 con cifrado y retención según la política de la organización. Verifique el `.sha256` antes de considerar válido el backup.

## Validación

```bash
curl -fsS https://syncfiles.example.com/health/live
curl -fsS https://syncfiles.example.com/health/ready
aws ecs describe-services --cluster "$AWS_CLUSTER" --services "$AWS_SERVICE"
```

## Limitaciones

- No es HA: una sola tarea writer sobre EFS/SQLite.
- El script no crea VPC, EFS, roles, ALB, IAM ni EventBridge.
- Los secretos deberían migrarse a AWS Secrets Manager; V1 los inyecta como environment variables para mantener el despliegue mínimo.
