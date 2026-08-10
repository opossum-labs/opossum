# Verstärkungsmodellierung im Raytracer — Umsetzungsplan

Arbeitsdokument für die schrittweise Einführung aktiver Medien in die bestehende
node-basierte Optik-Simulation.

> Konkrete Umsetzungspläne pro Eskalationsstufe (was fehlt im Core, Backend, GUI) liegen in
> [`plan_verstaerkung/`](plan_verstaerkung/README.md), aufbauend auf einer Architektur-Recherche
> der aktuellen Codebase.

---

## Teil A — Grundideen

Dieser Teil ist die konzeptionelle Grundlage. Er enthält keine Umsetzungsschritte,
sondern die Entscheidungen, aus denen sich der Plan in Teil B ergibt.

### A.1 Vier Dinge, die getrennt bleiben müssen

Codes werden unflexibel, wenn diese vier vermischt werden:

1. **Payload** — was ein Ray trägt (Energie, Wellenlänge, Laufzeit, Hüllkurve, Feld)
2. **Medienzustand** — die Inversion, auf einem eigenen Gitter, unabhängig vom Ray-Sampling
3. **Verstärkungsmodell** — die lokale Physik, zustandsfrei
4. **Kopplungsschema** — Reihenfolge und Rückkopplung zwischen Licht und Medium

Punkt 4 ist der, der am schwersten nachrüstbar ist.

### A.2 Zeit statt Reihenfolge

Das Ergebnis darf **nicht** von der Iterationsreihenfolge abhängen (Array-Index,
Ray-ID, Thread-Scheduling), aber es **muss** von der Ankunftszeit abhängen.

Daraus folgt:

- Rays führen neben der optischen Weglänge (Phase) eine **Gruppenlaufzeit** mit:
  `t = t₀ + ∫ ds · n_g / c` mit `n_g = n − λ·dn/dλ`
- Batching erfolgt nach Ankunftszeit-Bin, nicht nach Schleifenposition
- Innerhalb eines Zeit-Bins ist der Medienzustand eingefroren; alle Beiträge werden
  gesammelt und **danach** angewandt
- Zwischen Zeit-Bins wird der Zustand fortgeschrieben

### A.3 Kein Doppelzählen der Sättigung

Frantz-Nodvik ist bereits die geschlossene Lösung des zeitabhängigen Problems.
Die Sättigung von Puls-Vorderflanke zu Hinterflanke steckt **in der Formel**.

- **L1 (skalar):** das gesamte Bundle ist genau ein Zeit-Bin. Kein Fortschreiben
  zwischen Rays — das wäre Doppelzählung.
- **L2 (spektral/CPA):** der Puls wird über die λ↔t-Zuordnung in Zeit-Bins zerlegt.
  Die Bins sind geordnet und sehen jeweils die Restinversion des vorherigen.
  Daraus entstehen Gain Narrowing und Rotverschiebung.
- Die Bin-Anzahl ist ein **Konvergenzparameter**, kein Physikparameter.

### A.4 Zellenweise Auswertung, nicht ray-weise

Mehrere gleichzeitige Rays können dieselbe Zelle treffen. Werden sie unabhängig
gegen den eingefrorenen Zustand ausgewertet, extrahieren sie in Summe potenziell
mehr Energie als vorhanden. Korrekt:

1. Estimator liefert die **Gesamtfluenz** aller gleichzeitigen Rays in der Zelle
2. Verstärkungsmodell wird **einmal pro Zelle** ausgewertet
3. Der resultierende Faktor wird auf alle Rays der Zelle angewandt,
   die Extraktion einmal deponiert

Konsequenz: die Node arbeitet auf **Bundle-Ebene**, nicht auf Ray-Ebene.

### A.5 z-Marsch

Die z-Richtung ist kausal: der Puls erreicht z=0 vor z=dz. Fortschreitendes
Update entlang z ist korrekt und gewollt.

Operativ: **äußere Schleife über z-Substeps, innere über Rays.** Nicht umgekehrt.
Bei verkipptem oder Brewster-geschnittenem Medium ist die Ebene senkrecht zur
mittleren Ausbreitungsrichtung gemeint.

### A.6 Capability-Verhandlung

Der Analyzer deklariert, was sein Bundle physikalisch hergibt. Das Modell
deklariert, was es braucht. Die Prüfung passiert beim **Zusammenbauen des
Systems**, nicht zur Laufzeit.

