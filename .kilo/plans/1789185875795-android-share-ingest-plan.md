# Plan: Recibir shares de texto/URLs desde otras apps (Chrome) con nombre de archivo — SyncFiles Android

## Contexto actual

- `AndroidManifest.xml:26-31` ya declara el intent-filter `SEND`/`SEND_MULTIPLE` + `text/plain` y `launchMode="singleTask"` en `MainActivity`. **No requiere cambios**.
- `MainActivity.handleShareIntent` (`MainActivity.kt:53-86`) ya extrae `EXTRA_TEXT` (simple y múltiple) pero **auto-guarda** vía `SyncEngine.ingestSharedText` (`SyncEngine.kt:65-109`) con nombre autogenerado `shared/<timestamp>.txt` y solo muestra un Toast.
- El objetivo del usuario: al compartir desde Chrome (incl. varias pestañas) se abra una pantalla para **nombrar el archivo .txt** antes de crearlo, con el contenido íntegro.
- No es posible verificar el origen (Chrome vs. otra app) de un `ACTION_SEND`: Android no expone esa info de forma fiable. Se acepta cualquier `text/plain`.
- No existen tests (`app/src/test` y `app/src/androidTest` están vacíos). Validación manual vía adb + build.

## Decisiones resueltas con el usuario

1. **Flujo**: pantalla de guardado ("Guardar compartido") con preview del contenido, campo de nombre pre-llenado con sugerencia, y botones Guardar/Cancelar. El archivo solo se crea al confirmar.
2. **Contenido**: un único archivo `.txt` con TODO el contenido tal cual llegó (URLs y texto mezclados, multi-línea). `SEND_MULTIPLE` (lista de textos) se une con `\n` en un solo contenido.
3. **Sin sesión/carpeta**: se guarda como **pendiente en SQLite dedicada** (`pending_shares`) + Toast "Guardado como pendiente — inicia sesión para completarlo". Tras login (o apertura de app con sesión+carpeta), se navega a la pantalla de guardado con el pendiente más antiguo cargado; se confirman de a uno.
4. **Nombre sugerido**: inteligente — 1 URL → dominio (`github.com.txt`); varias URLs → `pestanas-<yyyyMMdd>.txt`; texto puro → primeras ~4 palabras en kebab-case (`notas-de-reunion.txt`). Campo editable.
5. **Carpeta destino**: fija `<carpeta-sync>/shared/` (coincide con comportamiento actual).
6. **Validación de nombre**: sanitizar automáticamente (caracteres prohibidos `/ \ : * ? " < > |`, longitud máx ~100, asegurar sufijo `.txt`) + dedupe con sufijo `-1`, `-2`... si colisiona. Live preview de la ruta final en la pantalla.
7. **Alcance**: solo `text/plain`. Shares binarios (imágenes/PDF vía `EXTRA_STREAM`) quedan fuera de alcance.
8. **Cancelar en pantalla de guardado**: descarta el share pendiente/in-flight definitivamente (Snackbar "Descartado", sin undo).

## Tareas

### 1. Nuevo `data/local/PendingSharesStore.kt`
Seguir el patrón de `SyncQueueStore.kt` (SQLiteOpenHelper en DB propia, p.ej. `syncfiles_pending_shares.db`):
- Tabla `pending_shares`: `id INTEGER PRIMARY KEY AUTOINCREMENT, content TEXT NOT NULL, created_at INTEGER NOT NULL`.
- API: `insert(content)`, `oldest(): PendingShare?`, `delete(id)`, `count(): Int`.
- Cap FIFO: al insertar, si `count() > 20` borrar el más antiguo (evitar crecimiento ilimitado).

### 2. Nuevo `data/util/ShareNaming.kt`
Funciones puras (testables sin instrumentación):
- `suggestFileName(content: String, now: Date): String`:
  - Extraer URLs con `Regex("https?://\\S+")`.
  - 1 URL → host sin `www.` (p.ej. `github.com`).
  - >1 URL → `pestanas-yyyyMMdd`.
  - Sin URLs → primeras ~4 palabras del texto, lowercase, espacios → `-`, sin stopwords no es necesario (simple).
