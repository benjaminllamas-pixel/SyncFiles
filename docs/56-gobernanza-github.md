# Gobernanza de GitHub para V1

## Reglas de la rama `main`

Configura la protección de rama en **Settings → Branches → Branch protection rules**:

- pattern: `main`
- **Require a pull request before merging**: activado
- **Require approvals**: `1`
- **Dismiss stale approvals**: activado
- **Require status checks before merging**: activado
- checks obligatorios: `Rust (check + test + clippy)`, `Android (assembleDebug)`, `Release artifacts`
- **Require branches to be up to date before merging**: activado
- **Require conversation resolution before merging**: activado
- **Do not allow bypassing the above settings**: activado para producción
- **Include administrators**: activado

`CODEOWNERS` exige revisión de `@SyncFiles/maintainers` para cambios en workflows, documentación, scripts, Android y crates Rust. La protección de rama es una configuración del repositorio y no puede aplicarse desde un commit.

## Dependabot

`.github/dependabot.yml` revisa semanalmente Cargo, Gradle y GitHub Actions. Los PR automáticos deben pasar CI antes de merge.

## Secretos de release

Configura en **Settings → Secrets and variables → Actions**:

| Secreto | Uso |
|---|---|
| `SF_GPG_PRIVATE_KEY` | GPG base64 para Linux |
| `SF_GPG_PASSPHRASE` | Frase de GPG |
| `SF_APPLE_CERT_BASE64` | Certificado Apple `.p12` en base64 |
| `SF_APPLE_CERT_PASSWORD` | Contraseña del `.p12` |
| `SF_APPLE_IDENTITY` | Identidad de `codesign` |
| `SF_APPLE_ID` | Apple ID de notaría |
| `SF_APPLE_APP_PASSWORD` | Contraseña de aplicación |
| `SF_APPLE_TEAM_ID` | Team ID |
| `SF_WINDOWS_PFX_BASE64` | PFX en base64 |
| `SF_WINDOWS_CERT_PASSWORD` | Contraseña PFX |
| `SF_ANDROID_KEYSTORE_BASE64` | Keystore Android en base64 |
| `SF_ANDROID_KEYSTORE_PASSWORD` | Contraseña del keystore |
| `SF_ANDROID_KEYSTORE_ALIAS` | Alias de la clave |
| `SF_ANDROID_KEY_PASSWORD` | Contraseña de la clave |

El workflow de tag falla si falta la firma requerida. Los secretos nunca se escriben en el repositorio ni en artefactos.

## Verificación local de la configuración

```bash
gh api repos/OWNER/REPO/branches/main --jq '.protection.required_pull_request_reviews.required_approving_review_count'
gh api repos/OWNER/REPO/branches/main --jq '.protection.required_status_checks.contexts'
```
