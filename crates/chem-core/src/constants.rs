//! Constantes del motor core.
//!
//! Este módulo agrupa valores estáticos que participan en el cálculo de
//! fingerprints y en la compatibilidad entre versiones del motor. Cambios en
//! estas constantes pueden afectar la reproducibilidad si forman parte del
//! input del hashing (por diseño, `ENGINE_VERSION` sí lo es).

/// Versión lógica del motor (F2). Se incluye en el `StepFingerprintInput` para
/// asegurar que un cambio de versión del engine invalide/recalcule
/// determinísticamente los fingerprints aunque la definición y los datos no
/// cambien. Mantener estable mientras no haya cambios incompatibles.
pub const ENGINE_VERSION: &str = "F2.0";