- `sanitizeFileName(raw: String): String`: trim, eliminar prohibidos `/ \ : * ? " < > |` y saltos de línea, colapsar espacios, colapsar puntos, recortar a 100 chars, quitar puntos/espacios finales. Vacío tras sanitizar → devolver `compartido`.
- `ensureTxtExtension(name: String): String`: agregar `.txt` si no lo tiene (evitar doble `.txt.txt`).
- `dedupeFileName(root: File, name: String): String`: si existe `shared/<name>` probar `<base>-1.txt`, `-2`... hasta 50; devuelve el primero libre. Sin acentos en los autogenerados (`pestanas-`); los escritos por el usuario se respetan salvo sanitización.

### 3. Modificar `SyncEngine.kt`
- Reemplazar `ingestSharedText(text)` por `ingestNamedShare(content: String, fileName: String): String?`:
  - Validaciones de siempre: sesión activa, contenido no blank, `resolveRootFile()` no null → si falla devuelve null.
  - Ruta final: `shared/<fileName>` (fileName ya viene sanitizado+dedupeado desde la UI; el engine re-sanitiza por defensa).
  - Escribir `content` **tal cual** (sin la normalización `www→https://` actual — el usuario pidió fidelidad al contenido compartido; la sugerencia de nombre ya usa el URL).
  - Mantener el resto del flujo existente: `localStore.upsert(LocalFile(...status="local"...))`, `queue.enqueue(upload)`, `SyncScheduler.syncNow()`.
- Nota pre-existente (fuera de alcance): el `FileWatcher` también encolará el CREATE del archivo nuevo → doble entrada en `sync_queue` para la misma ruta (comportamiento ya presente hoy; el servidor procesa ambos uploads idempotentemente por path).

### 4. Nueva UI `ui/share/SaveShareScreen.kt` + `SaveShareViewModel.kt`
Patrones a imitar: `FilesScreen.kt` (Scaffold+TopAppBar+SnackbarHost, `ElevatedCard`), `SettingsScreen.kt` (`OutlinedTextField` con `RoundedCornerShape(14.dp)`), strings español hardcodeadas (consistente con el resto).
- Estado (`SaveShareUiState`): `content: String`, `suggestedName: String`, `name: String`, `previewPath: String?`, `saving: Boolean`, `lastMessage: String?`, `isPending: Boolean` (si vino de la cola), `pendingRemaining: Int`.
- Pantalla:
  - Preview: contenido en `LazyColumn`/`verticalScroll` dentro de card, máx ~6 líneas visibles, monospace opcional.
  - Campo nombre (`OutlinedTextField`, singleLine) pre-llenado con sugerencia.
  - Live preview de ruta: `<nombre-carpeta-sync>/shared/<name>.txt` recalculado en cada cambio del campo (usar `SessionStore.getSyncRootName()`).
  - Botones: "Guardar" (primary), "Descartar" (text/outlined).
- Al pulsar Guardar: `sanitize` + `ensureTxt` + `dedupe` → `syncEngine.ingestNamedShare` (en `Dispatchers.IO`):
  - Éxito → Snackbar "Guardado en shared/<nombre>"; si `isPending` borrar fila de `PendingSharesStore`.
  - Fallo (sin sesión/carpeta a mitad) → insertar en `PendingSharesStore` como fallback + Snackbar informativo.
  - Después: si quedan pendientes → cargar el siguiente en la misma pantalla; si no → `onDone()` (volver atrás).
- Al pulsar Descartar: si `isPending`, borrar fila; Snackbar "Descartado"; mismo "siguiente pendiente o salir".

### 5. Cableado en `MainActivity.kt` / `SyncFilesApp`
- Reescribir `handleShareIntent`:
  - Extraer textos (igual que hoy) y **unirlos con `\n`** en un solo contenido; blank → ignorar.
  - Si `sessionStore.getActiveSession() != null && sessionStore.getSyncRootUri() != null` → poner el contenido en un estado observable (p.ej. `mutableStateOf` a nivel Activity o un `ShareInboxHolder` simple) y navegar a ruta `"save-share"`.
  - Si no → `PendingSharesStore.insert(content)` + Toast "Guardado como pendiente — inicia sesión en SyncFiles para completarlo".
  - `onNewIntent` con la app abierta: nuevo share **reemplaza** lo que haya en la pantalla de guardado (caso raro, aceptado; el contenido no confirmado previo se pierde si era in-flight; los pendientes viven en SQLite).
