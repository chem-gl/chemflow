use thiserror::Error;

/// Errores del dominio de la aplicación
#[derive(Debug, Error)]
pub enum DomainError {
    #[error("Entidad no encontrada: {0}")]
    NotFound(String),
    #[error("Validación fallida: {0}")]
    Validation(String),
    #[error("Error genérico de dominio: {0}")]
    Generic(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_found_variant_format() {
        let err = DomainError::NotFound("RecursoX".into());
        assert_eq!(err.to_string(), "Entidad no encontrada: RecursoX");
    }

    #[test]
    fn test_validation_variant_format() {
        let err = DomainError::Validation("inválido".into());
        assert_eq!(err.to_string(), "Validación fallida: inválido");
    }

    #[test]
    fn test_generic_variant_format() {
        let err = DomainError::Generic("error genérico".into());
        assert_eq!(err.to_string(), "Error genérico de dominio: error genérico");
    }
}
