# L0 — `SmallSignalGain`

Entspricht Teil C Phase 4 aus [`../plan_verstaerkung.md`](../plan_verstaerkung.md).
**Voraussetzung:** [`01_L-1_ideal_amplifier.md`](01_L-1_ideal_amplifier.md).

## Ziel / neue Fähigkeit

Erste echte Spektroskopie-Physik, ungesättigt. Der entscheidende Unterschied zu `Const`: **der
Laufweg durch das Medium zählt**. Noch ray-weise auswertbar — jeder Ray wird unabhängig von allen
anderen skaliert, deshalb wird hier **noch kein** Fluenz-Schätzer gebraucht (der kommt erst mit der
Sättigung in L1).

## Core

- **Hartcodierte `GainMaterialData`** statt Materialbibliothek (Nutzervorgabe, siehe
  [`README.md`](README.md)): ein kleines Rust-Struct/Enum mit wenigen festen Presets (z. B.
  `GainMaterialPreset::YbYag | NdGlass`), jeweils feste σ_e, σ_a als geschlossene Formel (z. B.
  Gauß-Profil um eine Zentralwellenlänge) statt Tabelle + Interpolation. **Wichtig:** Zugriff nur
  über eine schmale Trait-Schnittstelle (`EmissionCrossSection`/`AbsorptionCrossSection` o. ä.),
  damit die parallel entstehende Materialbibliothek diese Schnittstelle später einfach
  implementiert, ohne dass `SmallSignalGain` (oder L1/L2) angefasst werden muss. Es gibt aktuell
  keine vergleichbare Struktur im Code (`opossum_core` hat kein `Material`-Konzept, nur
  `RefractiveIndexType` für n(λ) — das deckt keine Emissions-/Absorptionsquerschnitte ab).
- **Segmentierung des Innenpfads (`n_steps`)** — erster echter, aber einfacher z-Marsch-Baustein.
  Er entsteht **im gemeinsamen Volumen-Helper aus V2**, nicht in einer eigenen Node: der Helper
  bündelt bereits die Sequenz Eintrittsfläche → Volumen → Austrittsfläche für `Lens`, `Wedge`,
  `CylindricLens` und den Geometrie-Node, und wird hier um `n_steps` Zwischenschritte erweitert.
  Alle Volumen-Nodes bekommen die Fähigkeit damit gleichzeitig, ohne einzeln angefasst zu werden.
  Da eine Volumenpropagation im Code bisher komplett fehlt (bestätigt: kein "z-march"/"substep"
  irgendwo), ist das die Stelle, an der dieser Mechanismus überhaupt erst entsteht — L1 erweitert
  ihn nur um Fluenz-Kopplung.
- `G = exp(∫ σ_e(λ)·ΔN ds)`, optional Reabsorption `−σ_a(λ)·N_1`. Reine Multiplikation pro Ray,
  kein Zustands-Update (eingefrorene Inversion — ein fester Wert oder ein einfaches Profil, noch
  kein eigenständiges `InversionField`-Objekt, das kommt erst mit L1).
- Warndiagnostik bei überzogener Extraktion: nutzt den Diagnostik-Slot aus `00_fundament.md`.

## Backend

Keine Änderung. Das Material-Preset ist wieder ein Enum-Proptype (gleiches Muster wie
`RefractiveIndexType`), läuft komplett generisch über das Properties-Wire-Protokoll.

## GUI

- Neue Enum-Auswahl für das Material-Preset — derselbe Dropdown-Mechanismus wie beim
  `GainModel`-Enum selbst. Pro Preset ein kleiner, meist **read-only** Sub-Editor (feste Werte
  anzeigen, nicht live editierbar) — dadurch wird das fehlende Tabellen-/Kurven-Editor-Präzedenz-
  Problem umgangen, ohne es zu lösen (bleibt für die spätere Materialbibliothek offen).
- `n_steps` ist ein einfacher `I32`/`F64`-Wert, kein neuer Editor nötig.

## Tests / Abnahme

- Homogene Inversion, gerader Durchgang: `G` gegen analytisch `exp(g₀L)`.
- Negative Inversion → Beer-Lambert-Absorption, gegen Analytik.
- Konvergenz: Ergebnis stabil bei Verdopplung von `n_steps`.
- Chromatik: Verstärkung folgt der Form von `σ_e(λ)`.
- Schräger Durchgang: Verstärkung skaliert mit der tatsächlichen Weglänge.
- Warnung feuert bei überzogener Extraktion.
- Stil-Vorbild für alle Physik-Tests dieser und folgender Stufen:
  `nodes/reflective_grating.rs`, Testmodul ab Zeile 269 — Referenzwert wird **inline aus der
  Formel** berechnet (nicht hartcodiert), Vergleich über `approx::assert_relative_eq!` mit enger
  Toleranz.
- **Abnahme:** Kleinsignalverstärkung eines realen Verstärkerkopfs stimmt mit einer unabhängigen
  Handrechnung überein.

## Modularität

Der z-Marsch im Volumen-Helper und die `GainMaterialData`-Schnittstelle sind die zwei Teile dieser
Stufe, die L1 unverändert übernimmt und nur erweitert (Fluenz-Kopplung bzw. zusätzliche Felder
τ_f/N_dop). Es entsteht kein zweites, paralleles Marsch- oder Material-System.
