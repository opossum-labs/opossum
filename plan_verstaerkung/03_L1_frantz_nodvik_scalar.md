# L1 — `FrantzNodvik`

> Früherer Arbeitstitel dieser Stufe: `FrantzNodvikScalar`. Maßgeblich ist die Nomenklatur aus
> [`README.md`](README.md).

Entspricht Teil C Phase 3, 5 und 6 aus [`../plan_verstaerkung.md`](../plan_verstaerkung.md) — hier
bewusst zusammengefasst, weil Inversionsfeld, Fluenz-Schätzer und Sättigungsmodell nur gemeinsam
einen sinnvollen Zwischenstand ergeben (kein sinnvoller Regressionspunkt dazwischen).
**Voraussetzung:** [`02_L0_small_signal_gain.md`](02_L0_small_signal_gain.md).

## Ziel / neue Fähigkeit

Sättigung, Extraktionseffizienz, Multipass. Die physikalisch aufwändigste Stufe — hier entstehen
die drei Bausteine, die L2 und der `PumpSolver` danach nur noch wiederverwenden.

## Core

### `InversionField`

Eigenes Gitter, unabhängig vom Ray-Sampling. Es gibt dafür noch keine Entsprechung im Code (kein
`Material`-Struct, kein Feld-Konzept außerhalb von Detektor-Auswertungen). Zwei Provider:

| Provider | Parameter |
|---|---|
| `Uniform` | ein Wert, wahlweise `beta`, `N2` oder `stored_energy_density` — alle drei ineinander umrechenbar |
| `AnalyticProfile` | einfache Formel (Super-Gauß transversal, Exponential in z), keine Tabelle |

Lebenszyklus nach Teil A.7 (Init/Evolve/Reset), gebunden an den Analyzer-Lauf — Ansatzpunkt ist
`OpticNode::reset_data()` (`core_optics/optic_node.rs:55`), das bereits bei jedem
`RayTracingAnalyzer::analyze()`-Lauf aufgerufen wird (`analyzers/raytrace.rs:183`). Materialwerte
(σ_e, σ_a, τ_f, N_dop) kommen aus der in L0 gebauten hartcodierten `GainMaterialData` — hier wird
nur die Provider-Logik ergänzt, keine neue Materialschicht.

### `FieldEstimator`

Das ist der wichtigste Fund der Architektur-Recherche: **die Voronoi-Fluenzschätzung existiert
bereits ebenenunabhängig**, wird aber aktuell nirgends produktiv verwendet.

- `Rays::calc_fluence_at_position(iso: &Isometry)` (`light/rays.rs:831`) projiziert Ray-Positionen
  durch eine beliebige `Isometry`, baut Voronoi-Zellen (`utils/griddata.rs`) und liefert Fluenz pro
  Zelle — exakt die Schnittstelle "Bundle + Ebene → Fluenz pro Zelle" aus Teil A.6/A.4 des
  Root-Plans. Aktuell nur in Unit-Tests aufgerufen (`rays.rs:2596`), von keinem Analyzer verwendet.
- **Aufgabe hier:** diesen Pfad aus dem Testcode in einen echten Produktionspfad heben, aufrufbar
  bei jedem z-Substep. Die Schätzer-Auswahl existiert als Enum bereits
  (`FluenceEstimator::{Voronoi, KDE, Binning, HelperRays}`,
  `core_optics/hit_map/fluence_estimator.rs:11`) — keine neue Enum-Definition nötig, nur ein neuer
  Aufrufpfad.
- **Empfehlung für den z-Marsch:** `FluenceEstimator::HelperRays` (`FluenceRays`,
  `light/rays.rs:1519`) statt Voronoi als Default für die Schleife — die per-Ray-Dreiecks-Fluenz
  braucht keine Neu-Tessellierung des gesamten Querschnitts pro Schritt, was bei vielen Substeps
  relevant für Performance ist. Voronoi bleibt als Alternative wählbar (Glättungsparameter explizit,
  siehe Root-Plan Phase 5).

### z-Marsch

Äußere Schleife über Substeps **im gemeinsamen Volumen-Helper** (V2 der Vorbereitungsstufe), der
seit L0 bereits segmentiert — hier wird er nicht neu gebaut, sondern um die Fluenz-Kopplung
erweitert. Innere Schleife über Rays. Weil der Helper von allen Volumen-Nodes genutzt wird, gilt
die Sättigung damit automatisch für Linse, Wedge, Zylinderlinse und den Geometrie-Node.
Ebenendefinition bei verkipptem/Brewster-geschnittenem Medium: die Ebene senkrecht zur mittleren
Ausbreitungsrichtung.