| Capability | Bedeutung |
|---|---|
| `energy_weight` | Ray trägt absolute Energie, Bundle kennt Gesamtenergie |
| `per_ray_wavelength` | jeder Ray hat eine eigene Wellenlänge |
| `arrival_time` | Gruppenlaufzeit wird mitgeführt |
| `temporal_envelope` | Ray trägt ein zeitliches Profil |
| `coherent_field` | komplexe Amplitude (später, Fourier-Kopplung) |

### A.7 Zustands-Lebenszyklus

Geometrie und Alignment sind **Definition** (read-only während des Laufs).
Die Inversion ist **Laufzeitzustand**. Sie braucht:

- **Init:** zu Beginn eines Schusses aus dem `InversionProvider`
- **Evolve:** Abbau über alle Durchgänge hinweg
- **Reset:** am Ende des Schusses

Referenzen auf dieselbe Node teilen den Zustand über Pointer-Semantik — das ist
korrekt und gewollt für Multipass. Zu prüfen ist nur, ob Caching bzw. partielle
Neuauswertung diesen impliziten Datenpfad kennt.

### A.8 Eskalationsstufen

| Stufe | Modell | Neue Fähigkeit |
|---|---|---|
| **L−1** | `IdealAmplifier` | Kettenauslegung, Systemüberblick |
| **L0** | `SmallSignalGain` | echte Spektroskopie, ungesättigt |
| **L1** | `FrantzNodvikScalar` | Sättigung, Extraktionseffizienz, Multipass |
| **L2** | `FrantzNodvikSpectral` | Gain Narrowing, Rotverschiebung, CPA |
| **L3** | `RateEquationMarching` | Pumpe während Verstärkung, Regen (später) |
| **L4** | ASE (später) | Speicherlimit, transversale Inversion |
| **L5** | Feldkopplung (später) | zusammen mit Fourier-Propagation |

---

## Teil B — Arbeitsweise

Regeln für die Umsetzung. Diese gelten für jede Phase.

1. **Erst analysieren, dann vorschlagen, dann implementieren.** Jede Phase
   beginnt mit einer Ist-Aufnahme der betroffenen Stellen in der Codebase und
   einem konkreten Umsetzungsvorschlag zur Freigabe. Nicht direkt loscoden.
2. **Eine Phase nach der anderen.** Keine Phase beginnt, bevor die vorherige
   abgenommen ist.
3. **Jede Phase endet mit grünen Tests**, inklusive der bestehenden.
   Physikalische Abnahmekriterien sind Teil der Definition of Done.
4. **Keine spekulativen Abstraktionen.** Nur bauen, was die aktuelle Phase
   und die unmittelbar folgende brauchen. Der Plan nennt die Richtung —
   das ist genug Vorschau.
5. **Regression zuerst:** bevor irgendetwas geändert wird, muss ein
   Referenzergebnis eines bestehenden Systems festgehalten sein. Nach jeder
   Phase muss es sich reproduzieren lassen.
6. **Bei Widersprüchen zwischen Plan und Codebase gewinnt die Codebase** —
   aber der Konflikt wird gemeldet, nicht stillschweigend umgangen.

---

## Teil C — Phasen

### Phase 0 — Ist-Aufnahme (keine Änderungen)

**Ziel:** verstehen, wie sich der Plan auf die konkrete Codebase abbildet.

**Zu beantworten:**

- Wie ist ein Ray repräsentiert? Welche Felder trägt er, wo werden sie akkumuliert?
- Wie werden Wellenlänge und optische Weglänge derzeit mitgeführt?
- Wie sieht die Node-Basisklasse aus? Was ist der Vertrag `Node.propagate(...)`?
  Ray-Level oder Bundle-Level?
- Wie funktioniert der Referenzmechanismus für Nodes technisch?
- Gibt es Caching oder partielle Neuauswertung des Graphen?
- Wie sind Materialien modelliert? Wo lebt `n(λ)`? Sellmeier oder Tabelle?
- Wo genau sitzt die Voronoi-Fluenzrekonstruktion, wovon hängt sie ab?
- Wie sind Analyzer aufgebaut? Kennt ein Bundle seine absolute Gesamtenergie?
- Wie ist die Volumenpropagation durch dickes Material implementiert
  (ein Sprung oder bereits segmentiert)?
