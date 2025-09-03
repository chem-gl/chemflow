# Sección 15 - Creación de un Step Completo (Ejemplo Canon)

Objetivo: guiar la implementación de un Step alineado con los diagramas (clases core, eventos, BD) garantizando: determinismo, reproducibilidad, branching limpio, recovery y desacoplamiento dominio.

### 15.1 Principios (Refuerzo)

- Inmutabilidad: artifacts producidos no se mutan ni se re‑emiten con mismo hash alterado.
- Determinismo: fingerprint depende sólo de inputs + parámetros canonizados + versiones internas.
- Validación temprana: `validate_params` separa errores de configuración de fallos runtime.
- Fusión estable: orden de merge fijo (base → injectors → overrides → gate → runtime\*).
- Runtime derivations: no alteran fingerprint (se reflejan sólo en metadata/artifact payload).
- Recovery: `rehydrate` reproduce misma identidad (UUID) sin lógica adicional.
- Branching: `clone_for_branch` cambia UUID, mantiene configuración base.

### 15.2 Contrato Minimal (Trait Conceptual)

```rust
pub trait StepDefinition {
    fn id(&self) -> Uuid;                // Identificador estable (persistencia / eventos)
    fn name(&self) -> &str;              // Nombre legible
    fn kind(&self) -> StepKind;          // Clasificación semántica
    fn required_input_kinds(&self) -> &[ArtifactKind];
    fn base_params(&self) -> Value;      // Declaración estática inicial
    fn validate_params(&self, merged: &Value) -> Result<Value, StepError>;
    fn run(&self, ctx: &mut ExecutionContext, params: &Value) -> Result<RunOutput, StepError>;
    fn fingerprint(&self, inputs: &[ArtifactRef], params: &Value) -> Fingerprint;
    fn rehydrate(meta: RehydrateMeta) -> Self where Self: Sized;      // Recovery
    fn clone_for_branch(&self) -> Self where Self: Sized;             // Branching
}
```

### 15.3 Data Shapes Relacionados

| Tipo             | Contenido                               | Nota                                          |
| ---------------- | --------------------------------------- | --------------------------------------------- |
| ExecutionContext | inputs[], params merged, event_sink     | No persistente                                |
| RunOutput        | artifacts[], metadata JSON              | metadata se persiste parcial (según política) |
| Artifact         | id, kind, hash, payload, metadata       | hash = función canónica determinista          |
| RehydrateMeta    | id, serialized_params, internal_version | Fuente para rehidratar                        |

### 15.4 Ejemplo: `NormalizePropertiesStep`

Propósito: Normalizar una tabla de propiedades moleculares generando un artifact `NormalizedProperties`.