- En `SyncFilesApp` NavHost agregar destino `"save-share"` con su ViewModel (factory con `appContext`, engine accesible vía `(appContext as SyncFilesApplication).syncEngine`).
- Disparadores de pendientes tras login/apertura:
  - En `onLoginSuccess` (después de `navigate("home")`): si `PendingSharesStore.count() > 0` y hay carpeta → `navigate("save-share")`.
  - En la composición de `"home"` con `LaunchedEffect`: mismo check (cubre app ya logueada que abre por icono; también cubre "eligió carpeta después" porque home se re-compone al volver de settings).
  - Condición para navegar: sesión activa && `getSyncRootUri() != null` && pendientes > 0. Sin carpeta → no molestar (siguen pendientes).
- El contenido in-flight (no pendiente) vive solo en memoria (ViewModel del backstack entry): muerte de proceso a mitad lo pierde; aceptado (los ya confirmados como pendientes están en SQLite).

### 6. Strings y recursos
- Strings en español hardcodeadas en composables (consistente con `FilesScreen`/`LoginScreen`); no tocar `strings.xml` salvo necesidad.

## Edge cases cubiertos
- `SEND_MULTIPLE` con `EXTRA_TEXT` como `ArrayList<CharSequence>` → join `\n`.
- Share con texto blank → ignorar silenciosamente.
- Nombre con solo caracteres prohibidos → `compartido.txt` tras sanitizar.
- Colisión de nombres → dedupe `-1`, `-2`... máximo 50 intentos.
- Sin sesión al compartir → pendiente + Toast.
- Sin carpeta al compartir (con sesión) → pendiente + Toast (la condición de handleShareIntent exige ambos).
- Proceso muerto con pendientes → persisten en SQLite, reaparecen al abrir app.
- Doble share consecutivo (onNewIntent) → reemplaza la pantalla actual.

## Fuera de alcance (explícito)
- Shares binarios (`image/*`, `application/*`, `EXTRA_STREAM`).
- Direct Share shortcuts / `ShortcutManagerCompat` para ranking en el share sheet.
- Undo del descarte.
- Arreglo del doble-enqueue watcher+manual (pre-existente).
- Encripción del contenido pendiente (consistente con el resto de BDs locales del proyecto).

## Validación (manual, no hay suite de tests)
1. Build: `./gradlew :app:assembleDebug` desde `syncfiles-android/`.
2. Con sesión+carpeta activas:
   - `adb shell am start -a android.intent.action.SEND -t text/plain --es android.intent.extra.TEXT "https://github.com/usuario/repo" -n com.syncfiles.client.android/.MainActivity` → abre pantalla con sugerencia `github.com.txt`.
   - Multi-línea: `--es android.intent.extra.TEXT "https://a.com\nhttps://b.org"` → sugerencia `pestanas-<hoy>.txt`; guardar crea UN archivo con ambas líneas.
   - `SEND_MULTIPLE` con array → un solo archivo con los textos unidos.
   - Nombre con `/ ? *` → sanitiza en preview; nombre duplicado → dedupe visible en preview al guardar.
   - Verificar en carpeta sync: archivo creado en `shared/`, aparece en cola/sync (Home o `files` screen), sube al servidor.
3. Sin sesión (logout previo): compartir → Toast pendiente; login → pantalla de guardado abre con el pendiente; guardar/descartar funciona; compartir 3 shares sin sesión → se procesan de a uno tras login.
4. Sin carpeta (sesión activa, root no elegido): compartir → Toast pendiente; elegir carpeta en Settings → al volver a Home abre pantalla de guardado.
5. Cancelar flujo: descartar pendiente → eliminado de SQLite (re-compartir no lo re-muestra).

## Archivos afectados
| Archivo | Acción |
|---|---|
| `app/src/main/java/.../data/local/PendingSharesStore.kt` | nuevo |
| `app/src/main/java/.../data/util/ShareNaming.kt` | nuevo |
| `app/src/main/java/.../ui/share/SaveShareScreen.kt` | nuevo |
| `app/src/main/java/.../ui/share/SaveShareViewModel.kt` | nuevo |
| `app/src/main/java/.../MainActivity.kt` | modificar (handleShareIntent, NavHost, pendientes) |
| `app/src/main/java/.../data/local/SyncEngine.kt` | modificar (ingestNamedShare reemplaza ingestSharedText) |
| `AndroidManifest.xml` | sin cambios (filtro ya presente) |
