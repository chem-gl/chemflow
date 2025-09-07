//! Módulo: almacenamiento de eventos (event sourcing) para "flows".
//!
//! Este módulo define una interfaz (trait) para un almacén de eventos
//! append-only y una implementación en memoria. La idea es que cada "flow"
//! (identificado por un UUID) acumula una secuencia ordenada de eventos
//! (FlowEvent). Cada evento tiene:
//! - seq: número de secuencia creciente (0, 1, 2, ...)
//! - flow_id: identificador del flujo al que pertenece
//! - kind: el "tipo" o payload del evento (FlowEventKind)
//! - ts: marca de tiempo en UTC en el momento del append
//!
//! El almacén es "append-only": solo se agregan eventos al final. No hay
//! mutaciones ni borrados de eventos existentes. Esto facilita reproducir
//! el estado reconstruyendo desde el log de eventos.

use chrono::Utc; // Fuente de tiempo en UTC para timestamp de eventos.
use std::collections::HashMap;
use uuid::Uuid; // Identificador único para cada "flow".

use super::{FlowEvent, FlowEventKind};

/// Almacenamiento de eventos append-only para "flows".
///
/// Propósito:
/// - Registrar cronológicamente todo lo que sucede dentro de un flow.
/// - Permitir reconstruir el estado aplicando eventos en orden.
/// - Desacoplar la generación de eventos de su persistencia concreta.
///
/// Contrato:
/// - Los eventos para un mismo `flow_id` se entregan en orden de secuencia
///   ascendente.
/// - `append_kind` asigna automáticamente `seq` y `ts`.
pub trait EventStore {
    /// Agrega un evento a partir de su kind y devuelve el evento completo (con
    /// seq y ts).
    ///
    /// Parámetros:
    /// - `flow_id`: identifica el flujo al que se le agrega el evento.
    /// - `kind`: el contenido (tipo/payload) del evento.
    ///
    /// Efectos:
    /// - Calcula `seq` como el tamaño actual de la lista de eventos del flow
    ///   (0-based).
    /// - Asigna `ts` con `Utc::now()`.
    /// - Inserta el evento al final de la lista del flow.
    ///
    /// Retorna:
    /// - El `FlowEvent` recién persistido (incluyendo `seq` y `ts`).
    fn append_kind(&mut self, flow_id: Uuid, kind: FlowEventKind) -> FlowEvent;

    /// Lista eventos de un flujo (orden ascendente por `seq`).
    ///
    /// Parámetros:
    /// - `flow_id`: el flujo del cual obtener todos los eventos.
    ///
    /// Retorna:
    /// - Un vector de `FlowEvent` en orden de inserción. Si el flow no existe,
    ///   retorna un `Vec` vacío.
    ///
    /// Nota: el orden ascendente por `seq` está garantizado porque solo hacemos
    /// append en el vector interno.
    fn list(&self, flow_id: Uuid) -> Vec<FlowEvent>;
}

/// Implementación en memoria del almacén de eventos.
///
/// Estructura de datos:
/// - `inner`: HashMap<flow_id, Vec<FlowEvent>> Cada `flow_id` mapea a un vector
///   que mantiene los eventos en el orden exacto en que fueron agregados.
///
/// Consideraciones:
/// - Es volátil (no persistente). Se pierde al terminar el proceso.
/// - No es thread-safe por sí solo. Si se requiere acceso concurrente, envolver
///   en `Mutex`/`RwLock` o usar una implementación persistente/concurrente.
///
/// Complejidad:
/// - `append_kind`: O(1) promedio (insert en HashMap + push al Vec).
/// - `list`: O(n) para clonar los eventos del flow.
pub struct InMemoryEventStore {
    /// Mapa de cada flow a su lista ordenada de eventos.
    pub inner: HashMap<Uuid, Vec<FlowEvent>>,
}

impl Default for InMemoryEventStore {
    /// Crea un almacén vacío sin flows registrados.
    fn default() -> Self {
        Self { inner: HashMap::new() }
    }
}

impl EventStore for InMemoryEventStore {
    fn append_kind(&mut self, flow_id: Uuid, kind: FlowEventKind) -> FlowEvent {
        // Obtiene (o crea) el vector de eventos para este flow.
        // `or_insert_with(Vec::new)` inicializa un Vec vacío si el flow no existe aún.
        let vec = self.inner.entry(flow_id).or_default();

        // El siguiente número de secuencia es la longitud actual del vector (0-based).
        // Importante: si en otros lugares se espera 1-based, ajustar aquí.
        let seq = vec.len() as u64;

        // Construye el evento completo con su timestamp actual en UTC.
        let ev = FlowEvent { seq,
                             flow_id,
                             kind,
                             ts: Utc::now() };

        // Guardamos una copia dentro del vector para mantener la historia.
        // Usamos `clone()` para poder devolver también una copia por valor al llamante.
        // Alternativa sin clon extra:
        //   vec.push(ev);
        //   vec.last().cloned().unwrap()
        // pero este patrón actual es claro y seguro.
        vec.push(ev.clone());

        // Devolvemos el evento recién agregado (incluye seq y ts).
        ev
    }

    fn list(&self, flow_id: Uuid) -> Vec<FlowEvent> {
        // Recupera los eventos del flow si existen y los clona para devolver propiedad.
        // `cloned()` requiere que `FlowEvent` implemente `Clone`.
        // Si el flow no existe, devuelve un Vec vacío. La clonación desacopla al
        // consumidor del store interno evitando aliasing/mutaciones accidentales.
        self.inner.get(&flow_id).cloned().unwrap_or_default()
    }
}
