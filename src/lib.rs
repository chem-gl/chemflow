pub mod errors;
pub mod hashing;

#[cfg(test)]
mod tests {
    use core::{assert_eq, convert::Into};
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
