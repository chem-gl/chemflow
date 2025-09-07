//! Modelos neutrales (Artifact, Fingerprint, ExecutionContext,...)
//!
//! Propósito en el flujo:
//! - Representar datos de entrada/salida de Steps como `Artifact` neutro (JSON
//!   + hash) sin semántica de dominio.
//! - Proveer `ExecutionContext` que entrega input y params al `StepDefinition`.
//! - Ofrecer tipado fuerte opcional (`ArtifactSpec`, `TypedArtifact`) para
//!   ergonomía y validación sin contaminar el núcleo con tipos del dominio.

pub mod artifact;
pub mod context;
pub mod fingerprint;
pub mod typed_artifact;

pub use artifact::{Artifact, ArtifactKind};
pub use context::ExecutionContext;
pub use fingerprint::StepFingerprintInput;
pub use typed_artifact::{ArtifactDecodeError, ArtifactSpec, TypedArtifact};
