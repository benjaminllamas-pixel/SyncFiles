# Cliente Windows

## Requisitos

- Windows 10 Pro+ (versión 2004)
- .NET Runtime 6.0+
- 100MB espacio en disco
- 200MB RAM mínimo

## Componentes

**UI:** WPF
- Tray icon integration
- Right-click context menu
- Bubble notifications

**File Watcher:** ReadDirectoryChangesW
- Monitoreo recursivo
- Buffer: 64KB (configurable)

**Almacenamiento:** 
- DPAPI para claves
- SQLite para metadata

## Distribución

Instalador MSI:
- Silent install: `msiexec /i SyncFiles.msi /quiet`
- Auto-update: Windows Update Integration
- Desinstalación limpia (sin residuos)

## Integración del Sistema

