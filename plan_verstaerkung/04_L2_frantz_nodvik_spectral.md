# L2 — `FrantzNodvikSpectral`

Entspricht Teil C Phase 7 aus [`../plan_verstaerkung.md`](../plan_verstaerkung.md).
**Voraussetzung:** [`03_L1_frantz_nodvik_scalar.md`](03_L1_frantz_nodvik_scalar.md).

## Ziel / neue Fähigkeit

CPA, Gain Narrowing, Rotverschiebung. Erste Stufe, in der die in `00_fundament.md` vorab gebaute
Gruppenlaufzeit und Capability-Prüfung tatsächlich gebraucht werden.

## Core

- **Zeit-Binning des Bundles** über die in `00_fundament.md` eingeführte Gruppenlaufzeit
  (`arrival_time`) — bei gechirpten Pulsen ergibt sich die λ↔t-Zuordnung automatisch aus dem
  Strecker, muss nicht vorgegeben werden. Alternativ eine explizite λ(t)-Chirp-Funktion als
  Fallback.
- Der Capability-Check aus `00_fundament.md` greift hier zum ersten Mal wirklich: der Analyzer
  muss `arrival_time` tatsächlich liefern, sonst bricht der Aufbau mit einem verständlichen Fehler
  ab, statt stillschweigend falsche Ergebnisse zu produzieren.
- **Bins geordnet abarbeiten**, jeder Bin sieht die Restinversion des vorherigen. Innerhalb eines
  Bins: eingefroren, zellenweise — das ist exakt die in L1 gebaute `InversionField` +
  `FieldEstimator` + `ExtractionReducer`-Kette, hier **unverändert wiederverwendet**, nur einmal
  pro Bin statt einmal pro Node-Durchlauf aufgerufen.
- Volles σ_e(λ)/σ_a(λ) über den Wellenlängenbereich statt eines Einzelwerts bei λ₀ — weiterhin
  hartcodiert (Nutzervorgabe), aber jetzt als kleine geschlossene Kurvenform (z. B. Gauß/
  Lorentz-Profil um λ₀) statt Tabelle, um weiterhin ohne Materialbibliothek und ohne den
  fehlenden GUI-Tabellen-Editor auszukommen.
- `n_time_slices` ist ein reiner Konvergenzparameter, kein Physikparameter — muss im Code und in
  der Doku klar so benannt/kommentiert sein, damit spätere Nutzer ihn nicht mit einer physikalischen
  Pulsdauer verwechseln.

## Backend

Keine Änderung. `n_time_slices` ist ein einfacher Zahlwert; `time_source` (`group_delay` vs.
explizite Chirp-Funktion) ist ein weiteres Enum-Proptype nach bekanntem Muster.

## GUI

- `time_source`-Dropdown nach dem etablierten Muster.
- Kein neuer Plot-/Tabellen-Bedarf — auch die Chirp-Funktion bleibt eine kleine geschlossene
  Formel mit wenigen Zahlenparametern, kein editierbares Kurven-Widget.

## Tests / Abnahme

- Konvergenz in `n_time_slices` — explizit dokumentieren, ab wann stabil.
- Grenzfall schmalbandig: geht in L1 über.
- Gain Narrowing: Ausgangsbandbreite < Eingangsbandbreite, Betrag gegen Literaturformel.
- Rotverschiebung des Schwerpunkts in der erwarteten Richtung; umgekehrter Chirp → umgekehrtes
  Vorzeichen der Verschiebung.
- Energiebilanz weiterhin erfüllt; Reihenfolgeunabhängigkeit innerhalb eines Bins.
- **Abnahme:** Ein CPA-Verstärker zeigt Gain Narrowing in der aus der Literatur bekannten
  Größenordnung.

## Modularität

Diese Stufe fügt ausschließlich eine Zeit-Binning-Schicht **über** L1 hinzu. Es entsteht kein
neuer Estimator, kein neues Inversionsfeld-Konzept, kein neuer Node-Typ.
