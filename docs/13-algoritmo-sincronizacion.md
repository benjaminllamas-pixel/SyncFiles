# Algoritmo de Sincronización

## Descripción
Especificación del algoritmo core que detecta, transmite y aplica cambios entre clientes y servidor.

## Modelo de Sincronización

### Tipo: Bidireccional Continua (Continuous Two-Way Sync)

## Principios Fundamentales

1. **Bidireccional:** Cambios fluyen en ambas direcciones
2. **Incremental:** Solo se sincronizan cambios, no copias completas
3. **Eficiente:** Minimiza ancho de banda y CPU
4. **Confiable:** Garantiza entrega eventual de cambios
5. **Resiliente:** Maneja desconexiones y recupera

## Arquitectura del Sync Engine

### Componentes Principales

- **Detectores de Cambios:** Monitorean archivos y carpetas
- **Transmisores:** Envian cambios al servidor
- **Aplicadores:** Actualizan el estado del archivo en el cliente
- **Gestores de Conflictos:** Resuelven discrepancias entre versiones

### Flujo de Datos

1. El detector de cambios identifica una modificación en un archivo.
2. El transmisor envía la información del cambio al servidor.
3. El aplicador recibe la actualización y modifica el archivo correspondiente.
4. Si hay un conflicto, el gestor de conflictos interviene para resolverlo.

## Estadísticas por Archivo

- Cambios más frecuentes (qué líneas)
- Usuarios más activos
- Dispositivos que modificaron
- Patrones temporales de cambio

## Beneficios

- Identificar secciones problemáticas
- Detectar sabotaje (muchos cambios de un mismo usuario en corto tiempo)

