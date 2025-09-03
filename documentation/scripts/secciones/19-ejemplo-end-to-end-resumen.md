# Sección 19 - Ejemplo End‑To‑End (Resumen)

```mermaid
flowchart LR
    subgraph Ingesta
        A[Acquire Molecules]
        B[Build Families]
    end
    subgraph Propiedades
        C[Compute Properties]
        D[Select Preferred (Policy)]
    end
    subgraph Agregación
        E[Aggregate Metrics]
    end
    subgraph Decisiones
        F{Branch Criteria Met?}
        G{Human Gate?}
    end
    subgraph Salida
        H[Generate Report]
        I[Persist Artifacts / Publish]
    end

    A --> B --> C --> D --> E --> F
    F -- Yes --> BR[Create Branch] --> C
    F -- No --> G
    G -- Yes --> UI[Await User Input] --> G
    G -- No --> H --> I
```

Descripción breve: El diagrama muestra el flujo lineal básico con puntos de decisión para branching determinista y gate humano. Cada transición emite eventos (StepStarted, ArtifactCreated, StepCompleted) y los artifacts quedan indexados por hash para reproducibilidad.