```rust
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StaticParams {
    pub min: f64,
    pub max: f64,
    pub strategy: String,      // "zscore" | "minmax"
    pub seed: Option<u64>,
}

pub struct NormalizePropertiesStep {
    id: Uuid,
    static_params: StaticParams,
    internal_version: &'static str, // para fingerprint / migraciones
}

impl NormalizePropertiesStep {
    pub fn new(static_params: StaticParams) -> Self {
        Self { id: Uuid::new_v4(), static_params, internal_version: "v1" }
    }
    pub fn rehydrate_with(id: Uuid, static_params: StaticParams, internal_version: &'static str) -> Self {
        Self { id, static_params, internal_version }
    }
}

impl StepDefinition for NormalizePropertiesStep {
    fn id(&self) -> Uuid { self.id }
    fn name(&self) -> &str { "NormalizeProperties" }
    fn kind(&self) -> StepKind { StepKind::Custom("NormalizeProperties") }
    fn required_input_kinds(&self) -> &[ArtifactKind] {
        const KINDS: &[ArtifactKind] = &[ArtifactKind::PropertiesTable];
        KINDS
    }
    fn base_params(&self) -> Value { serde_json::to_value(&self.static_params).unwrap() }

    fn validate_params(&self, merged: &Value) -> Result<Value, StepError> {
        let min = merged.get("min").and_then(|v| v.as_f64()).ok_or_else(|| StepError::invalid("missing min"))?;
        let max = merged.get("max").and_then(|v| v.as_f64()).ok_or_else(|| StepError::invalid("missing max"))?;
        if !(min < max) { return Err(StepError::invalid("min must be < max")); }
        let strategy = merged.get("strategy").and_then(|v| v.as_str()).unwrap_or("");
        if strategy != "zscore" && strategy != "minmax" { return Err(StepError::invalid("unsupported strategy")); }
        if let Some(dt) = merged.get("dynamic_threshold").and_then(|v| v.as_f64()) {
            if dt < 0.0 || dt > 1.0 { return Err(StepError::invalid("dynamic_threshold out of [0,1]")); }
        }
        Ok(merged.clone())
    }

    fn run(&self, ctx: &mut ExecutionContext, params: &Value) -> Result<RunOutput, StepError> {
        if ctx.inputs.is_empty() { return Err(StepError::invalid("no input artifacts")); }
        let derived_cutoff = ctx.inputs.first()
            .and_then(|a| a.metadata.get("mean_logP").and_then(|v| v.as_f64()))
            .unwrap_or(0.5);
        ctx.event_sink.emit(FlowEventPayload::ProviderInvoked {
            provider_id: "norm-core".into(),
            version: self.internal_version.into(),
            params_hash: short_hash(params),
        });
        let artifact_payload = json!({
            "strategy": params.get("strategy").cloned().unwrap_or(Value::String("minmax".into())),
            "derived_cutoff": derived_cutoff,
            "normalized_count": 1234,
        });
        let artifact_hash = hash_json(&artifact_payload);
        let artifact = Artifact::new(
            ArtifactKind::NormalizedProperties,
            artifact_hash.clone(),
            artifact_payload,
            json!({"source_step": self.id, "schema_version": 1})
        );
        ctx.event_sink.emit(FlowEventPayload::ArtifactCreated {
            artifact_id: artifact.id,
            kind: artifact.kind,
            hash: artifact.hash.clone(),
        });
        Ok(RunOutput { artifacts: vec![artifact], metadata: json!({"derived_cutoff": derived_cutoff}) })
    }

    fn fingerprint(&self, inputs: &[ArtifactRef], params: &Value) -> Fingerprint {
        let mut hashes: Vec<&str> = inputs.iter().map(|r| r.hash.as_str()).collect();
        hashes.sort();
        let canonical = canonical_json(params);
        Fingerprint::new(format!("{}|{}|{}", hashes.join("+"), canonical, self.internal_version))
    }

    fn rehydrate(meta: RehydrateMeta) -> Self where Self: Sized {
        let static_params: StaticParams = serde_json::from_value(meta.serialized_params)
            .expect("params deserialize");
        Self::rehydrate_with(meta.id, static_params, meta.internal_version)
    }

    fn clone_for_branch(&self) -> Self where Self: Sized {
        Self { id: Uuid::new_v4(), static_params: self.static_params.clone(), internal_version: self.internal_version }
    }
}
```

### 15.5 Checklist Específico del Ejemplo

| Aspecto               | Garantía                                   | Dónde               |
| --------------------- | ------------------------------------------ | ------------------- |
| Validación rangos     | min < max                                  | validate_params     |
| Estrategia soportada  | enum controlado                            | validate_params     |
| Fingerprint estable   | inputs ordenados + json canónico + versión | fingerprint         |
| Evento observabilidad | ProviderInvoked emitido                    | run                 |
| Creación artifact     | ArtifactCreated emitido                    | run                 |
| Metadata mínima       | source_step + schema_version               | run (Artifact::new) |
| Campos adicionales    | derived_cutoff, normalized_count           | run                 |

### 15.6 Relación con Diagramas

- Diagrama de clases Core: usa StepDefinition, Artifact, ExecutionContext.
- State Machine: transiciones StepStarted / StepCompleted / StepFailed aplican igual.
- ER: artifact → WORKFLOW_STEP_ARTIFACTS; evento ProviderInvoked → EVENT_LOG.
- Fingerprint participa en branching (sección 10) al compararse contra ejecuciones previas.

### 15.7 Errores y Estrategias de Retry

| Error              | Tipo       | Acción                             |
| ------------------ | ---------- | ---------------------------------- |
| Parámetro inválido | Validation | Emite StepValidationFailed, no run |
| Falta input        | Validation | Igual que arriba                   |
| Excepción cálculo  | Runtime    | StepFailed, elegible retry         |

### 15.8 Extensión Posterior

Para soporte de múltiples normalizaciones simultáneas: producir varios artifacts (uno por estrategia) — cada uno con su propio hash y evento ArtifactCreated.

---

