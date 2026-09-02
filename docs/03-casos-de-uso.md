# Casos de Uso

## Descripción
Escenarios detallados de interacción del usuario con el sistema.

## CU-01: Registro e Instalación Inicial

**Actor Principal:** Usuario nuevo  
**Precondiciones:** Usuario tiene acceso a internet

**Flujo Principal:**
1. Usuario descarga la aplicación
2. Ejecuta el instalador
3. Crea cuenta (correo, contraseña)
4. Configura carpeta de sincronización
5. Inicia sincronización automática

**Flujo Alternativo (Cuenta existente):**
- Usuario ingresa credenciales existentes

---

## CU-02: Sincronización Automática

**Actor Principal:** Sistema  
**Precondiciones:** Cliente instalado y autenticado

**Flujo Principal:**
1. Cliente monitorea carpeta local
2. Detecta cambios en archivos
3. Encripta cambios
4. Envía al servidor
5. Servidor actualiza otros clientes
6. Otros clientes descargan y desencriptan

---

## CU-03: Resolución de Conflictos

**Actor Principal:** Usuario / Sistema

**Escenario:** Dos dispositivos modifican el mismo archivo simultáneamente

**Flujo de Resolución:**
1. Sistema detecta conflicto
2. Crea versión alternativa del archivo
3. Notifica al usuario
4. Usuario elige versión a mantener

---

## CU-04: Compartición de Archivos

**Actor Principal:** Usuario propietario  
**Actor Secundario:** Usuario receptor

**Flujo:**
1. Usuario selecciona archivo para compartir
2. Define permisos (lectura/escritura)
3. Genera enlace o invita usuario
4. Usuario receptor recibe notificación
5. Usuario receptor accede a archivo compartido

---

## CU-05: Recuperación de Versiones Anteriores

**Actor Principal:** Usuario

**Flujo:**
1. Usuario accede historial de archivo
2. Selecciona versión anterior
3. Sistema restaura versión anterior
4. Cambio se sincroniza a todos los dispositivos
