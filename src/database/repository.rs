//! Repositorio de persistencia para ejecuciones de steps y familias.
//! Proporciona almacenamiento en memoria (rápido para tests y prototipos) y, si
//! se inicializa con un pool PostgreSQL, persiste también en base de datos.
//!
//! Responsabilidades clave:
//! - Guardar metadatos de ejecución (StepExecutionInfo) con parámetros y
//!   proveedores.
//! - Upsert de familias de moléculas con sus propiedades y proveedor fuente.
//! - Guardar relación many-to-many step <-> familia para reconstruir flujos e
//!   historial.
//! - Recuperar familias y ejecuciones por ID, así como filtrar por
//!   root_execution_id para reconstruir un árbol/linaje completo, incluyendo
//!   ramas.
//! - Soporte para branching mediante duplicación controlada de step_id en ramas
//!   (save_step_execution_for_branch).
//!
//! Notas de Trazabilidad:
//! Cada propiedad almacenada en una familia lleva un ProviderReference que
//! señala proveedor, versión, parámetros de ejecución y execution_id único.
//! Esto permite auditoría y reproducibilidad independiente del step que la
//! generó.
use crate::data::family::{MoleculeFamily, ProviderReference};
use crate::molecule::Molecule;
use crate::workflow::step::{StepExecutionInfo, StepStatus};
use sha2::{Digest, Sha256};
use sqlx::Row; // Para acceso dinámico a columnas al usar sqlx::query en lugar de query! macro
use std::collections::HashMap;
use uuid::Uuid;

pub fn compute_sorted_hash<T: serde::Serialize>(value: &T) -> String {
    let json = serde_json::to_value(value).unwrap_or(serde_json::Value::Null);
    let canonical = canonical_json(&json);
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn canonical_json(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<_> = map.keys().collect();
            keys.sort();
            let inner: Vec<String> = keys.into_iter().map(|k| format!("\"{}\":{}", k, canonical_json(&map[k]))).collect();
            format!("{{{}}}", inner.join(","))
        }
        serde_json::Value::Array(arr) => {
            let inner: Vec<String> = arr.iter().map(canonical_json).collect();
            format!("[{}]", inner.join(","))
        }
        _ => v.to_string(),
    }
}

#[derive(Clone)]
pub struct WorkflowExecutionRepository {
    in_memory: std::sync::Arc<tokio::sync::RwLock<HashMap<Uuid, Vec<StepExecutionInfo>>>>,
    pub pool: Option<sqlx::Pool<sqlx::Postgres>>,
}

impl WorkflowExecutionRepository {
    pub fn new(_in_memory_only: bool) -> Self {
        Self { in_memory: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
               pool: None /* in-memory only (placeholder for future pool wiring using flag) */ }
    }

    pub async fn with_pool(pool: sqlx::Pool<sqlx::Postgres>) -> Self {
        Self { in_memory: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
               pool: Some(pool) }
    }

