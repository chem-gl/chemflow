use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Error interno: {0}")]
    Internal(String),
    #[error("Error en IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("Error de configuración: {0}")]
    Config(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_internal_variant_format() {
        let err = CoreError::Internal("algo malo".into());
        assert_eq!(err.to_string(), "Error interno: algo malo");
    }

    #[test]
    fn test_io_variant_from() {
        let io_err = std::io::Error::other("falló IO");
        let err: CoreError = io_err.into();
        assert_eq!(err.to_string(), "Error en IO: falló IO");
    }

    #[test]
    fn test_config_variant_format() {
        let err = CoreError::Config("mala configuración".into());
        assert_eq!(err.to_string(), "Error de configuración: mala configuración");
    }
}