- Wie sieht die Testinfrastruktur aus?

**Ergebnis:** kurzes Dokument mit den Antworten plus einer Liste der Stellen,
an denen der Plan auf Widerstand stößt.

**Nicht-Ziel:** irgendeine Codeänderung.

---

### Phase 1 — Fundament ohne Physik

**Ziel:** alle Träger einbauen, die spätere Modelle brauchen. Nach dieser Phase
verhält sich das System physikalisch identisch zu vorher.

**Umzusetzen:**

1. **Gruppenlaufzeit auf dem Ray.** Getrenntes Akkumulatorfeld neben der
   optischen Weglänge. `n_g` aus dem Materialmodell:
   - Sellmeier vorhanden → analytische Ableitung `dn/dλ`
   - nur Tabellen → C²-Spline oder Sellmeier-Fit. **Keine naive numerische
     Differentiation**, das verstärkt Interpolationsrauschen massiv.
   - Für Vakuum/Luft-Strecken trivial, aber konsistent mitführen.
2. **Capability-Deklaration** auf Analyzer und Bundle (siehe A.6).
3. **Absolute Bundle-Energie**: Analyzer deklariert die Gesamtenergie bzw.
   Pulsenergie. Ohne sie ist jedes sättigende Modell wertlos.
4. **Validierungs-Hook beim Systemaufbau**: Nodes können Capabilities anfordern,
   fehlende führen zu einem verständlichen Fehler *vor* dem Trace.
5. **Diagnostik-Slot an Nodes**, pass-getaggt (Pass-Index, Ankunftszeit).
   Historie wird über Referenzen geteilt — die Einträge müssen sich nachträglich
   nach Durchgang auftrennen lassen.

**Tests:**

- Bekannter Glasblock: `n_g` gegen analytisch berechneten Wert aus Sellmeier
- Zwei Wellenlängen durch denselben Block: differentielle Laufzeit gegen
  GDD·Δω aus der analytischen Dispersion
- Vakuumstrecke: Gruppenlaufzeit = geometrische Länge / c
- **Regression:** bestehendes Testsystem liefert unveränderte Ergebnisse

**Abnahme:** Gruppenlaufzeit stimmt für mindestens zwei Materialien mit
analytischer Rechnung überein; alle Altfunktionalität unverändert.

---

### Phase 2 — Node-Skelett und `IdealAmplifier` (L−1)

**Ziel:** die Verstärker-Node existiert und ist im Graphen benutzbar.

**Umzusetzen:**

1. Verstärker-Node als reguläre Node im bestehenden System.
2. `GainModel`-Schnittstelle, minimal:
   - deklariert benötigte Capabilities
   - erhält lokale Größen, gibt neue Größen plus Extraktionsbeitrag zurück
   - **mutiert keinen Zustand**
3. `IdealAmplifier` als erste Implementierung.

**Parameter `IdealAmplifier`:**

| Parameter | Pflicht | Bedeutung |
|---|---|---|
| `gain` | ja | Energieverstärkung (Faktor oder dB) |
| `max_extractable_energy` | nein, empfohlen | deckelt `gain`, sobald das Bundle mehr zöge |
| `transmission` | nein | passive Verluste |
| `spectral_shape` | nein | (λ₀, Δλ, Profil) — Gain Narrowing ohne Physik |
| `aperture` | nein | sonst aus Geometrie |

Capabilities: **keine**. Läuft mit jedem Analyzer.

Der Energiedeckel ist wichtig: ohne ihn produziert eine achtstufige Kette
kommentarlos absurde Energien.

**Tests:**

- Einzelstufe: Ausgangsenergie = Eingang × gain
- Kette aus fünf Stufen: Produkt der Faktoren
- Deckel greift: Ausgang = Eingang + `max_extractable_energy`
- `spectral_shape` verschmälert das Spektrum in der erwarteten Richtung
- Node-Referenz: fünf Durchgänge = gain⁵

**Abnahme:** Ein realistisches Mehrstufen-Layout lässt sich vollständig
durchrechnen und liefert plausible Energien pro Stufe.

---

### Phase 3 — Materialdaten und Inversionsfeld

**Ziel:** die physikalische Datengrundlage, noch ohne Verstärkungsmodell.

