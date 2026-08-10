# Verstärkungsmodellierung — Umsetzungspläne pro Eskalationsstufe

Dieser Ordner zerlegt den konzeptionellen Plan aus [`../plan_verstaerkung.md`](../plan_verstaerkung.md)
(Teil A–D) in einzelne, nacheinander abarbeitbare Umsetzungspläne. Jede Datei beantwortet für
**eine** Eskalationsstufe: was fehlt im Core, im Backend, im GUI — auf Übersichtsniveau, nicht
Zeilen-genau.

Die konzeptionellen Grundideen (Teil A), Arbeitsregeln (Teil B) und physikalischen Tests/
Abnahmekriterien pro Stufe (Teil C) stehen weiterhin nur in der Root-Datei — hier wird nicht
dupliziert, sondern verlinkt.

**Zum Einstieg:** [`UEBERSICHT.md`](UEBERSICHT.md) erklärt Leitidee, Aufbaureihenfolge,
Laufzeit-Datenfluss und Aufwandsverteilung im Zusammenhang — inklusive Diagrammen.

## Reihenfolge

```
── Stufe 00: Infrastruktur, keine Physik ──────────────────────
00_vorbereitung.md                 ── Struktur & Bedienoberfläche (zuerst)
        │
00_fundament.md                    ── physikalische Träger (Ray-Felder, Gruppenlaufzeit)
── Eskalationsstufen ──────────────────────────────────────────
        │
01_L-1_ideal_amplifier.md          ── erste nutzbare Verstärkung im Graphen
        │
02_L0_small_signal_gain.md         ── erste echte Spektroskopie (ray-weise)
        │
03_L1_frantz_nodvik_scalar.md      ── Sättigung, Fluenz-Kopplung, z-Marsch (Kernstück)
        │
        ├── 04_L2_frantz_nodvik_spectral.md   ── CPA / Gain Narrowing (baut auf L1 auf)
        │
        └── 05_pump_solver.md                 ── orthogonal zu L2, teilt sich die L1-Infrastruktur

06_L3_L4_L5_outlook.md             ── bewusst nur Stichpunkte, siehe Teil D des Root-Plans
```

Stufe 00 besteht aus zwei Dokumenten: `00_vorbereitung.md` legt fest, **wie** ein Verstärker
repräsentiert und bedient wird (Property-Träger, Geometrie-Node, Wizard, Kontextmenü,
Canvas-Status, globales Panel), `00_fundament.md` legt die **physikalischen Träger** nach
(Ray-Energie-Mutator, Gruppenlaufzeit, Capability-Check, Diagnostik). Die Vorbereitung kommt
zuerst, weil sie die Architekturentscheidungen enthält, auf denen alles Weitere aufbaut.

`04` und `05` sind beide von `03` (L1) abhängig, aber nicht voneinander — sie können in
beliebiger Reihenfolge oder parallel angegangen werden.

## Entscheidungen, die für alle Dokumente gelten

- **Keine Materialbibliothek.** Ein Kollege baut eine generische Materialbibliothek parallel;
  dieser Plan nimmt sie nicht vorweg. Spektroskopische Daten (σ_e, σ_a, τ_f) werden bis auf
  Weiteres **hartcodiert** (wenige Presets, z. B. Yb:YAG/Nd:Glas) hinter einer schmalen Trait-
  Schnittstelle eingebunden, damit die spätere Bibliothek nur diese Schnittstelle implementieren
  muss, statt L0–L2 umzubauen.
- **Ein wachsendes Modell-Enum, getragen als Property.** Es gibt nicht pro Stufe einen neuen
  Node-Typ, sondern ein `GainModel`-Enum, das mit jeder Stufe eine weitere Variante bekommt —
  genau das Muster, das `RefractiveIndexType` (`opossum_core/src/refractive_index/mod.rs`) im Code
  bereits vorlebt und für das GUI/Backend bereits einen generischen Auswahl-Mechanismus haben.
  Getragen wird dieses Enum als Property `amp config` von **allen Volumen-Nodes** (`Lens`,
  `Wedge`, `CylindricLens` sowie einem neuen Geometrie-Node-Typ), nicht von einem einzelnen
  dedizierten Verstärker-Node. Details und Begründung in
  [`00_vorbereitung.md`](00_vorbereitung.md). Das ist die technische Basis für die gewünschte
  Modularität: jede Stufe fügt hinzu, ersetzt nichts.
- **Gruppenlaufzeit und Capability-Deklaration werden upfront in `00_fundament.md` gebaut**,
  obwohl sie architektonisch erst ab L2 zwingend nötig wären — das ist eine bewusste
  Nutzerentscheidung (nicht meine ursprüngliche Empfehlung), siehe `00_fundament.md` für die
  Begründung im Klartext.
- **Backend- und GUI-Aufwand konzentrieren sich in der Vorbereitungsstufe, nicht in den
  Eskalationsstufen.** Ab L−1 gilt tatsächlich: Backend praktisch null (Node-Properties laufen
  generisch über das `Proptype`-Enum, `opossum_backend/src/nodes/properties.rs`), GUI ein bekanntes,
  begrenztes Muster (ein Dropdown-Eintrag plus ein kleiner Sub-Editor pro neuer Modellvariante).
  Die Bedienoberfläche selbst — Geometrie-Wizard, „as amplifier"-Kontextmenü, Amp-Status an der
  Node, globales Übersichtspanel — ist dagegen echte Arbeit und liegt vollständig in
  [`00_vorbereitung.md`](00_vorbereitung.md).
- **Tabellen/Kurven/2D-Felder haben keinen GUI-Editor-Präzedenzfall** — deshalb vermeidet dieser
  Plan sie bewusst (siehe hartcodierte Materialdaten oben).
- **Nomenklatur der Modellvarianten:** `None` (ungepumpt, Default) · `Const` (fester Faktor, keine
  Sättigung, Verteilung höchstens räumlich oder spektral fest) · `SmallSignalGain` (Unterschied zu
  `Const`: **der Laufweg zählt**) · `FrantzNodvik` · `FrantzNodvikSpectral` · `RateEquation` ·
  `EndPumping`. Die Dokumente `01`–`04` verwenden teils noch ältere Arbeitstitel
  (z. B. `IdealAmplifier` für `Const`); maßgeblich ist diese Liste.
- **Dynamische Ports sind ein eigenes Teilprojekt.** `EndPumping` soll je Facette einen Pump-Port
  erzeugen. Ports *hinzufügen* wäre billig, das *Entfernen* und die daraus folgende graphweite
  Aufräumarbeit dagegen nicht — die nötige Maschinerie fehlt komplett. Deshalb bleibt `EndPumping`
  vorerst außen vor; die Begründung mit allen fehlenden Teilen steht in
  [`00_vorbereitung.md`](00_vorbereitung.md) unter V7.
