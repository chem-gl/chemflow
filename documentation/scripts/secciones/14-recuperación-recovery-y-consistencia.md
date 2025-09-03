# Sección 14 - Recuperación (Recovery) y Consistencia

Algoritmo resumido: cargar ejecuciones → marcar Running huérfanas → reconstruir cache de artifacts por hash → validar integridad (hashes) → localizar primer Pending → continuar. Eventos RecoveryStarted / RecoveryCompleted encapsulan resultado.

