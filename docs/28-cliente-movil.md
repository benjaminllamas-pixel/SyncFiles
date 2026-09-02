# Cliente Móvil

## iOS (Swift)

**Requisitos:**
- iOS 14+
- App Groups entitlement

**UI:** SwiftUI
- Document picker integration
- Share sheet integration

**Background Sync:**
- URLSessionBackgroundConfiguration
- Silent push notifications

## Android (Kotlin)

**Requisitos:**
- Android 10+
- Permissions: READ_EXTERNAL_STORAGE, WRITE_EXTERNAL_STORAGE

**UI:** Jetpack Compose
- Material Design 3
- File picker con FileProvider

**Background Sync:**
- WorkManager
- FCM para notificaciones

## Características Comunes

- Upload/download selective de archivos
- Offline capability (sync cuando conecta)
- Battery optimization (sinc en WiFi, no cellular)
- Storage permissions (no acceso a todo el device)
