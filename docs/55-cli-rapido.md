# CLI de SyncFiles

## Instalación

Descomprime el archivo `syncfiles-cli-<version>-<platform>.tar.gz` o `.zip` y añade el directorio al `PATH`:

```bash
tar -xzf syncfiles-cli-*-linux-x86_64.tar.gz
export PATH="$PWD/syncfiles-cli-*/:$PATH"
```

## Configuración

```bash
export SF_SERVER_URL="https://syncfiles.example.com"
export SF_EMAIL="usuario@example.com"
export SF_PASSWORD="contraseña"
export SF_DEVICE_ID="mi-portatil"
```

## Comandos

```bash
syncfiles-cli login
syncfiles-cli upload ./documento.txt
syncfiles-cli download <file_id> ./documento.txt
syncfiles-cli list
syncfiles-cli diff 0
syncfiles-cli logout
```

## Verificación de integridad

Cada descarga compara el SHA-256 recibido con el metadato del servidor. Antes de instalar un release, verifica el archivo con:

```bash
sha256sum -c syncfiles-cli-<version>-<platform>.tar.gz.sha256
```