    pub async fn save_step_execution(&self, execution: &StepExecutionInfo) -> Result<(), Box<dyn std::error::Error>> {
        // 1. Siempre guarda en memoria para acceso rápido (cache de sesiones /
        //    pruebas).
        {
            let mut guard = self.in_memory.write().await;
            guard.entry(execution.step_id).or_default().push(execution.clone());
        }
        if let Some(pool) = &self.pool {
            // Asegura que las tablas básicas existan (defensa ante BD recién creada sin
            // migraciones aplicadas).
            self.ensure_core_schema(pool).await?;
            // Calcular integrity_ok si no viene seteado (compara hash almacenado con hash recomputado de parámetros)
            let integrity_status = execution.integrity_ok.unwrap_or_else(|| {
                if let Some(h) = &execution.parameter_hash { h == &compute_sorted_hash(&execution.parameters) } else { true }
            });
            // 2. Persistencia en PostgreSQL: la tabla workflow_step_executions debe existir
            //    (creada por migraciones). ON CONFLICT permite actualizar estado y
            //    parámetros si se re-ejecuta un step o cambia su estado (ej: Running ->
            //    Completed).
                        let failure_message: Option<String> = match &execution.status { StepStatus::Failed(msg) => Some(msg.clone()), _ => None };
                        let insert_res = sqlx::query(
                                "INSERT INTO workflow_step_executions (step_id, name, description, status, failure_message, parameters, providers_used, start_time, end_time, parameter_hash, root_execution_id, parent_step_id, branch_from_step_id, input_family_ids, input_snapshot, step_config, integrity_ok)
                                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)
                                 ON CONFLICT (step_id) DO UPDATE SET \
                                     name = EXCLUDED.name, \
                                     description = EXCLUDED.description, \
                                     status = EXCLUDED.status, \
                                     failure_message = EXCLUDED.failure_message, \
                                     end_time = EXCLUDED.end_time, \
                                     parameters = EXCLUDED.parameters, \
                                     providers_used = EXCLUDED.providers_used, \
                                     parameter_hash = EXCLUDED.parameter_hash, \
                                     root_execution_id = EXCLUDED.root_execution_id, \
                                     parent_step_id = EXCLUDED.parent_step_id, \
                                     branch_from_step_id = EXCLUDED.branch_from_step_id, \
                                     input_family_ids = EXCLUDED.input_family_ids, \
                                     input_snapshot = EXCLUDED.input_snapshot, \
                                     step_config = EXCLUDED.step_config, \
                                     integrity_ok = EXCLUDED.integrity_ok",
                        ).bind(execution.step_id)
                         .bind(&execution.step_name)
                         .bind(&execution.step_description)
             .bind(match &execution.status {
                       StepStatus::Pending => "Pending",
                       StepStatus::Running => "Running",
                       StepStatus::Completed => "Completed",
                       StepStatus::Failed(_) => "Failed",
                   })
             .bind(&failure_message)
             .bind(serde_json::to_value(&execution.parameters)?)
             .bind(serde_json::to_value(&execution.providers_used)?)
             .bind(execution.start_time)
             .bind(execution.end_time)
             .bind(&execution.parameter_hash)
                 .bind(execution.root_execution_id)
                 .bind(execution.parent_step_id)
                 .bind(execution.branch_from_step_id)
                 .bind(serde_json::to_value(&execution.input_family_ids)?)
                  .bind(execution.input_snapshot.as_ref().map(serde_json::to_value).transpose()?)
                  .bind(execution.step_config.as_ref().map(serde_json::to_value).transpose()?)
                  .bind(integrity_status)
             .execute(pool)
             .await;
            if let Err(e) = insert_res {
                // Si la columna input_snapshot aún no existe (BD antigua), reintentar sin ella.
                if let Some(db_err) = e.as_database_error() { if db_err.code().map(|c| c == "42703").unwrap_or(false) {
                    sqlx::query(
                        "INSERT INTO workflow_step_executions (step_id, name, description, status, failure_message, parameters, providers_used, start_time, end_time, parameter_hash, root_execution_id, parent_step_id, branch_from_step_id, input_family_ids, step_config, integrity_ok)
                         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)
                         ON CONFLICT (step_id) DO UPDATE SET \
                             name = EXCLUDED.name, \
                             description = EXCLUDED.description, \
                             status = EXCLUDED.status, \
                             failure_message = EXCLUDED.failure_message, \
                             end_time = EXCLUDED.end_time, \
                             parameters = EXCLUDED.parameters, \
                             providers_used = EXCLUDED.providers_used, \
                             parameter_hash = EXCLUDED.parameter_hash, \
                             root_execution_id = EXCLUDED.root_execution_id, \
                             parent_step_id = EXCLUDED.parent_step_id, \
                             branch_from_step_id = EXCLUDED.branch_from_step_id, \
                             input_family_ids = EXCLUDED.input_family_ids, \
                             step_config = EXCLUDED.step_config, \
                             integrity_ok = EXCLUDED.integrity_ok"
                    ).bind(execution.step_id)
                     .bind(&execution.step_name)
                     .bind(&execution.step_description)
                     .bind(match &execution.status {
                         StepStatus::Pending => "Pending",
                         StepStatus::Running => "Running",
                         StepStatus::Completed => "Completed",
                         StepStatus::Failed(_) => "Failed",
                     })
                     .bind(&failure_message)
                     .bind(serde_json::to_value(&execution.parameters)?)
                     .bind(serde_json::to_value(&execution.providers_used)?)
                     .bind(execution.start_time)
                     .bind(execution.end_time)
                     .bind(&execution.parameter_hash)
                     .bind(execution.root_execution_id)
                     .bind(execution.parent_step_id)
                     .bind(execution.branch_from_step_id)
                     .bind(serde_json::to_value(&execution.input_family_ids)?)
                     .bind(execution.step_config.as_ref().map(serde_json::to_value).transpose()?)
                     .bind(integrity_status)
                     .execute(pool)
                     .await?;
                } else { return Err(e.into()); }}
            }
        }
        Ok(())
    }

    async fn ensure_core_schema(&self, pool: &sqlx::Pool<sqlx::Postgres>) -> Result<(), Box<dyn std::error::Error>> {
        // Creamos tablas críticas con IF NOT EXISTS para ser idempotente.
        // 0001
    sqlx::query("CREATE TABLE IF NOT EXISTS workflow_step_executions ( step_id UUID PRIMARY KEY, name TEXT NOT NULL, description TEXT, status TEXT NOT NULL, failure_message TEXT NULL, parameters JSONB NOT NULL DEFAULT '{}'::jsonb, providers_used JSONB NOT NULL DEFAULT '[]'::jsonb, start_time TIMESTAMPTZ NOT NULL, end_time TIMESTAMPTZ NOT NULL, parameter_hash TEXT, input_snapshot JSONB, step_config JSONB, integrity_ok BOOLEAN )").execute(pool).await?;
    let _ = sqlx::query("ALTER TABLE workflow_step_executions ADD COLUMN IF NOT EXISTS failure_message TEXT").execute(pool).await;
        // Añadir columnas nuevas para branching/lineage si no existen
    let _ = sqlx::query("ALTER TABLE workflow_step_executions ADD COLUMN IF NOT EXISTS root_execution_id UUID").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE workflow_step_executions ADD COLUMN IF NOT EXISTS parent_step_id UUID").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE workflow_step_executions ADD COLUMN IF NOT EXISTS branch_from_step_id UUID").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE workflow_step_executions ADD COLUMN IF NOT EXISTS input_family_ids JSONB").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE workflow_step_executions ADD COLUMN IF NOT EXISTS input_snapshot JSONB").execute(pool).await; // new snapshot column
    let _ = sqlx::query("ALTER TABLE workflow_step_executions ADD COLUMN IF NOT EXISTS step_config JSONB").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE workflow_step_executions ADD COLUMN IF NOT EXISTS integrity_ok BOOLEAN").execute(pool).await;
        sqlx::query("CREATE TABLE IF NOT EXISTS molecule_families ( id UUID PRIMARY KEY, name TEXT NOT NULL, description TEXT, molecules JSONB NOT NULL DEFAULT '[]'::jsonb, properties JSONB NOT NULL DEFAULT '{}'::jsonb, parameters JSONB NOT NULL DEFAULT '{}'::jsonb, provenance JSONB, frozen BOOLEAN NOT NULL DEFAULT FALSE, frozen_at TIMESTAMPTZ NULL, family_hash TEXT )").execute(pool).await?;
        // 0002 link
        sqlx::query("CREATE TABLE IF NOT EXISTS workflow_step_family ( step_id UUID NOT NULL REFERENCES workflow_step_executions(step_id) ON DELETE CASCADE, family_id UUID NOT NULL REFERENCES molecule_families(id) ON DELETE CASCADE, PRIMARY KEY (step_id, family_id) )").execute(pool).await?;
        // 0003 properties & results
        sqlx::query("CREATE TABLE IF NOT EXISTS molecule_family_properties ( family_id UUID NOT NULL REFERENCES molecule_families(id) ON DELETE CASCADE, property_name TEXT NOT NULL, value DOUBLE PRECISION, source TEXT, frozen BOOLEAN DEFAULT FALSE, timestamp TIMESTAMPTZ NOT NULL DEFAULT now(), PRIMARY KEY (family_id, property_name, timestamp) )").execute(pool).await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS workflow_step_results ( step_id UUID NOT NULL REFERENCES workflow_step_executions(step_id) ON DELETE CASCADE, result_key TEXT NOT NULL, result_value JSONB NOT NULL, result_type TEXT NOT NULL DEFAULT 'raw', PRIMARY KEY (step_id, result_key) )").execute(pool).await?;
        // 0004 molecules normalization
        sqlx::query("CREATE TABLE IF NOT EXISTS molecules ( inchikey TEXT PRIMARY KEY, inchi TEXT NOT NULL, smiles TEXT NOT NULL, common_name TEXT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT now(), updated_at TIMESTAMPTZ NOT NULL DEFAULT now() )").execute(pool).await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS molecule_family_molecules ( family_id UUID NOT NULL REFERENCES molecule_families(id) ON DELETE CASCADE, molecule_inchikey TEXT NOT NULL REFERENCES molecules(inchikey) ON DELETE CASCADE, position INT NOT NULL DEFAULT 0, PRIMARY KEY (family_id, molecule_inchikey) )").execute(pool).await?;
        // Índices esenciales (idempotentes)
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_workflow_step_status ON workflow_step_executions(status)").execute(pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_workflow_step_start_time ON workflow_step_executions(start_time)").execute(pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_family_properties_name ON molecule_family_properties(property_name)").execute(pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_molecule_family_molecules_family ON molecule_family_molecules(family_id)").execute(pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_molecule_family_molecules_inchikey ON molecule_family_molecules(molecule_inchikey)").execute(pool).await?;
        // GIN optional
        let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_workflow_step_executions_providers_used_gin ON workflow_step_executions USING GIN (providers_used jsonb_path_ops)").execute(pool)
                                                                                                                                                                               .await; // ignorar fallo si extensión no disponible
                                                                                                                                                                                       // Normalized property provenance tables (needed so upsert_family transaction
                                                                                                                                                                                       // doesn't rollback if migrations not applied)
        sqlx::query("CREATE TABLE IF NOT EXISTS molecule_family_property_providers ( family_id UUID NOT NULL REFERENCES molecule_families(id) ON DELETE CASCADE, property_name TEXT NOT NULL, provider_type TEXT NOT NULL, provider_name TEXT NOT NULL, provider_version TEXT NOT NULL, execution_parameters JSONB NOT NULL DEFAULT '{}'::jsonb, execution_id UUID NOT NULL, PRIMARY KEY (family_id, property_name, execution_id) )").execute(pool).await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS molecule_family_property_steps ( family_id UUID NOT NULL REFERENCES molecule_families(id) ON DELETE CASCADE, property_name TEXT NOT NULL, step_id UUID NOT NULL, PRIMARY KEY (family_id, property_name, step_id) )").execute(pool).await?;
        Ok(())
    }

    /// Acceso de solo lectura al pool (principalmente para tests de
    /// integración).
    pub fn pool(&self) -> Option<&sqlx::Pool<sqlx::Postgres>> {
        self.pool.as_ref()
    }

    pub async fn upsert_family(&self, family: &MoleculeFamily) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(pool) = &self.pool {
            // Debug: molecule count before
            let before: Option<i64> = match sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM molecules").fetch_one(pool).await {
                Ok((c,)) => Some(c),
                Err(_) => None,
            };
            // Normalización: usamos transacción para mantener consistencia entre tablas.
            let mut tx = pool.begin().await?;
            // 1. Upsert de la familia primero (evita violar FK al insertar relaciones de
            //    moléculas).
            let inchikeys: Vec<&String> = family.molecules.iter().map(|m| &m.inchikey).collect();
            eprintln!("[upsert_family] upserting family row {} (molecule_count={})", family.id, inchikeys.len());
            sqlx::query(
             "INSERT INTO molecule_families (id, name, description, molecules, properties, parameters, provenance, frozen, frozen_at, family_hash)
              VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
              ON CONFLICT (id) DO UPDATE SET name=EXCLUDED.name, description=EXCLUDED.description, molecules=EXCLUDED.molecules, properties=EXCLUDED.properties, parameters=EXCLUDED.parameters, provenance=EXCLUDED.provenance, frozen=EXCLUDED.frozen, frozen_at=EXCLUDED.frozen_at, family_hash=EXCLUDED.family_hash"
            )
            .bind(family.id)
            .bind(&family.name)
            .bind(&family.description)
            .bind(serde_json::to_value(&inchikeys)?)
            .bind(serde_json::to_value(&family.properties)?)
            .bind(serde_json::to_value(&family.parameters)?)
            .bind(serde_json::to_value(&family.provenance)?)
            .bind(family.frozen)
            .bind(family.frozen_at)
            .bind(&family.family_hash)
            .execute(&mut *tx)
            .await?;

            // 2. Upsert de moléculas + relaciones ahora que la fila de familia existe.
            for (idx, mol) in family.molecules.iter().enumerate() {
                eprintln!("[upsert_family] inserting molecule {} ({}/{})", mol.inchikey, idx + 1, family.molecules.len());
                sqlx::query(
                            "INSERT INTO molecules (inchikey, inchi, smiles, common_name) VALUES ($1,$2,$3,$4)
                     ON CONFLICT (inchikey) DO UPDATE SET inchi = EXCLUDED.inchi, smiles = EXCLUDED.smiles, common_name = EXCLUDED.common_name, updated_at = now()",
                ).bind(&mol.inchikey)
                 .bind(&mol.inchi)
                 .bind(&mol.smiles)
                 .bind(&mol.common_name)
                 .execute(&mut *tx)
                 .await?;
                sqlx::query(
                            "INSERT INTO molecule_family_molecules (family_id, molecule_inchikey, position) VALUES ($1,$2,$3)
                     ON CONFLICT (family_id, molecule_inchikey) DO UPDATE SET position = EXCLUDED.position",
                ).bind(family.id)
                 .bind(&mol.inchikey)
                 .bind(idx as i32)
                 .execute(&mut *tx)
                 .await?;
            }
            eprintln!("[upsert_family] completed molecules link for family {} total={}", family.id, family.molecules.len());

            // 3. Propiedades (igual que antes) -> tabla flatten.
            for (prop_name, entry) in &family.properties {
                for value in &entry.values {
                    sqlx::query(
                                "INSERT INTO molecule_family_properties (family_id, property_name, value, source, frozen, timestamp)
                         VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING",
                    ).bind(family.id)
                     .bind(prop_name)
                     .bind(value.value)
                     .bind(&value.source)
                     .bind(value.frozen)
                     .bind(value.timestamp)
                     .execute(&mut *tx)
                     .await?;
                }
                // Proveniencia normalizada: proveedores
                for prov in &entry.providers {
                    sqlx::query(
                                "INSERT INTO molecule_family_property_providers (family_id, property_name, provider_type, provider_name, provider_version, execution_parameters, execution_id)
                         VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING",
                    ).bind(family.id)
                     .bind(prop_name)
                     .bind(&prov.provider_type)
                     .bind(&prov.provider_name)
                     .bind(&prov.provider_version)
                     .bind(serde_json::to_value(&prov.execution_parameters)?)
                     .bind(prov.execution_id)
                     .execute(&mut *tx)
                     .await?;
                }
                // Steps originantes
                for sid in &entry.originating_steps {
                    sqlx::query("INSERT INTO molecule_family_property_steps (family_id, property_name, step_id) VALUES ($1,$2,$3) ON CONFLICT DO NOTHING").bind(family.id)
                                                                                                                                                          .bind(prop_name)
                                                                                                                                                          .bind(sid)
                                                                                                                                                          .execute(&mut *tx)
                                                                                                                                                          .await?;
                }
            }

            tx.commit().await?;
            if let (Some(bc), Ok((ac,))) = (before, sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM molecules").fetch_one(pool).await) {
                eprintln!("[upsert_family] molecule count before={} after={} delta={} fam_id={}", bc, ac, ac - bc, family.id);
            }
        }
        Ok(())
    }

    pub async fn link_step_family(&self, step_id: Uuid, family_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(pool) = &self.pool {
            // Relación step <-> familia para reconstruir qué familias se generaron /
            // modificaron en cada step.
            sqlx::query("INSERT INTO workflow_step_family (step_id, family_id) VALUES ($1,$2) ON CONFLICT DO NOTHING").bind(step_id)
                                                                                                                      .bind(family_id)
                                                                                                                      .execute(pool)
                                                                                                                      .await?;
        }
        Ok(())
    }

    pub async fn get_family(&self, id: Uuid) -> Result<Option<MoleculeFamily>, Box<dyn std::error::Error>> {
        if let Some(pool) = &self.pool {
            // Recuperamos metadatos de la familia (sin las moléculas aún).
            let row_opt = sqlx::query("SELECT id, name, description, properties, parameters, provenance, frozen, frozen_at, family_hash FROM molecule_families WHERE id = $1").bind(id)
                                                                                                                                                                              .fetch_optional(pool)
                                                                                                                                                                              .await?;

            if let Some(row) = row_opt {
                let id: Uuid = row.try_get("id")?;
                let name: String = row.try_get("name")?;
                let description: Option<String> = row.try_get("description")?;
                let properties_val: serde_json::Value = row.try_get("properties")?;
                let parameters_val: serde_json::Value = row.try_get("parameters")?;
                let provenance_val: Option<serde_json::Value> = row.try_get("provenance")?;
                let frozen: bool = row.try_get("frozen")?;
                let frozen_at: Option<chrono::DateTime<chrono::Utc>> = row.try_get("frozen_at")?;
                let family_hash: Option<String> = row.try_get("family_hash")?;

                // Ahora obtenemos las moléculas vía la tabla normalizada.
                let molecule_rows = sqlx::query(
                                                "SELECT m.inchikey, m.inchi, m.smiles, m.common_name FROM molecule_family_molecules fm
                     JOIN molecules m ON m.inchikey = fm.molecule_inchikey WHERE fm.family_id = $1 ORDER BY fm.position ASC",
                ).bind(id)
                                    .fetch_all(pool)
                                    .await?;
                let mut molecules: Vec<Molecule> = Vec::with_capacity(molecule_rows.len());
                for r in molecule_rows {
                    molecules.push(Molecule { inchikey: r.try_get("inchikey")?,
                                              inchi: r.try_get("inchi")?,
                                              smiles: r.try_get("smiles")?,
                                              common_name: r.try_get("common_name")? });
                }

                // Interpret JSON null (Some(Value::Null)) as absence of provenance to avoid
                // deserialization error.
                let provenance: Option<crate::data::family::FamilyProvenance> = match provenance_val {
                    None => None,
                    Some(serde_json::Value::Null) => None,
                    Some(v) => Some(serde_json::from_value(v)?),
                };
                let mut family: MoleculeFamily = MoleculeFamily { id,
                                                                  name,
                                                                  description,
                                                                  molecules,
                                                                  properties: serde_json::from_value(properties_val)?,
                                                                  parameters: serde_json::from_value(parameters_val)?,
                                                                  provenance,
                                                                  frozen,
                                                                  frozen_at,
                                                                  family_hash };
                // Reconstruir providers y steps normalizados (merge sobre lo ya deserializado
                // de properties JSON) Cargamos todos los providers y steps y
                // los agregamos/evitamos duplicados.
                let provider_rows = sqlx::query("SELECT property_name, provider_type, provider_name, provider_version, execution_parameters, execution_id FROM molecule_family_property_providers WHERE family_id = $1").bind(id)
                                                                                                                                                                                                                        .fetch_all(pool)
                                                                                                                                                                                                                        .await?;
                let mut prov_map: HashMap<String, Vec<ProviderReference>> = HashMap::new();
                for r in provider_rows {
                    let property_name: String = r.try_get("property_name")?;
                    let pr = ProviderReference { provider_type: r.try_get("provider_type")?,
                                                 provider_name: r.try_get("provider_name")?,
                                                 provider_version: r.try_get("provider_version")?,
                                                 execution_parameters: serde_json::from_value(r.try_get("execution_parameters")?)?,
                                                 execution_id: r.try_get("execution_id")? };
                    prov_map.entry(property_name).or_default().push(pr);
                }
                let step_rows = sqlx::query("SELECT property_name, step_id FROM molecule_family_property_steps WHERE family_id = $1").bind(id).fetch_all(pool).await?;
                let mut step_map: HashMap<String, Vec<uuid::Uuid>> = HashMap::new();
                for r in step_rows {
                    let property_name: String = r.try_get("property_name")?;
                    let sid: uuid::Uuid = r.try_get("step_id")?;
                    step_map.entry(property_name).or_default().push(sid);
                }
                // Fusionar en family.properties
                for (prop_name, prop_entry) in family.properties.iter_mut() {
                    if let Some(provs) = prov_map.get(prop_name) {
                        // Evitar duplicados por execution_id
                        let existing_ids: std::collections::HashSet<_> = prop_entry.providers.iter().map(|p| p.execution_id).collect();
                        for pr in provs {
                            if !existing_ids.contains(&pr.execution_id) {
                                prop_entry.providers.push(pr.clone());
                            }
                        }
                    }
                    if let Some(steps) = step_map.get(prop_name) {
                        let existing: std::collections::HashSet<_> = prop_entry.originating_steps.iter().cloned().collect();
                        for sid in steps {
                            if !existing.contains(sid) {
                                prop_entry.originating_steps.push(*sid);
                            }
                        }
                    }
                }
                return Ok(Some(family));
            }
        }
        Ok(None)
    }

    /// Congela una familia existente marcando frozen, timestamp y recalculando
    /// hash.
    pub async fn freeze_family(&self, family_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(mut fam) = self.get_family(family_id).await? {
            if fam.frozen {
                return Ok(());
            }
            fam.frozen = true;
            fam.frozen_at = Some(chrono::Utc::now());
            let hash = compute_sorted_hash(&serde_json::json!({
                                               "id": fam.id,
                                               "molecules": fam.molecules.iter().map(|m| &m.inchikey).collect::<Vec<_>>(),
                                               "parameters": fam.parameters,
                                               "properties": fam.properties.keys().collect::<Vec<_>>(),
                                               "frozen": fam.frozen,
                                               "frozen_at": fam.frozen_at,
                                           }));
            fam.family_hash = Some(hash);
            self.upsert_family(&fam).await?;
        }
        Ok(())
    }

    /// Inserta/actualiza resultados con tipo específico (result_type).
    pub async fn upsert_step_results_typed(&self, step_id: Uuid, results: &HashMap<String, serde_json::Value>, result_type: &str) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(pool) = &self.pool {
            for (k, v) in results {
                sqlx::query(
                    "INSERT INTO workflow_step_results (step_id, result_key, result_value, result_type) VALUES ($1,$2,$3,$4)\n                     ON CONFLICT (step_id, result_key) DO UPDATE SET result_value = EXCLUDED.result_value, result_type = EXCLUDED.result_type"
                )
                .bind(step_id)
                .bind(k)
                .bind(serde_json::to_value(v)?)
                .bind(result_type)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    pub async fn get_execution(&self, execution_id: Uuid) -> Result<Vec<StepExecutionInfo>, Box<dyn std::error::Error>> {
        // Preferimos in-memory (más rápido). Si se necesita consolidar con BD, se puede
        // extender.
        let guard = self.in_memory.read().await;
        Ok(guard.get(&execution_id).cloned().unwrap_or_default())
    }

    pub async fn get_step_execution(&self, execution_id: Uuid, step_index: usize) -> Result<StepExecutionInfo, Box<dyn std::error::Error>> {
        let all = self.get_execution(execution_id).await?;
        all.get(step_index).cloned().ok_or("Step not found".into())
    }

    pub async fn save_step_execution_for_branch(&self, execution: &StepExecutionInfo, branch_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        // Se clona la ejecución y se altera el step_id para representar la rama.
        let mut cloned = execution.clone();
        cloned.step_id = branch_id; // treat branch as separate id for now
        self.save_step_execution(&cloned).await
    }

    pub async fn get_step(&self, _step_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        Err("Not implemented".into())
    }

    pub async fn save_step_for_branch(&self, _step: &(), _branch_id: Uuid) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// Recolecta todos los steps que comparten el mismo root_execution_id. Esto
    /// permite reconstruir el linaje completo (incluyendo steps de ramas)
    /// ordenado cronológicamente.
    pub async fn get_steps_by_root(&self, root_id: Uuid) -> Vec<StepExecutionInfo> {
        let guard = self.in_memory.read().await;
        let mut collected = Vec::new();
        for vec_exec in guard.values() {
            for exec in vec_exec {
                if exec.root_execution_id == root_id {
                    collected.push(exec.clone());
                }
            }
        }
        collected.sort_by_key(|e| e.start_time);
        collected
    }

    /// Verifica integridad recomputando hash de parámetros y comparando (solo in-memory).
    pub async fn verify_execution_integrity(&self, step_id: Uuid) -> Option<bool> {
        if let Ok(entries) = self.get_execution(step_id).await { if let Some(last) = entries.last() {
            if let Some(h) = &last.parameter_hash { return Some(h == &compute_sorted_hash(&last.parameters)); }
        }}
        None
    }

    /// Construye estructura en árbol (mapa parent->children) desde root_id.
    pub async fn build_branch_tree(&self, root_id: Uuid) -> serde_json::Value {
        let steps = self.get_steps_by_root(root_id).await;
        let mut children: HashMap<Uuid, Vec<&StepExecutionInfo>> = HashMap::new();
        for s in &steps { if let Some(parent) = s.parent_step_id { children.entry(parent).or_default().push(s); } }
        fn build(node: &StepExecutionInfo, map: &HashMap<Uuid, Vec<&StepExecutionInfo>>) -> serde_json::Value {
            let kids = map.get(&node.step_id).cloned().unwrap_or_default();
            serde_json::json!({
                "step_id": node.step_id,
                "name": node.step_name,
                "children": kids.into_iter().map(|c| build(c, map)).collect::<Vec<_>>()
            })
        }
        // Raíces: aquellos sin parent o que son branch origins.
        let roots: Vec<&StepExecutionInfo> = steps.iter().filter(|s| s.parent_step_id.is_none()).collect();
        serde_json::json!(roots.into_iter().map(|r| build(r, &children)).collect::<Vec<_>>())
    }

    /// Lista valores de una propiedad (flatten) con su provider principal (primer provider registrado) filtrando opcionalmente por provider_name.
    pub async fn list_property_values(&self, property: &str, provider_filter: Option<&str>) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        if let Some(pool) = &self.pool {
            let rows = if let Some(pf) = provider_filter {
                sqlx::query(
                    "SELECT mfp.family_id, mfp.property_name, mfp.value, mfp.source, mfp.timestamp, prov.provider_name FROM molecule_family_properties mfp LEFT JOIN LATERAL (SELECT provider_name FROM molecule_family_property_providers p WHERE p.family_id = mfp.family_id AND p.property_name = mfp.property_name LIMIT 1) prov ON TRUE WHERE mfp.property_name = $1 AND prov.provider_name = $2"
                ).bind(property).bind(pf).fetch_all(pool).await.unwrap_or_default()
            } else {
                sqlx::query(
                    "SELECT mfp.family_id, mfp.property_name, mfp.value, mfp.source, mfp.timestamp, prov.provider_name FROM molecule_family_properties mfp LEFT JOIN LATERAL (SELECT provider_name FROM molecule_family_property_providers p WHERE p.family_id = mfp.family_id AND p.property_name = mfp.property_name LIMIT 1) prov ON TRUE WHERE mfp.property_name = $1"
                ).bind(property).fetch_all(pool).await.unwrap_or_default()
            };
            for r in rows {
                let val = serde_json::json!({
                    "family_id": r.try_get::<Uuid,_>("family_id").ok(),
                    "property": r.try_get::<String,_>("property_name").ok(),
                    "value": r.try_get::<f64,_>("value").ok(),
                    "source": r.try_get::<Option<String>,_>("source").ok().flatten(),
                    "timestamp": r.try_get::<chrono::DateTime<chrono::Utc>,_>("timestamp").ok(),
                    "provider": r.try_get::<Option<String>,_>("provider_name").ok().flatten(),
                });
                out.push(val);
            }
        }
        out
    }

    /// Exporta un reporte consolidado de un root_execution_id con pasos y familias.
    pub async fn export_workflow_report(&self, root_id: Uuid) -> serde_json::Value {
        let steps = self.get_steps_by_root(root_id).await;
        let tree = self.build_branch_tree(root_id).await;
        serde_json::json!({
            "root_execution_id": root_id,
            "steps": steps,
            "branch_tree": tree,
            "generated_at": chrono::Utc::now()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::step::StepStatus;
    use chrono::Utc;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_repository_methods() {
        let repo = WorkflowExecutionRepository::new(true);

    let execution_info = StepExecutionInfo { step_id: Uuid::new_v4(),
                         step_name: "test".into(),
                         step_description: "repo test".into(),
                                                 parameters: HashMap::new(),
                                                 parameter_hash: Some(compute_sorted_hash(&serde_json::json!({}))),
                                                 providers_used: Vec::new(),
                                                 start_time: Utc::now(),
                                                 end_time: Utc::now(),
                                                 status: StepStatus::Completed,
                                                 root_execution_id: Uuid::new_v4(),
                                                 parent_step_id: None,
                                                 branch_from_step_id: None,
                         input_family_ids: Vec::new(),
                         input_snapshot: None,
                         step_config: None,
                         integrity_ok: None };

        // Test save_step_execution
        repo.save_step_execution(&execution_info).await.unwrap();

        // Test get_execution
        let executions = repo.get_execution(execution_info.step_id).await.unwrap();
        assert_eq!(executions.len(), 1);

        // Test get_step_execution
        let step = repo.get_step_execution(execution_info.step_id, 0).await.unwrap();
        assert_eq!(step.step_id, execution_info.step_id);

        // Test save_step_execution_for_branch
        let branch_id = Uuid::new_v4();
        repo.save_step_execution_for_branch(&execution_info, branch_id).await.unwrap();

        let branch_executions = repo.get_execution(branch_id).await.unwrap();
        assert_eq!(branch_executions.len(), 1);

        // Test get_step (will error but calls the method)
        let _ = repo.get_step(Uuid::new_v4()).await;

        // Test save_step_for_branch
        repo.save_step_for_branch(&(), Uuid::new_v4()).await.unwrap();

        // Call get_family (will be None in in-memory mode without persisted DB pool)
        let _none = repo.get_family(Uuid::new_v4()).await.unwrap();
        // Exercise freeze_family (no-op without DB, ensures method is used)
        repo.freeze_family(Uuid::new_v4()).await.unwrap();
        // Test get_steps_by_root (should find entries for existing root ids)
        let list = repo.get_steps_by_root(execution_info.root_execution_id).await;
        assert!(!list.is_empty());
    }
}

#[cfg(test)]
mod repository_usage_tests {
    use super::*;
    use crate::workflow::step::StepStatus;
    use chrono::Utc;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_repo_all_methods() {
        let repo = WorkflowExecutionRepository::new(true);
    let info = StepExecutionInfo { step_id: Uuid::new_v4(),
                       step_name: "test".into(),
                       step_description: "repo usage".into(),
                                       parameters: HashMap::new(),
                                       parameter_hash: Some(compute_sorted_hash(&serde_json::json!({}))),
                                       providers_used: Vec::new(),
                                       start_time: Utc::now(),
                                       end_time: Utc::now(),
                                       status: StepStatus::Pending,
                                       root_execution_id: Uuid::new_v4(),
                                       parent_step_id: None,
                                       branch_from_step_id: None,
                                       input_family_ids: Vec::new(),
                                       input_snapshot: None,
                                       step_config: None,
                                       integrity_ok: None };
        repo.save_step_execution(&info).await.unwrap();
        let all = repo.get_execution(info.step_id).await.unwrap();
        assert_eq!(all.len(), 1);
        let one = repo.get_step_execution(info.step_id, 0).await.unwrap();
        assert_eq!(one.step_id, info.step_id);
        let branch = Uuid::new_v4();
        repo.save_step_execution_for_branch(&info, branch).await.unwrap();
        let branched = repo.get_execution(branch).await.unwrap();
        assert_eq!(branched.len(), 1);
        let _ = repo.get_step(Uuid::new_v4()).await;
        repo.save_step_for_branch(&(), Uuid::new_v4()).await.unwrap();
        let _none = repo.get_family(Uuid::new_v4()).await.unwrap();
        // Exercise freeze_family again to avoid dead code warning
        repo.freeze_family(Uuid::new_v4()).await.unwrap();
        let _by_root = repo.get_steps_by_root(info.root_execution_id).await;
    }
}

async fn _use_repository_methods() {
    use crate::workflow::step::{StepExecutionInfo, StepStatus};
    use chrono::Utc;
    use std::collections::HashMap;
    use uuid::Uuid;

    let repo = WorkflowExecutionRepository::new(true);
    let id = Uuid::new_v4();
    let info = StepExecutionInfo { step_id: id,
                                   step_name: "example".into(),
                                   step_description: "usage".into(),
                                   parameters: HashMap::new(),
                                   parameter_hash: Some(compute_sorted_hash(&serde_json::json!({}))),
                                   providers_used: Vec::new(),
                                   start_time: Utc::now(),
                                   end_time: Utc::now(),
                                   status: StepStatus::Completed,
                                   root_execution_id: Uuid::new_v4(),
                                   parent_step_id: None,
                                   branch_from_step_id: None,
                                   input_family_ids: Vec::new(),
                                   input_snapshot: None,
                                   step_config: None,
                                   integrity_ok: None };
    let _ = repo.save_step_execution(&info).await;
    let _ = repo.get_execution(id).await;
    let _ = repo.get_step_execution(id, 0).await;
    let _ = repo.save_step_execution_for_branch(&info, id).await;
    let _ = repo.get_step(id).await;
    let _ = repo.save_step_for_branch(&(), id).await;
}