**Umzusetzen:**

1. **`GainMaterialData`** — reine Daten, kein Verhalten:
   - `σ_e(λ)`, `σ_a(λ)` (Tabelle + Interpolation)
   - `τ_f`, Dotierdichte `N_dop`
   - `n(λ)` — an das bestehende Materialsystem anschließen, nicht duplizieren
   - abgeleitet: `F_sat(λ) = hν/(σ_e+σ_a)`, Transparenzinversion `β_min`
   - optional: Temperaturabhängigkeit
2. **`InversionField`** — eigenes Gitter, unabhängig vom Ray-Sampling.
   Sampling an einem Punkt, Deposition in eine Zelle, Klonen, Reset.
3. **`InversionProvider`**, zwei Implementierungen:

   | Provider | Parameter |
   |---|---|
   | `Uniform` | eine Zahl, wahlweise `beta`, `N2` [1/cm³] oder `stored_energy_density` [J/cm³] — alle drei erlauben, intern auf eine normieren |
   | `AnalyticProfile` | Ausdruck oder Vorlage (Super-Gauß transversal, Exponential in z), Amplitude, Gitterauflösung |

4. **Lebenszyklus** nach A.7: Init / Evolve / Reset, an den Schuss-Scope gebunden.

**Tests:**

- Umrechnung `beta` ↔ `N2` ↔ `stored_energy_density` ist konsistent und
  rundreisefest
- Gespeicherte Gesamtenergie durch Integration über das Gitter = analytischer
  Wert für ein homogenes Profil
- Reset stellt den Ausgangszustand wieder her
- Zwei Node-Referenzen sehen dasselbe Feldobjekt

**Abnahme:** Für ein reales Material (z. B. Yb:YAG oder Nd:Glas) sind
Querschnitte geladen und `F_sat` stimmt mit dem Literaturwert überein.

---

### Phase 4 — `SmallSignalGain` (L0)

**Ziel:** erste echte Physik. Noch ray-weise auswertbar, kein Estimator nötig.

**Umzusetzen:**

1. Segmentierung des Innenpfads (`n_steps`), Schrittweite an die Zellgröße
   des Inversionsgitters gekoppelt.
2. `G = exp(∫ σ_e(λ)·ΔN ds)`, optional mit Reabsorption `−σ_a(λ)·N_1`.
3. Kein Zustands-Update (frozen inversion).
4. **Warndiagnostik**: wenn die extrahierte Energie die gespeicherte übersteigt,
   ist man außerhalb des Gültigkeitsbereichs — sichtbar melden, nicht abstürzen.

**Parameter:**

| Parameter | Bedeutung |
|---|---|
| `material` | `GainMaterialData` |
| `inversion_provider` | |
| `n_steps` | Marching-Auflösung |
| `include_reabsorption` | Quasi-3-Niveau |

Capabilities: `per_ray_wavelength`.

**Tests:**

- Homogene Inversion, gerader Durchgang: `G` gegen analytisch `exp(g₀L)`
- Negative Inversion → Beer-Lambert-Absorption, gegen Analytik
- Konvergenz: Ergebnis stabil bei Verdopplung von `n_steps`
- Chromatik: Verstärkung folgt der Form von `σ_e(λ)`
- Schräger Durchgang: Verstärkung skaliert mit der tatsächlichen Weglänge
- Warnung feuert bei überzogener Extraktion

**Abnahme:** Kleinsignalverstärkung eines realen Verstärkerkopfs stimmt mit
einer unabhängigen Handrechnung überein.

---

### Phase 5 — `FieldEstimator` und Bundle-Level-Propagation

**Ziel:** die strukturelle Voraussetzung für alles Sättigende.
Noch keine neue Physik.

**Umzusetzen:**

1. **Voronoi-Rekonstruktion herauslösen** aus der Optik-Auswertung in eine
   eigenständige Komponente, aufrufbar an beliebigen Ebenen im Volumen.
   - Schnittstelle: Bundle + Ebene → Fluenz pro Zelle
   - Austauschbare Implementierungen: Voronoi, festes Gitter, KDE
   - Glättungsparameter explizit, nicht implizit
2. **Node-Vertrag auf Bundle-Ebene** umstellen (siehe A.4).
3. **z-Marsch**: äußere Schleife über Substeps, innere über Rays (siehe A.5).
   Ebenendefinition bei verkippten/Brewster-Medien sauber behandeln.

