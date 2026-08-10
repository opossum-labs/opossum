# L−1 — `Const`

> Früherer Arbeitstitel dieser Stufe: `IdealAmplifier`. Maßgeblich ist die Nomenklatur aus
> [`README.md`](README.md).

Entspricht Teil C Phase 2 aus [`../plan_verstaerkung.md`](../plan_verstaerkung.md).
**Voraussetzung:** [`00_vorbereitung.md`](00_vorbereitung.md) und
[`00_fundament.md`](00_fundament.md).

## Ziel / neue Fähigkeit

Reine Buchhaltung, keine Materialphysik: Kettenauslegung, Systemüberblick. Erste echte
`GainModel`-Variante — ab hier verstärkt das Modell tatsächlich.

## Core

- `GainModel::Const { gain, max_extractable_energy, transmission, spectral_shape, aperture }`
  als erste echte Variante des in V1 angelegten Enums.
- Auswertung im gemeinsamen Volumen-Helper aus V2: liest `Rays::total_energy()` (existiert bereits,
  `light/rays.rs:534`) für den Energie-Deckel, skaliert jede Ray-Energie über den in
  `00_fundament.md` gebauten Mutator. Kein Zustand, keine Iteration nötig — eine Stufe ist ein
  einzelner multiplikativer Faktor, **der Laufweg spielt hier bewusst noch keine Rolle** (genau das
  ist der Unterschied zu L0).
- `spectral_shape` (λ₀, Δλ, Profil): einfache geschlossene Gewichtsfunktion über
  `ray.wavelength()`, **keine** `Spectrum`-Tabelle — passt zur Entscheidung gegen die
  Materialbibliothek und vermeidet den fehlenden GUI-Editor für `Proptype::Spectrum`.
- `aperture`: bestehende Mechanik direkt wiederverwendbar (`test_set_aperture`-Helper,
  `Aperture`-Proptype) — keine neue Arbeit.
- Energie-Deckel: `max_extractable_energy` begrenzt `gain` genau dann, wenn die geforderte
  Extraktion die Grenze überschreitet — reine Arithmetik auf der schon vorhandenen
  Bundle-Gesamtenergie, kein neuer Datenpfad.
- Mehrstufige Ketten und Multipass: **keine neue Infrastruktur** — normale Graph-Verkettung bzw.
  der bereits existierende `NodeReference`-Mechanismus (`nodes/reference.rs`) reichen aus.

## Backend

Keine Änderung. `gain`, `max_extractable_energy`, `transmission` sind einfache `F64`/`Energy`-
Proptypes, existieren bereits als Wire-Typen.

## GUI

Vollständig "for free": reine Zahlen-/Längen-Editoren über die generischen `get_primitive_editor`/
`get_geometric_editor`-Pfade (`node_editor/properties_editor/mod.rs`). Einzige neue Arbeit ist der
kleine `Const`-Sub-Editor für das `GainModel`-Dropdown aus der Vorbereitungsstufe — ein Formular
mit vier/fünf Zahlenfeldern, kein neues Widget-Muster.

## Tests / Abnahme

- Einzelstufe: Ausgangsenergie = Eingang × gain.
- Kette aus fünf Stufen: Produkt der Faktoren.
- Deckel greift: Ausgang = Eingang + `max_extractable_energy`.
- `spectral_shape` verschmälert das Spektrum in der erwarteten Richtung.
- Node-Referenz: fünf Durchgänge = gain⁵.
- **Abnahme:** ein realistisches Mehrstufen-Layout lässt sich vollständig durchrechnen und liefert
  plausible Energien pro Stufe.

## Modularität

Diese Stufe liefert das Muster, das L0–L2 nur noch erweitern: eine weitere `GainModel`-Variante
im selben Enum, ein weiterer Zweig im selben Volumen-Helper. Kein neuer Node-Typ, keine neue
Registrierung, kein neuer Backend- oder GUI-Mechanismus.
