# L3–L5 — Ausblick (bewusst kein Umsetzungsplan)

Diese Stufen sind in [`../plan_verstaerkung.md`](../plan_verstaerkung.md) Teil D explizit als
**"bewusst zurückgestellt"** markiert. Auf Nutzerwunsch bleibt dieses Dokument ein Kurz-Stub statt
einer vollen Core/Backend/GUI-Aufschlüsselung — die physikalische Grundlage ist im Root-Plan noch
nicht ausgearbeitet genug, um verantwortbar zu planen, ohne zu spekulieren.

## L3 — `RateEquationMarching`

Nötig, wenn Pulsdauer und τ_f vergleichbar werden oder während der Verstärkung gepumpt wird
(Regen, Q-Switch). Baut auf der Capability `temporal_envelope` auf (noch nicht Teil des
Fundaments — kommt erst, wenn diese Stufe konkret angegangen wird). Vermutlich ein echtes
Zeitschrittverfahren mit Event-Queue statt des sequentiellen z-Marschs aus L1/L2, weil hier zwei
Durchgänge *gleichzeitig* im Medium sein können.

## L4 — ASE

Braucht Ray-Quellen, die aus Medienzellen heraus emittieren. Der `PumpSolver`
([`05_pump_solver.md`](05_pump_solver.md)) legt die Struktur dafür bereits an, da er bereits mit
Deposition/Emission aus dem `InversionField` heraus arbeitet.

## L5 — Feldkopplung

Kommt zusammen mit einer künftigen Fourier-Propagation (`LightData::Fourier` ist im Code aktuell
ein reiner Platzhalter ohne Implementierung, `light/lightdata/mod.rs:20`). Weil die
`GainModel`-Schnittstelle auf Fluenz/Intensität definiert ist und nicht auf Ray-Energie, ist
dasselbe Modellobjekt punktweise auf ein Feldgitter anwendbar (Split-Step: dz propagieren → Gain
anwenden → dz propagieren) — sofern die Schnittstelle aus `00_fundament.md` diese Eigenschaft
beim Bau von L0–L2 nicht versehentlich verliert.

## Wann dieses Dokument aufzubrechen ist

Sobald L2 und der `PumpSolver` abgeschlossen sind und eine dieser drei Stufen konkret angegangen
werden soll: dann ein eigenes Dokument nach demselben Schema wie `01`–`05` anlegen, basierend auf
einer frischen Architektur-Recherche zum dann aktuellen Stand von `opossum_core` — nicht auf
dieser Vorausschau, die zu diesem Zeitpunkt möglicherweise veraltet ist.