**Tests:**

- Bekanntes analytisches Strahlprofil (Gauß, Top-Hat): rekonstruierte Fluenz
  gegen Analytik, für verschiedene Ray-Zahlen
- Energieerhaltung des Estimators: Integral der Fluenz = Bundle-Energie
- Konvergenz mit steigender Ray-Zahl
- Ergebnis unabhängig von der Ray-Reihenfolge im Array (Permutationstest)
- **Regression:** die bestehende Fluenzauswertung an Optiken liefert
  unveränderte Ergebnisse

**Abnahme:** Der Estimator ist an mindestens einer Ebene *innerhalb* eines
Volumens aufrufbar und erhält die Energie.

---

### Phase 6 — `FrantzNodvikScalar` (L1)

**Ziel:** Sättigung. Ab hier macht das System echte Verstärkerauslegung.

**Umzusetzen:**

1. Pro Substep und Zelle, mit der **Gesamtfluenz** aller Rays der Zelle:

   `F_out = F_sat · ln{ 1 + [exp(F_in/F_sat) − 1] · G₀ }`,  `G₀ = exp(σ_e·ΔN·ds)`

2. Der resultierende Faktor wird auf alle Rays der Zelle angewandt.
3. **`ExtractionReducer`**: sammelt Beiträge, schreibt danach ins Inversionsfeld.
4. Fortschreiben entlang z, eingefroren innerhalb eines Substeps.
5. Gültigkeitsprüfung: τ_Puls ≪ τ_f, keine Pumpe während des Durchgangs —
   als Assertion mit der Pulsdauer als Metadatum vom Analyzer.

**Parameter:**

| Parameter | Bedeutung |
|---|---|
| `material` | σ_e, σ_a bei λ₀ |
| `central_wavelength` | λ₀ |
| `inversion_provider` | |
| `estimator` | Implementierung + Glättung |
| `n_steps` | |
| `update_state` | bool |

Capabilities: `energy_weight` + absolute Bundle-Energie.
**Keine Pulsdauer nötig** — FN ist zeitintegriert und pulsformunabhängig.

**Tests — das ist die kritischste Phase, entsprechend gründlich:**

- Homogene Inversion, Top-Hat-Strahl: gegen die analytische FN-Lösung
- Grenzfall `F_in ≪ F_sat`: geht in `F_in·G₀` über (Anschluss an L0)
- Grenzfall `F_in ≫ F_sat`: geht in `F_in + F_sat·ln G₀` über
- **Globale Energiebilanz:** extrahierte Photonenenergie = Abnahme der
  gespeicherten Energie, bis auf τ_f-Zerfall. Als dauerhafte Assertion einbauen,
  nicht nur als Test — sie fängt praktisch jeden Fehler in der
  Estimator-Reducer-Kette ab.
- **Keine Übersättigung:** ΔN wird nie negativ (bzw. nie unter β_min)
- **Reihenfolgeunabhängigkeit:** Permutation der Rays ändert nichts
- **Multipass:** vier Durchgänge über Node-Referenzen; Gewinn pro Pass fällt
  monoton, Gesamtextraktion konsistent mit der gespeicherten Energie
- **Reset:** zweiter Lauf desselben Systems liefert identische Ergebnisse
  (fängt vergessenes Zurücksetzen des Zustands)
- Konvergenz in `n_steps` und in der Ray-Zahl
- Gaußstrahl: transversales Ausbrennen sichtbar, Fluenzprofil flacht ab

**Abnahme:** Extraktionseffizienz und Ausgangsenergie eines realen
Multipass-Verstärkers reproduzieren eine unabhängige 1D-Referenzrechnung
innerhalb weniger Prozent.

---

### Phase 7 — `FrantzNodvikSpectral` (L2)

**Ziel:** CPA, Gain Narrowing, Rotverschiebung.

**Umzusetzen:**

1. Zeit-Binning des Bundles über die Ankunftszeit aus Phase 1 —
   bei gechirpten Pulsen ergibt sich die λ↔t-Zuordnung **automatisch aus dem
   Strecker**, sie muss nicht vorgegeben werden.
