# Gestión de Claves

## Descripción
Políticas y procedimientos para manejo, almacenamiento y distribución de claves criptográficas.

## Tipos de Claves

### 1. Master Key (Contraseña del Usuario)
**Origen:** Contraseña ingresada por usuario
**Almacenamiento:** Memoria volátil solamente (RAM)
**Duración:** Sesión activa
**Rotación:** Cambio de contraseña

### 2. Derived Encryption Key
**Origen:** PBKDF2(Master Key)
**Almacenamiento:** Caché local protegido
**Duración:** Sesión o 8 horas
**Distribución:** Solo en cliente local

### 3. TLS Session Key
**Origen:** Handshake TLS 1.3
**Almacenamiento:** RAM (kernel space si posible)
**Duración:** Sesión TLS (< 24 horas)
**Distribución:** Solo entre cliente y servidor

### 4. Public Key (para Compartición)
**Origen:** Generado al registro
**Almacenamiento:** Servidor (público)
**Duración:** Indefinida (hasta revocación)
**Distribución:** Público

### 5. Private Key (para Compartición)
**Origen:** Generado al registro
**Almacenamiento:** Encriptado con Master Key
**Duración:** Indefinida
**Distribución:** Solo en cliente autorizado

## Ciclo de Vida de Claves

### Fase 1: Generación

