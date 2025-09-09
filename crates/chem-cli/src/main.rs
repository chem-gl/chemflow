use chem_core::FlowEngine;
use uuid::Uuid;

fn main() {
    // Cargar .env si existe para obtener DATABASE_URL
    let _ = dotenvy::dotenv();
    // CLI mínima: `chem retry --flow <UUID> --step <ID> [--reason <TXT>] [--max <N>]`
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && args[1] == "retry" {
    let mut flow: Option<Uuid> = None;
        let mut step: Option<String> = None;
        let mut reason: Option<String> = None;
        let mut max: Option<u32> = None;
        let mut i = 2;
        while i < args.len() {
            match args[i].as_str() {
                "--flow" => {
                    i += 1;
                    if i < args.len() { flow = Uuid::parse_str(&args[i]).ok(); }
                }
                "--step" => {
                    i += 1;
                    if i < args.len() { step = Some(args[i].clone()); }
                }
                "--reason" => {
                    i += 1;
                    if i < args.len() { reason = Some(args[i].clone()); }
                }
                "--max" => {
                    i += 1;
                    if i < args.len() { max = args[i].parse::<u32>().ok(); }
                }
                _ => {}
            }
            i += 1;
        }
        
        if let (Some(flow_id), Some(step_id)) = (flow, step) {
            // Si hay DATABASE_URL, usar Postgres; de lo contrario, no se puede operar sobre flows existentes
            if std::env::var("DATABASE_URL").is_ok() {
                let pool = match chem_persistence::build_dev_pool_from_env() {
                    Ok(p) => p,
                    Err(e) => { eprintln!("[chem retry] pool error: {e}"); std::process::exit(5); }
                };
                let provider = chem_persistence::PoolProvider { pool };
                let event_store = chem_persistence::PgEventStore::new(provider);
                let repo = chem_persistence::PgFlowRepository::new();
                let mut engine: FlowEngine<_, _> = FlowEngine::new_with_stores(event_store, repo);
                let events = engine.events_for(flow_id);
                if events.is_empty() { eprintln!("[chem retry] flow no encontrado: {}", flow_id); std::process::exit(4); }
                // Reconstruir ids de steps a partir de eventos para un def mínimo
                let mut ids: Vec<String> = events.iter().filter_map(|e| match &e.kind {
                    chem_core::FlowEventKind::StepStarted { step_id, .. } => Some(step_id.clone()),
                    chem_core::FlowEventKind::StepFinished { step_id, .. } => Some(step_id.clone()),
                    chem_core::FlowEventKind::UserInteractionRequested { step_id, .. } => Some(step_id.clone()),
                    chem_core::FlowEventKind::UserInteractionProvided { step_id, .. } => Some(step_id.clone()),
                    _ => None,
                }).collect();
                ids.dedup();
                if !ids.iter().any(|s| s == &step_id) { eprintln!("[chem retry] step_id no pertenece al flow"); std::process::exit(4); }
                struct DummyStep(String);
                impl chem_core::StepDefinition for DummyStep {
                    fn id(&self) -> &str { &self.0 }
                    fn base_params(&self) -> serde_json::Value { serde_json::Value::Null }
                    fn run(&self, _ctx: &chem_core::model::ExecutionContext) -> chem_core::step::StepRunResult { chem_core::step::StepRunResult::Failure { error: chem_core::errors::CoreEngineError::Internal("dummy".into()) } }
                    fn kind(&self) -> chem_core::step::StepKind { chem_core::step::StepKind::Transform }
                    fn name(&self) -> &str { self.id() }
                }
                let def = chem_core::repo::build_flow_definition(&ids.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                    ids.iter().map(|s| Box::new(DummyStep(s.clone())) as Box<dyn chem_core::StepDefinition>).collect());
                match engine.schedule_retry(flow_id, &def, &step_id, reason.clone(), max) {
                    Ok(true) => { println!("agendado: flow={} step={} reason={:?} max={:?}", flow_id, step_id, reason, max); std::process::exit(0); }
                    Ok(false) => { eprintln!("rechazado: estado/política"); std::process::exit(4); }
                    Err(e) => { eprintln!("error: {e}"); std::process::exit(5); }
                }
            } else {
                eprintln!("[chem retry] requiere DATABASE_URL para operar contra backend persistente");
                std::process::exit(4);
            }
        } else {
            eprintln!("Uso: chem retry --flow <UUID> --step <ID> [--reason <TXT>] [--max <N>]");
            std::process::exit(2);
        }
    } else {
        // Support minimal `approve --flow <UUID> --step <ID> --provided '<JSON>'`
        if args.len() >= 2 && args[1] == "approve" {
            let mut flow: Option<Uuid> = None;
            let mut step: Option<String> = None;
            let mut provided: Option<String> = None;
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--flow" => { i += 1; if i < args.len() { flow = Uuid::parse_str(&args[i]).ok(); } }
                    "--step" => { i += 1; if i < args.len() { step = Some(args[i].clone()); } }
                    "--provided" => { i += 1; if i < args.len() { provided = Some(args[i].clone()); } }
                    _ => {}
                }
                i += 1;
            }
            if let (Some(flow_id), Some(step_id), Some(prov)) = (flow, step, provided) {
                if std::env::var("DATABASE_URL").is_ok() {
                    let pool = match chem_persistence::build_dev_pool_from_env() {
                        Ok(p) => p,
                        Err(e) => { eprintln!("[chem approve] pool error: {e}"); std::process::exit(5); }
                    };
                    let provider = chem_persistence::PoolProvider { pool };
                    let event_store = chem_persistence::PgEventStore::new(provider);
                    let repo = chem_persistence::PgFlowRepository::new();
                    let mut engine: FlowEngine<_, _> = FlowEngine::new_with_stores(event_store, repo);
                    // Reconstruct a minimal definition from events (steps ids)
                    let events = engine.events_for(flow_id);
                    if events.is_empty() { eprintln!("[chem approve] flow no encontrado: {}", flow_id); std::process::exit(4); }
                    let mut ids: Vec<String> = events.iter().filter_map(|e| match &e.kind {
                        chem_core::FlowEventKind::StepStarted { step_id, .. } => Some(step_id.clone()),
                        chem_core::FlowEventKind::StepFinished { step_id, .. } => Some(step_id.clone()),
                        chem_core::FlowEventKind::UserInteractionRequested { step_id, .. } => Some(step_id.clone()),
                        chem_core::FlowEventKind::UserInteractionProvided { step_id, .. } => Some(step_id.clone()),
                        _ => None,
                    }).collect();
                    ids.dedup();
                    if !ids.iter().any(|s| s == &step_id) { eprintln!("[chem approve] step_id no pertenece al flow"); std::process::exit(4); }
                    struct DummyStep(String);
                    impl chem_core::StepDefinition for DummyStep {
                        fn id(&self) -> &str { &self.0 }
                        fn base_params(&self) -> serde_json::Value { serde_json::Value::Null }
                        fn run(&self, _ctx: &chem_core::model::ExecutionContext) -> chem_core::step::StepRunResult { chem_core::step::StepRunResult::Failure { error: chem_core::errors::CoreEngineError::Internal("dummy".into()) } }
                        fn kind(&self) -> chem_core::step::StepKind { chem_core::step::StepKind::Transform }
                        fn name(&self) -> &str { self.id() }
                    }
                    let def = chem_core::repo::build_flow_definition(&ids.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                        ids.iter().map(|s| Box::new(DummyStep(s.clone())) as Box<dyn chem_core::StepDefinition>).collect());
                    // Parse provided JSON
                    let parsed: serde_json::Value = match serde_json::from_str(&prov) {
                        Ok(v) => v,
                        Err(e) => { eprintln!("[chem approve] provided JSON parse error: {e}"); std::process::exit(3); }
                    };
                    match engine.resume_user_input(flow_id, &def, &step_id, parsed) {
                        Ok(true) => { println!("approved: flow={} step={}", flow_id, step_id); std::process::exit(0); }
                        Ok(false) => { eprintln!("rechazado: estado no AwaitingUserInput"); std::process::exit(4); }
                        Err(e) => { eprintln!("error: {e}"); std::process::exit(5); }
                    }
                } else {
                    eprintln!("[chem approve] requiere DATABASE_URL para operar contra backend persistente");
                    std::process::exit(4);
                }
            } else {
                eprintln!("Uso: chem approve --flow <UUID> --step <ID> --provided '<JSON>'");
                std::process::exit(2);
            }
        }
        // Añadimos soporte mínimo para `branch --flow <UUID> --from-step <ID> [--div-hash <HEX>]`
        if args.len() >= 2 && args[1] == "branch" {
            let mut flow: Option<Uuid> = None;
            let mut from_step: Option<String> = None;
            let mut div_hash: Option<String> = None;
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--flow" => { i += 1; if i < args.len() { flow = Uuid::parse_str(&args[i]).ok(); } }
                    "--from-step" => { i += 1; if i < args.len() { from_step = Some(args[i].clone()); } }
                    "--div-hash" => { i += 1; if i < args.len() { div_hash = Some(args[i].clone()); } }
                    _ => {}
                }
                i += 1;
            }
            if let (Some(flow_id), Some(step_id)) = (flow, from_step) {
                if std::env::var("DATABASE_URL").is_ok() {
                    let pool = match chem_persistence::build_dev_pool_from_env() {
                        Ok(p) => p,
                        Err(e) => { eprintln!("[chem branch] pool error: {e}"); std::process::exit(5); }
                    };
                    let provider = chem_persistence::PoolProvider { pool };
                    let event_store = chem_persistence::PgEventStore::new(provider);
                    let repo = chem_persistence::PgFlowRepository::new();
                    let mut engine: FlowEngine<_, _> = FlowEngine::new_with_stores(event_store, repo);
                    // Reconstruir ids de steps a partir de eventos para construir una definición mínima
                    let events = engine.events_for(flow_id);
                    if events.is_empty() { eprintln!("[chem branch] flow no encontrado: {}", flow_id); std::process::exit(4); }
                    let mut ids: Vec<String> = events.iter().filter_map(|e| match &e.kind {
                        chem_core::FlowEventKind::StepStarted { step_id, .. } => Some(step_id.clone()),
                        chem_core::FlowEventKind::StepFinished { step_id, .. } => Some(step_id.clone()),
                        chem_core::FlowEventKind::UserInteractionRequested { step_id, .. } => Some(step_id.clone()),
                        chem_core::FlowEventKind::UserInteractionProvided { step_id, .. } => Some(step_id.clone()),
                        _ => None,
                    }).collect();
                    ids.dedup();
                    if !ids.iter().any(|s| s == &step_id) { eprintln!("[chem branch] step_id no pertenece al flow"); std::process::exit(4); }
                    struct DummyStep(String);
                    impl chem_core::StepDefinition for DummyStep {
                        fn id(&self) -> &str { &self.0 }
                        fn base_params(&self) -> serde_json::Value { serde_json::Value::Null }
                        fn run(&self, _ctx: &chem_core::model::ExecutionContext) -> chem_core::step::StepRunResult { chem_core::step::StepRunResult::Failure { error: chem_core::errors::CoreEngineError::Internal("dummy".into()) } }
                        fn kind(&self) -> chem_core::step::StepKind { chem_core::step::StepKind::Transform }
                        fn name(&self) -> &str { self.id() }
                    }
                    let def = chem_core::repo::build_flow_definition(&ids.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                        ids.iter().map(|s| Box::new(DummyStep(s.clone())) as Box<dyn chem_core::StepDefinition>).collect());
                    match engine.branch(flow_id, &def, &step_id, div_hash) {
                        Ok(bid) => { println!("branch creado: {} (from {}@{})", bid, flow_id, step_id); std::process::exit(0); }
                        Err(e) => { eprintln!("error: {e}"); std::process::exit(5); }
                    }
                } else {
                    eprintln!("[chem branch] requiere DATABASE_URL para operar contra backend persistente");
                    std::process::exit(4);
                }
            } else {
                eprintln!("Uso: chem branch --flow <UUID> --from-step <ID> [--div-hash <HEX>]");
                std::process::exit(2);
            }
        } else {
            println!("chem-cli: use 'retry' or 'branch' subcommands");
        }
    }
}
