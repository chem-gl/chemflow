## 24. Orden de Creación de Componentes (Secuencia Recomendada)

### 24.1 Mapa Dependencias

```
Fundaciones → Dominio → Motor Lineal → Persistencia (mem → Postgres) → Adaptadores/Steps
→ Políticas Básicas → Retry → Errores Persistidos → Branching → Inyección Avanzada / Human Gate
→ Políticas Avanzadas → Agregados Normalizados → Observabilidad → Hardening / Caching
```

### 24.2 Convenciones

Definiciones (Núcleo, Contrato Estabilizado, GATE_Fx, etc.)

Fases F0–F14 (cada una con: núcleo, contrato estabilizado, gate, paralelo seguro, objetivos, pasos sugeridos, criterios de cierre) reproducidas completas (F0 Fundaciones hasta F14 Hardening y Caching).
