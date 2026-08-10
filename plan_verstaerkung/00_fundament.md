# Fundament — physikalische Träger für alle Eskalationsstufen

Zweiter Teil der Stufe 00. Der erste Teil ist [`00_vorbereitung.md`](00_vorbereitung.md) (Struktur
und Bedienoberfläche) und kommt **vor** diesem Dokument. Entspricht Phase 1 aus
[`../plan_verstaerkung.md`](../plan_verstaerkung.md) Teil C.

## Ziel

Alle *physikalischen* Träger einbauen, die jede spätere Stufe braucht. Wie ein Verstärker im
Modell repräsentiert und bedient wird, ist zu diesem Zeitpunkt bereits geklärt — die
`amp config`-Property, der gemeinsame Volumen-Helper und der Geometrie-Node stammen aus der
Vorbereitungsstufe. Nach dieser Phase verhält sich alles weiterhin physikalisch identisch zu
vorher (`amp config` steht überall auf `None`).

## Warum Gruppenlaufzeit und Capability-Deklaration schon hier?

Die Architektur-Recherche zeigt: L−1/L0/L1 brauchen weder Ankunftszeit noch eine formale
Capability-Prüfung — `Rays::total_energy()` (Bundle-Energie) existiert bereits
(`opossum_core/src/light/rays.rs:534`), und `per_ray_wavelength` ist durch `AnalysisRayTrace`
ohnehin immer gegeben. Beides wird erst für L2 (Zeit-Binning) scharf. Das würde nach Teil B,
Regel 4 ("keine spekulativen Abstraktionen") dafür sprechen, es auf L2 zu verschieben.

**Entscheidung (Nutzervorgabe):** trotzdem upfront bauen, als gemeinsames Fundament vor L−1.
Grund: einmal in jede `Ray`-Instanz eingebaute Felder sind später ohne Migrationsschmerz nutzbar;
nachträgliches Einfügen in ein bereits von mehreren Gain-Modellen genutztes `Ray` wäre invasiver
als ein einmaliges Hinzufügen jetzt, wo `Ray` noch nicht von Gain-Code abhängt.

## Core

1. **Ray-Energie-Mutator.** `Ray.e` (`light/ray.rs:44`) ist privat, es gibt aktuell nur
   `filter_energy()` (`ray.rs:646`), das ausschließlich Transmission ≤ 1 zulässt
   (`ideal_filter/node.rs:95`, verbietet Verstärkung explizit). Neue Methode(n) direkt in
   `light/ray.rs`/`light/rays.rs` (Energie privat, kein Umweg möglich), z. B.
   `Ray::apply_gain_factor(&mut self, factor: f64)` ohne Obergrenze, plus eine
   `Rays`-Ebenen-Variante, die denselben Faktor auf eine Teilmenge von Rays anwendet (wird ab L1
   für die Zellen-weise Anwendung gebraucht, kann aber schon hier als generische Methode entstehen).
2. **Entfällt hier — siehe Vorbereitungsstufe.** Der Träger (`amp config`-Property auf allen
   Volumen-Nodes), der gemeinsame Volumen-Propagations-Helper und der neue Geometrie-Node-Typ
   entstehen in [`00_vorbereitung.md`](00_vorbereitung.md) (V1–V3). Dieses Dokument setzt sie
   voraus.
3. **`GainModel`-Trait-Vertrag.** Das Enum selbst existiert nach V1 bereits (mit `None` und einem
   Platzhalter). Hier kommt der Verhaltensvertrag dazu, den alle späteren Varianten erfüllen
   müssen (Teil C Phase 2): deklariert benötigte Fähigkeiten, bekommt lokale Größen (Wellenlänge,
   eingehende Fluenz/Energie, Weglänge), liefert neue Größen + Extraktionsbeitrag zurück,
   **mutiert selbst keinen Zustand**. Erste echte Variante (`Const`) kommt in Stufe L−1.
