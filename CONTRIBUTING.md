# Contribuyendo a ChemFlow

¡Gracias por tu interés en contribuir a ChemFlow! Sigue los pasos a continuación para configurar tu entorno de desarrollo y enviar cambios.

## Guía de Contribución

1. Fork del repositorio y crea una rama para tu característica o corrección de errores:

   ```bash
   git checkout -b feature/nombre-de-tu-funcionalidad
   ```

2. Asegúrate de que tu código sigue el estilo del proyecto:
   - Formatea el código: `cargo fmt`
   - Verifica con clippy: `cargo clippy --all-targets --all-features -- -D warnings`
3. Agrega pruebas para nuevos comportamientos o correcciones de errores.
4. Ejecuta las pruebas:

   ```bash
   cargo test --all-features
   ```

5. Envía tu pull request describiendo los cambios propuestos y por qué son necesarios.
6. Un mantenedor revisará tu PR y podrá solicitar cambios antes de integrarlo.

## Código de Conducta

Por favor, lee y respeta el [Código de Conducta](CODE_OF_CONDUCT.md) del proyecto.
