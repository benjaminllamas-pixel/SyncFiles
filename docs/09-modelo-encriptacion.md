# Modelo de Encriptación

## Descripción
Especificación de algoritmos y estrategias de encriptación para SyncFiles.

## Principios Fundamentales

1. **End-to-End:** Solo el usuario puede descifrar sus datos
2. **Default Secure:** Encriptación siempre activada
3. **Strong Crypto:** Algoritmos de grado militar
4. **Key Management:** Control total del usuario sobre sus claves

## Algoritmos de Encriptación

### Datos en Reposo

**Algoritmo Principal:** AES-256-GCM
**Modo:** Galois/Counter Mode (autenticación incluida)
**Tamaño de clave:** 256 bits
**Tamaño de IV:** 96 bits

**Alternativa:** ChaCha20-Poly1305
**Ventaja:** Mejor en dispositivos sin aceleración AES

### Datos en Tránsito

**Protocolo:** TLS 1.3
**Cipher Suites Permitidos:**
- TLS_AES_256_GCM_SHA384
- TLS_CHACHA20_POLY1305_SHA256

### Derivación de Claves

**Función:** PBKDF2 (Password-Based Key Derivation Function)
**Hash:** SHA-256
**Iteraciones:** 100,000+ (ajustable)
**Salt:** 16 bytes aleatorios

