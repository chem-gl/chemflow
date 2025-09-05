//! Modelos neutrales (Artifact, Fingerprint, ExecutionContext,...)

pub mod artifact;
pub mod fingerprint;
pub mod context;
pub mod typed_artifact;

pub use artifact::{Artifact, ArtifactKind};
pub use fingerprint::StepFingerprintInput;
pub use context::ExecutionContext;
pub use typed_artifact::{ArtifactSpec, ArtifactDecodeError, TypedArtifact};