4. **Gruppenlaufzeit auf `Ray`.** Neues Akkumulatorfeld neben `path_length` (`ray.rs:54`).
   Speisung aus `n_g = n − λ·dn/dλ`: `DispersionFormula` (`refractive_index/bounded_model.rs:16`)
   hat aktuell **keine** Ableitung — muss um eine analytische bzw. für tabellierte Modelle eine
   spline-basierte Ableitung erweitert werden (keine naive numerische Differenzierung, siehe
   Root-Plan A.2). Betroffen: `refr_index_sellmeier1.rs` (analytisch herleitbar), `RefrIndexSchott`/
   `RefrIndexConrady` (Formel-basiert, ebenfalls analytisch ableitbar), `RefrIndexAir` (kann grob
   bleiben, Dispersion in Luft ist für die Zielanwendung vernachlässigbar aber sollte konsistent
   mitgeführt werden).
5. **Capability-Deklaration (minimal, A.6).** Kein generisches Registry-System (dafür gibt es
   nirgends Präzedenz, auch nicht für Analyzer-Kompatibilität — `AnalyzerType` ist ein
   geschlossenes 3-Varianten-Enum ohne jede Kompatibilitätsprüfung, siehe
   `opossum_core/src/opm_document.rs:297`). Stattdessen: eine schlanke Prüf-Funktion, die vor
   `OpmDocument::analyze()` läuft, für die im Fundament bekannten Fähigkeiten
   (`per_ray_wavelength`, `arrival_time`, `energy_weight`) einen verständlichen `OpmResult`-Fehler
   wirft statt eines stillen Fallbacks. Bewusst kein größeres System — wächst erst, wenn eine
   Stufe (L2) das erste Mal wirklich etwas davon braucht, das nicht durch den Rust-Typ ohnehin
   garantiert ist.
6. **Diagnostik-Slot.** Neues Feld/Property an den Volumen-Nodes (Pass-Index, Ankunftszeit-Tag),
   analog zum bereits vorhandenen `OpticSurface::hit_map`-Muster (`core_optics/optic_surface.rs:41`)
   — wird über `NodeReference`/`Arc<Mutex<..>>` automatisch zwischen Durchgängen geteilt (bereits
   existierender Mechanismus, siehe `nodes/reference.rs:42`).

## Backend

Keine Änderung nötig. Die Träger existieren nach der Vorbereitungsstufe bereits; alles Weitere in
dieser Phase ist Ray-interne Physik ohne Wire-Sichtbarkeit.

## GUI

Keine Änderung nötig. Die Bedienoberfläche (Dropdown, Wizard, Kontextmenü, Canvas-Status,
globales Panel) entsteht vollständig in [`00_vorbereitung.md`](00_vorbereitung.md). Gruppenlaufzeit
und Capability-Check sind für den Nutzer unsichtbar — sie äußern sich höchstens als
Fehlermeldung vor dem Trace.

## Tests / Abnahme

- Bekannter Glasblock: `n_g` gegen analytisch berechneten Wert aus Sellmeier.
- Zwei Wellenlängen durch denselben Block: differentielle Laufzeit gegen GDD·Δω.
- Vakuumstrecke: Gruppenlaufzeit = geometrische Länge / c.
- Volumen-Node mit `amp config = None` verhält sich wie eine reine Durchgangs-Node.
- **Regression:** bestehende Testsysteme liefern unveränderte Ergebnisse.

## Was diese Phase für alle folgenden Stufen bereitstellt

Ray-Energie-Mutator + Gruppenlaufzeit + Capability-Check + Diagnostik-Slot + den
`GainModel`-Verhaltensvertrag. Zusammen mit der Vorbereitungsstufe (Property-Träger,
Volumen-Helper, Geometrie-Node, Bedienoberfläche) ist damit die gesamte Infrastruktur fertig: jede
weitere Datei in diesem Ordner fügt nur noch **eine neue `GainModel`-Variante** und die dafür
nötigen Core-Bausteine hinzu.