2. Bins geordnet abarbeiten, jeder sieht die Restinversion des vorherigen.
3. Innerhalb eines Bins: eingefroren, zellenweise, wie L1.
4. Alternativ explizite λ(t)-Chirp-Funktion als Fallback.

**Parameter:**

| Parameter | Bedeutung |
|---|---|
| `material` | volle σ_e(λ), σ_a(λ) |
| `n_time_slices` | **Konvergenzparameter** |
| `time_source` | `group_delay` oder explizite Chirp-Funktion |
| sonst wie L1 | |

Capabilities: `per_ray_wavelength` + `arrival_time`.

**Tests:**

- **Konvergenz in `n_time_slices`** — explizit dokumentieren, ab wann stabil
- Grenzfall schmalbandig: geht in L1 über
- Gain Narrowing: Ausgangsbandbreite < Eingangsbandbreite, Betrag gegen
  Literaturformel für den verwendeten Verstärker
- Rotverschiebung des Schwerpunkts in der erwarteten Richtung
- Umgekehrter Chirp → umgekehrtes Vorzeichen der Verschiebung
- Energiebilanz weiterhin erfüllt
- Reihenfolgeunabhängigkeit innerhalb eines Bins

**Abnahme:** Ein CPA-Verstärker zeigt Gain Narrowing in der aus der Literatur
bekannten Größenordnung.

---

### Phase 8 — `PumpSolver`

**Ziel:** Inversion aus einem Pumplaser berechnen statt vorgeben.

**Kernidee:** Das ist **dieselbe Node-Maschinerie mit umgedrehtem Vorzeichen** —
kein Sonderfall. Ein Pump-Analyzer erzeugt ein Bundle, das durch dasselbe Device
läuft; ein `AbsorptionModel` deponiert Energie statt sie zu entziehen.

**Parameter:**

| Parameter | Bedeutung |
|---|---|
| `pump_wavelength` | |
| `pump_energy` / `pump_power` | |
| `pump_duration` | |
| Geometrie | Ein-/Zweiseitenpumpen — ergibt sich aus dem Graphen |
| `sigma_a(λ_p)` | aus `GainMaterialData` |
| `quantum_efficiency` | |
| `tau_f` | Zerfall während des Pumpens |
| `N_dop` | |
| `absorption_saturation` | optional |

**Ausgabe:** Inversionsfeld **und** Wärmedeposition — letztere wird ohnehin für
die thermische Linse gebraucht.

**Tests:**

- Beer-Lambert-Absorptionsprofil gegen Analytik bei schwacher Pumpe
- Energiebilanz: absorbierte Pumpenergie = gespeicherte Energie + Quantendefekt-Wärme
- Zweiseitenpumpen ergibt symmetrisches Profil
- τ_f-Zerfall bei langer Pumpdauer korrekt
- Resultierendes Profil in L1 eingespeist liefert plausible Extraktion

**Abnahme:** Für einen realen Verstärkerkopf stimmen Absorptionsgrad und
gespeicherte Energie mit der Auslegungsrechnung überein.

---

## Teil D — Bewusst zurückgestellt

Diese Punkte sind bekannt und nicht vergessen, aber nicht Teil dieses Plans:

- **L3 `RateEquationMarching`** — nötig, wenn Pulsdauer und τ_f vergleichbar
  werden oder während der Verstärkung gepumpt wird (Regen, Q-Switch).
  Baut auf `temporal_envelope` auf.
- **Echte Zeitschrittverfahren mit Event-Queue** — nötig, sobald zwei Durchgänge
  *gleichzeitig* im Medium sind oder gegenläufig gepumpt wird. Der z-Marsch
  reicht für sequentielle Multipass-Layouts.
- **L4 ASE** — braucht Ray-Quellen, die aus Medienzellen heraus emittieren.
  Der `PumpSolver` aus Phase 8 legt die Struktur dafür bereits an.
- **L5 Feldkopplung** — kommt zusammen mit der Fourier-Propagation. Weil die
  `GainModel`-Schnittstelle auf Fluenz/Intensität definiert ist und nicht auf
  Ray-Energie, ist dasselbe Modellobjekt punktweise auf ein Feldgitter anwendbar
  (Split-Step: dz propagieren → Gain anwenden → dz propagieren).
- **Thermische Linse und Depolarisation** — die Wärmedeposition entsteht bereits
  in Phase 8, die optische Rückwirkung ist ein separates Thema.
