# Detección de Cambios

## Descripción
Mecanismos para detectar modificaciones en el sistema de archivos en tiempo real y de forma eficiente.

## Estrategias de Detección

### Estrategia 1: File System Events (Nativa)

#### Windows: ReadDirectoryChangesW

```csharp
// Monitoreo de carpeta
FileSystemWatcher watcher = new FileSystemWatcher(@"C:\SyncFolder");
watcher.NotifyFilter = NotifyFilters.FileName | NotifyFilters.LastWrite;
watcher.Changed += OnFileChanged;
watcher.Created += OnFileCreated;
watcher.Deleted += OnFileDeleted;
watcher.EnableRaisingEvents = true;

private void OnFileChanged(object sender, FileSystemEventArgs e)
{
    // Evento disparado inmediatamente (< 10ms)
    LogEvent($"File changed: {e.FullPath}");
}
```

**Ventajas:**
- Reporte inmediato (< 10ms)
- Bajo overhead
- Integrado en Windows

**Desventajas:**
- No reporta cambios de permisos
- Buffer límitado (64KB para eventos)

#### macOS: FSEvents

```swift
import Foundation

let paths = ["/Users/user/SyncFolder"]
let sinceWhen = FSEventStreamEventId(kFSEventStreamEventIdSinceNow)
let flags: FSEventStreamCreateFlags = .fileEvents

let callback: FSEventStreamCallback = { 
    streamRef, userData, numEvents, eventPaths, eventFlags, eventIds in
    // Callback en tiempo real
    for i in 0..<numEvents {
        let path = (eventPaths as! [String])[Int(i)]
        print("Event: \(path)")
    }
}

let stream = FSEventStreamCreate(
    kCFAllocatorDefault,
    callback,
    nil,
    paths as CFArray,
    sinceWhen,
    0.1,  // Latency: 100ms
    flags
)
```

**Características:**
- Eficiente en archivos grandes
- Agrupa eventos por directorio
- Latencia configurable

#### Linux: inotify

```python
import inotify_simple

inotify = inotify_simple.INotify()
watch_descriptor = inotify.add_watch('/home/user/SyncFolder', 
                                     inotify_simple.flags.MODIFY | 
                                     inotify_simple.flags.CREATE |
                                     inotify_simple.flags.DELETE)

for event in inotify.read(timeout_ms=100):
    print(f"Event: {event.name}, Type: {event.mask}")
```

**Características:**
- Configurable por evento
- Bajo consumo de CPU
- Límite de watches: sysctl fs.inotify.max_user_watches

### Estrategia 2: Polling Periódico

**Fallback cuando eventos no disponibles:**

```python
import os
import hashlib
from pathlib import Path
import time

class PollingWatcher:
    def __init__(self, path, interval=5):
        self.path = path
        self.interval = interval
        self.file_hashes = {}
    
    def get_file_hash(self, filepath):
        sha256 = hashlib.sha256()
        with open(filepath, 'rb') as f:
            for chunk in iter(lambda: f.read(4096), b''):
                sha256.update(chunk)
        return sha256.hexdigest()
    
    def scan(self):
        for filepath in Path(self.path).rglob('*'):
            if filepath.is_file():
                current_hash = self.get_file_hash(str(filepath))
                
                if filepath.name not in self.file_hashes:
                    # Nuevo archivo
                    self.on_created(filepath)
                    self.file_hashes[filepath.name] = current_hash
                elif self.file_hashes[filepath.name] != current_hash:
                    # Archivo modificado
                    self.on_modified(filepath)
                    self.file_hashes[filepath.name] = current_hash
    
    def run(self):
        while True:
            self.scan()
            time.sleep(self.interval)
```

**Características:**
- Confiable pero lento (lag de hasta 5s)
- Alto consumo de CPU/disco
- Uso: fallback, auditoría

## Detección de Cambios de Atributos

### Metadatos Monitoreados