### `ExtractionReducer`

Sammelt pro Zelle die Beiträge aller gleichzeitig treffenden Rays, wendet den Frantz-Nodvik-Faktor
**einmal pro Zelle** an (nicht pro Ray — sonst Doppelzählung, siehe Root-Plan A.4), skaliert dann
alle Rays der Zelle über den in `00_fundament.md` gebauten Ray-Energie-Mutator, schreibt die
Extraktion danach ins `InversionField`.

```
F_out = F_sat · ln{ 1 + [exp(F_in/F_sat) − 1] · G₀ },  G₀ = exp(σ_e·ΔN·ds)
```

### Multipass

**Keine neue Infrastruktur nötig** — `NodeReference` (`nodes/reference.rs`) existiert bereits
vollständig und funktioniert über `Weak<Mutex<dyn Analyzable>>` auf dieselbe Node-Instanz. Da das
`InversionField` Teil des Zustands der jeweiligen Volumen-Node ist und damit hinter demselben
`Arc<Mutex<..>>` liegt, wird es automatisch korrekt zwischen Durchgängen geteilt. Zu beachten: es
gibt keinen Graph-Ebenen-Cache, der das kaputt machen könnte (bestätigt — `reset_data()` ist die
einzige Rücksetz-Stelle, und die betrifft nur Surface-lokale Hit-Maps, nicht node-eigene Felder wie
das `InversionField`).

### Globale Energiebilanz

Extrahierte Photonenenergie = Abnahme der gespeicherten Energie (bis auf τ_f-Zerfall) — als
**Dauer-Assertion im Code**, nicht nur als Test, da sie laut Root-Plan praktisch jeden Fehler in
der Estimator-Reducer-Kette abfängt.

## Backend

Keine Änderung. `estimator` und `inversion_provider` sind Enum-Proptypes (gleiches Muster wie
`RefractiveIndexType`/`FilterTypeBuilder`), laufen generisch übers Wire-Protokoll.

## GUI

- `estimator`-Dropdown (Voronoi/KDE/Binning/HelperRays) — Auswahl-Mechanismus existiert 1:1 im
  Code (`FluenceEstimator` ist bereits `IntoEnumIterator`-fähig, siehe die Verwendung in
  `select_options_from_enum_iterator`), nur ein neuer Menüpunkt im Editor, kein neues Muster.
- `inversion_provider`-Dropdown (Uniform/AnalyticProfile) mit je einem kleinen Sub-Editor
  (wenige Zahlenfelder) — gleiches Dropdown+Sub-Editor-Muster wie überall sonst.
- Kein neuer Bedarf an Tabellen-/2D-Feld-Editoren: das `InversionField` selbst muss in dieser
  Stufe nicht interaktiv editierbar sein, nur seine Erzeugungs-Parameter (die sind Skalare).

## Tests / Abnahme — kritischste Phase, entsprechend gründlich

- Homogene Inversion, Top-Hat-Strahl: gegen die analytische FN-Lösung.
- Grenzfall `F_in ≪ F_sat` → `F_in·G₀` (Anschluss an L0); Grenzfall `F_in ≫ F_sat` →
  `F_in + F_sat·ln G₀`.
- Globale Energiebilanz als Dauer-Assertion (s. o.).
- Keine Übersättigung: ΔN nie negativ (bzw. nie unter β_min).
- Reihenfolgeunabhängigkeit: Permutation der Rays ändert nichts.
- Multipass: vier Durchgänge über `NodeReference`, Gewinn pro Pass fällt monoton.
- Reset: zweiter Lauf desselben Systems liefert identische Ergebnisse (fängt vergessenes
  Zurücksetzen des Zustands).
- Konvergenz in `n_steps` und in der Ray-Zahl.
- Gaußstrahl: transversales Ausbrennen sichtbar, Fluenzprofil flacht ab.
- **Abnahme:** Extraktionseffizienz und Ausgangsenergie eines realen Multipass-Verstärkers
  reproduzieren eine unabhängige 1D-Referenzrechnung innerhalb weniger Prozent.

## Modularität

`InversionField`, `FieldEstimator` und `ExtractionReducer` sind die zentralen neuen Bausteine
dieser Stufe. [`04_L2_frantz_nodvik_spectral.md`](04_L2_frantz_nodvik_spectral.md) verwendet alle
drei unverändert, nur mehrfach (einmal pro Zeit-Bin);
[`05_pump_solver.md`](05_pump_solver.md) verwendet dieselbe Infrastruktur mit umgedrehtem
Vorzeichen. Es entsteht an keiner Stelle ein zweites, paralleles System.
