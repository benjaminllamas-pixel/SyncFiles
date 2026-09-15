# Diagnóstico: Archify falla en este ambiente

## Síntoma
Archify: Quick Scan / Full Review falla en VS Code (v1.136.1) dentro del proyecto SyncFiles.

## Causa raíz (confirmada)
**No hay ninguna extensión proveedora de modelos de lenguaje (LLM) instalada en VS Code.**

Archify (dineshkumarsaini.archify-vscode 0.3.0) **no tiene IA propia**: todo escaneo llama a
`vscode.lm.selectChatModels()` y usa el primer modelo disponible. Sin proveedor, la lista viene
vacía y el escaneo falla con:

> "No language model is available. Please ensure a Copilot extension is installed and signed in."

### Evidencia
1. `out/extension.js` de Archify (clase del proveedor LLM):
   `lm.selectChatModels()` → si `length === 0` → `null` → el escaneo no puede puntuar.
2. Extensiones instaladas (`~/.vscode/extensions/`): kilocode.kilo-code (3 versiones),
   markdownlint, debugpy, pylance, python-envs, python y archify. **Ninguna** es GitHub Copilot,
   Claude for VS Code ni Amazon Q.
3. **Kilo Code NO sirve como proveedor**: usa sus propias claves/proveedores API y no registra
   modelos en `vscode.lm` (verificado en su `package.json` — no contribution de language models).
4. Logs (`Code/logs/20260906T012006/`): Archify se instala y activa correctamente; no hay errores
   de instalación. El fallo es solo la ausencia de modelos.

## Problema secundario (bug de Archify 0.3.0, no bloquea el escaneo)
VS Code 1.136 rechaza el registro de las tools LM de Archify:

> "Extension CANNOT register tool with 'canBeReferencedInPrompt' set without a 'toolReferenceName'"
> → `Error: Tool "archify_review"/"archify_scan"/"archify_deps" was not contributed`

Esto rompe las tools de chat (`@archify` en Copilot Chat) pero **no** los comandos Quick Scan /
Full Review del sidebar. Se corrige cuando el autor publique una versión fija; no es accionable
por el usuario.

## Pasos de corrección (elegir una opción)
Archify acepta cualquiera de estas extensiones como proveedor LLM:

1. **Opción A — GitHub Copilot** (recomendada, plan gratuito disponible)
   - Instalar extensión `GitHub.copilot` y `GitHub.copilot-chat`
   - Iniciar sesión con cuenta GitHub y verificar que Copilot Chat responde
2. **Opción B — Claude for VS Code**
   - Instalar `anthropic.claude-code` / extensión Claude y autenticar
3. **Opción C — Amazon Q**
   - Instalar `amazonwebservices.amazon-q` y autenticar con Builder ID

Nota: el comando "Archify: Save API Key" **no** arregla el escaneo — esa clave es solo para
publicar reportes en Archify Cloud; el motor de escaneo usa exclusivamente `vscode.lm`.

## Validación
1. Recargar VS Code tras instalar el proveedor (Cmd+Shift+P → "Reload Window")
2. Abrir el panel Archify en la Activity Bar
3. Ejecutar "Archify: Quick Scan ⚡" sobre el workspace SyncFiles
4. Verificar que aparece el reporte con score global y 10 categorías
5. Confirmar en consola de la extensión (Output → Archify) el mensaje
   `Archify: Using LLM model "<id>"` como prueba de detección del modelo

## Fuera de alcance
- Reparar el bug de `toolReferenceName` de Archify 0.3.0 (requiere nueva versión del autor)
- Archivo de reporte esperado: los proyectos Rust (syncfiles-*) están dentro de los globs
  soportados por Archify (`*.rs`, `Cargo.toml` vía manifests), así que el workspace es compatible
