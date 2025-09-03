//! ChemFlow Rust Library
//!
//! Este crate actúa como la librería central de ChemFlow:
//! - Expone `errors` para manejar errores de núcleo y dominio.
//! - Expone `hashing` para serializar JSON en forma canónica.
//!
//! Puede usarse desde `main.rs` o por otros crates/clientes.

pub mod errors;
pub mod hashing;

#[cfg(test)]
mod tests {
	use super::errors::{core_error::CoreError, domain_error::DomainError};

	#[test]
	fn core_error_tests() {
		let i = CoreError::Internal("fallo".into()).to_string();
		assert_eq!(i, "Error interno: fallo");
	}

	#[test]
	fn domain_error_tests() {
		let d = DomainError::Validation("x".into()).to_string();
		assert_eq!(d, "Validación fallida: x");
	}
}
