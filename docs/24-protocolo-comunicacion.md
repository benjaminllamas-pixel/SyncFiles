# Protocolo de Comunicación

## Opciones Evaluadas

### WebDAV
**Ventajas:** Standard, amplio soporte
**Desventajas:** Ineficiente para sync, sin delta

### rsync
**Ventajas:** Especializado en sync
**Desventajas:** No seguro por defecto, binario

### SFTP
**Ventajas:** Secure, estándar
**Desventajas:** No optimizado para delta

### Custom Binary Protocol
**Ventajas:** Optimizado para sync, encriptación integrada
**Desventajas:** Más desarrollo, menos estándar

## Decisión: Custom Protocol

**Basado en:** Binary framing + TLS 1.3

**Implementación:**

```rust
// Rust: Servidor
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:8443").await?;
    let tls = create_tls_acceptor();
    
    loop {
        let (socket, _) = listener.accept().await?;
        let tls = tls.clone();
        
        tokio::spawn(async move {
            let tls_stream = tls.accept(socket).await?;
            handle_client(tls_stream).await
        });
    }
}

async fn handle_client(stream: TlsStream<TcpStream>) {
    let mut reader = BufReader::new(stream);
    
    loop {
        let msg = read_frame(&mut reader).await?;
        process_message(msg).await;
    }
}
```
