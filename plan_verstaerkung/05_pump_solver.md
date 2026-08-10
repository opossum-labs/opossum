# `PumpSolver` — orthogonale Ergänzung zu L1/L2

Entspricht Teil C Phase 8 aus [`../plan_verstaerkung.md`](../plan_verstaerkung.md). Kein Punkt der
Eskalationstabelle (Teil A.8) selbst, sondern eine Erweiterung, die parallel zu L2 oder danach
angegangen werden kann. **Voraussetzung:** [`03_L1_frantz_nodvik_scalar.md`](03_L1_frantz_nodvik_scalar.md)
(nicht L2 — beide hängen nur von L1 ab, nicht voneinander).

> **Blockiert durch V7.** Die vom Nutzer gewünschte Bedienung — Modellvariante `EndPumping`, die
> **je Facette einen Pump-Port erzeugt** — setzt dynamische Ports voraus, und die sind ein eigenes
> Teilprojekt (siehe [`00_vorbereitung.md`](00_vorbereitung.md), V7). Ohne V7 lässt sich der
> PumpSolver zwar physikalisch bauen, aber nicht so bedienen, wie vorgesehen. Reihenfolge deshalb:
> **V7 vor diesem Dokument**, oder bewusst mit fest verdrahteten Ports starten und die
> Bedienoberfläche später nachziehen.

## Ziel

Inversion aus einem Pumplaser berechnen statt vorzugeben. Kernidee des Root-Plans: **dieselbe
Maschinerie mit umgedrehtem Vorzeichen** — kein Sonderfall, kein neues Ökosystem. Ein Pump-Bundle
läuft durch dasselbe Volumen; ein `AbsorptionModel` (Spiegelbild von `GainModel`) deponiert Energie
statt sie zu entziehen.

Dieses Dokument existiert bewusst getrennt von L2, weil es beweist, dass die in L1 gebaute
Infrastruktur tatsächlich wiederverwendbar ist, statt nur für Sättigung maßgeschneidert zu sein —
das war die explizite Modularitäts-Anforderung an diesen Plan.

## Core

- **Wiederverwendung ohne Änderung:** `InversionField`, der z-Marsch im gemeinsamen Volumen-Helper
  und die Volumen-Nodes selbst — keine neue Node, keine neue Registrierung.
- **Neu:** `AbsorptionModel` als Spiegelbild von `GainModel` (eigene Enum-Familie oder
  vorzeichen-gespiegelte Variante desselben Konzepts — Designentscheidung bei der Umsetzung, kein
  Blocker für die Planung), das statt einer Extraktion eine Deposition ins `InversionField`
  schreibt.
- **Geometrie / Pumpseiten:** Ein- oder Zweiseitenpumpen wird über die Pump-Ports abgebildet, die
  `EndPumping` je Facette anlegt — genau der Teil, der an V7 hängt. Solange V7 aussteht, bleibt nur
  die Behelfslösung, das Pump-Bundle über einen der vorhandenen Ports einzuspeisen.
- **Wärmedeposition** als Nebenprodukt (relevant für eine spätere thermische Linse) — hier nur als
  zusätzlicher Ausgabewert des `AbsorptionModel` mitgeführt; die optische Rückwirkung ist
  ausdrücklich **nicht** Teil dieses Dokuments (siehe Root-Plan Teil D).
- Pump-Parameter (`pump_wavelength`, `pump_energy`/`pump_power`, `pump_duration`,
  `quantum_efficiency`, `absorption_saturation`) sind einfache Skalare, `sigma_a(λ_p)` kommt aus
  derselben hartcodierten `GainMaterialData`-Schnittstelle wie in L0 — keine zweite Materialquelle.

## Backend

Keine Änderung, **sofern V7 steht** — dann ist `EndPumping` nur eine weitere Enum-Variante. Ohne
V7 fehlt dem Wire-Protokoll der Port-Änderungs-Fall (siehe V7).

## GUI

Keine neue Kategorie — Zahlenfelder plus ein Enum-Dropdown, identisch zum etablierten Muster. Die
zusätzlichen Pump-Ports an der Node erscheinen automatisch, sobald das Port-Set sie enthält
(`NodeInfo` transportiert Ports bereits).

## Tests / Abnahme

- Beer-Lambert-Absorptionsprofil gegen Analytik bei schwacher Pumpe.
- Energiebilanz: absorbierte Pumpenergie = gespeicherte Energie + Quantendefekt-Wärme.
- Zweiseitenpumpen ergibt symmetrisches Profil.
- τ_f-Zerfall bei langer Pumpdauer korrekt.
- Resultierendes Profil in L1 eingespeist liefert plausible Extraktion — das ist der eigentliche
  Modularitäts-Nachweis: ein von `PumpSolver` gefülltes `InversionField` muss ohne Anpassung als
  Eingabe für den L1-Mechanismus funktionieren.
- **Abnahme:** Für einen realen Verstärkerkopf stimmen Absorptionsgrad und gespeicherte Energie
  mit der Auslegungsrechnung überein.

## Modularität

Wenn dieses Dokument am Ende mehr Core-Arbeit braucht als "ein neues Modell-Enum + eine
Deposition-statt-Extraktion-Variante des `ExtractionReducer`", ist das ein Signal, dass L1 die
Verstärkungs-Infrastruktur zu eng an "Extraktion" statt an "Fluenz-Kopplung mit Energiefluss in
beide Richtungen" gebaut hat — in dem Fall lohnt sich ein kurzer Rücksprung zu L1, bevor hier
weitergebaut wird.
