## 15. Creación de un Step Completo (Ejemplo Canon)

### 15.1 Principios

Inmutabilidad, determinismo, validación temprana, fusión estable, runtime derivations fuera fingerprint, recovery y branching soportados.

### 15.2 Trait Conceptual

```rust
pub trait StepDefinition {
	fn id(&self) -> Uuid;
	fn name(&self) -> &str;
	fn kind(&self) -> StepKind;
	fn required_input_kinds(&self) -> &[ArtifactKind];
	fn base_params(&self) -> Value;
	fn validate_params(&self, merged: &Value) -> Result<Value, StepError>;
	fn run(&self, ctx: &mut ExecutionContext, params: &Value) -> Result<RunOutput, StepError>;
	fn fingerprint(&self, inputs: &[ArtifactRef], params: &Value) -> Fingerprint;
	fn rehydrate(meta: RehydrateMeta) -> Self where Self: Sized;
	fn clone_for_branch(&self) -> Self where Self: Sized;
}
```

### 15.3 Data Shapes

ExecutionContext, RunOutput, Artifact, RehydrateMeta definiciones conceptuales.

### 15.4 Ejemplo NormalizePropertiesStep

(Código completo reproducido.)

```rust
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StaticParams {
	pub min: f64,
	pub max: f64,
	pub strategy: String,
	pub seed: Option<u64>,
}

pub struct NormalizePropertiesStep {
	id: Uuid,
	static_params: StaticParams,
	internal_version: &'static str,
}

impl NormalizePropertiesStep {
	pub fn new(static_params: StaticParams) -> Self {
		Self { id: Uuid::new_v4(), static_params, internal_version: "v1" }
	}
	pub fn rehydrate_with(id: Uuid, static_params: StaticParams, internal_version: &'static str) -> Self {
		Self { id, static_params, internal_version }
	}
}

// ... implementación StepDefinition (idéntica al original) ...
```

### 15.5 Checklist

Tabla garantías (validación rangos, fingerprint, eventos, metadata).

### 15.6 Relación Diagramas

Conexión a Core, State Machine, ER y branching.

### 15.7 Errores y Retry

Tabla (Validation vs Runtime).

### 15.8 Extensión

Múltiples estrategias → múltiples artifacts.
