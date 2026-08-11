# Vorbereitung — Struktur und Bedienoberfläche (vor jeder Physik)

Erster Teil der Stufe 00. Der zweite Teil ist [`00_fundament.md`](00_fundament.md) (die
physikalischen Träger). Diese Datei kommt zuerst: sie legt fest, **wie ein Verstärker im Modell
überhaupt repräsentiert und bedient wird**, noch bevor irgendeine Verstärkungsphysik existiert.

Nach dieser Stufe verhält sich das System physikalisch exakt wie heute — es existieren nur die
Träger, die Bedienoberfläche und der leere Modellzustand `None`.

---

## Warum es diese Stufe gibt

Die ursprünglichen Stufen-Dokumente behandelten Verstärkung als reines Physik-Feature und gingen
davon aus, dass Backend und GUI fast nichts zu tun haben. Das stimmt für die *Physik-Parameter*,
aber nicht für die Bedienung: Geometrie-Wizard beim Anlegen, „as amplifier" auf bestehenden Nodes,
Amp-Status an der Node, ein dokumentweites Übersichtspanel. Diese Dinge sind untereinander
abhängig und gehören vor die Physik, nicht zwischen die Eskalationsstufen verstreut.

---

## Architekturentscheidungen

Diese sechs Entscheidungen gelten ab hier für alle folgenden Dokumente.

### 1. Amp-Config ist eine Property, kein eigener Node-Typ

`Lens`, `Wedge` und `CylindricLens` deklarieren dauerhaft eine Property `amp config` mit Default
`None`. „As amplifier" ist damit ein **normaler Property-Patch** über den bestehenden generischen
Endpunkt — kein neues Backend-Operations-Modul, kein UUID-Jonglieren, kein Nachspielen von
Verbindungen, Undo funktioniert über den vorhandenen Property-Undo.

Begründung: `NodeAttr` (`core_optics/node_attr.rs:50-76`) ist konsequent typ-agnostisch — jedes
Feld wird generisch von den Blanket-Impls und der Backend-CRUD konsumiert. Der dokumentierte Weg
für node-typ-spezifische Konfiguration ist ein `Proptype` in `props`, genau wie `Lens` seine
Krümmungen liest (`nodes/lens/mod.rs:221-258`).

Wichtig für den `.opm`-Roundtrip: `set_node_attr` **merged** Properties
(`core_optics/node_attr_ext.rs:169-187`) — Properties aus der Datei, die die Default-Node nicht
kennt, fallen still weg. Deshalb muss die Property **immer** deklariert sein und darf nicht erst
beim Umschalten entstehen.

### 2. Die Gain-Physik lebt in einem gemeinsamen Volumen-Helper

Nicht pro Node-Typ implementiert, sondern einmal — aufgerufen von allen Volumen-Nodes. Ideal- und
Detektor-Nodes bekommen die Property gar nicht erst und können damit nicht verstärken.

### 3. Zusätzlich ein neuer Volumen-Node-Typ mit Geometrie-Auswahl

Scheibe, Rod und eckiger Slab existieren heute schlicht nicht — es gibt nur Linse (sphärisch),
Wedge (planparallel/keilförmig) und Zylinderlinse. Der neue Typ trägt dieselbe
`amp config`-Property.

### 4. Amp-Config-Bedienung: Kurzstatus an der Node, Bearbeitung im Seitenpanel

### 5. Globale Übersicht als zweite Ansicht im Seitenpanel

Kein separates Fenster, sondern ein schmaler vertikaler Umschalter neben dem bestehenden Panel
(VS-Code-artig): „Node-Properties" ↔ „Amp-Config".

### 6. Dynamische Ports / End pumping sind ein eigenes späteres Teilprojekt

Siehe [V7](#v7--zurückgestellt-dynamische-ports--end-pumping).

---

## Umsetzung

Reihenfolge = Commit-Reihenfolge. V1–V2 sind Core, V3–V6 bauen darauf auf und sind einzeln
abnehmbar.

### V1 — Amp-Config als Property (Core)

Neue `Proptype`-Variante, die das `GainModel`-Enum hält (`properties/proptype.rs:64-132`, plus
`to_html`/`export_data`-Arm). Enum-Varianten zunächst nur `None` (Default) und ein Platzhalter;
die echten Modelle kommen mit den Physikstufen dazu.

Deklariert von `nodes/lens/mod.rs`, `nodes/wedge/mod.rs`, `nodes/cylindric_lens/…`. Physikalisch
passiert nichts — `None` ist Durchgang.

**Test:** `.opm`-Roundtrip mit und ohne gesetzte Property; Altdateien ohne die Property laden
sauber mit Default; Regression aller bestehenden Systeme.

### V2 — Gemeinsamer Volumen-Propagations-Helper (Core)

`Lens`, `Wedge` und `CylindricLens` rufen heute jeweils selbst zweimal
`pass_through_surface_generic` auf (`nodes/lens/analysis_raytrace.rs:31-48`) — Eintritt mit
Materialindex, Austritt mit Umgebungsindex. Diese Sequenz wird in einen gemeinsamen Helper neben
`unified_analyze_single_surface_node` (`core_optics/optic_node_ext.rs:326`/`:365`) gezogen.

In dieser Stufe **verhaltensneutral** — der Helper tut exakt, was die drei Nodes heute tun. Er ist
die Naht, an der später die Segmentierung aus [L0](02_L0_small_signal_gain.md) und die
Fluenz-Kopplung aus [L1](03_L1_frantz_nodvik_scalar.md) landen, ohne dass drei Nodes einzeln
angefasst werden müssen.

**Test:** Regression — die drei Node-Typen liefern identische Ergebnisse wie vorher.

### V3 — Neuer Volumen-Node-Typ + Geometrie-Wizard (Core / Backend / GUI)

**Core:** neuer Node-Typ mit `Geometry`-Enum-Property, der in `update_surfaces()` verzweigt —
exakt das `Lens`-Muster (`nodes/lens/mod.rs:219-287`): Scheibe/Rod = zwei `Plane`, Rod mit
sphärischen Enden = zwei `Sphere`, transversale Form über eine Port-`Aperture`
(`ApertureShape::BinaryCircle` bzw. `BinaryRectangle`, `apertures/mod.rs:326`). Trägt dieselbe
`amp config`-Property aus V1.

> **Explizites Nicht-Ziel: Mantelflächen.** Es gibt keinen begrenzten `GeoSurface` — nur `Plane`,
> `Sphere`, `Cylinder` (als Brechfläche, nicht als Rod-Mantel) und `Parabola`; `Cuboid` existiert
> nur im ungenutzten Render-Pfad. Strahlen sehen die Seitenwände eines Rods also nicht,
> Totalreflexion am Mantel ist nicht modelliert. Das ist eine bestehende Lücke der Codebase, keine
> neue — aber sie ist genau für endgepumpte Rods und ASE relevant.

**GUI-Wizard:** `AppCommand::AddNode` wird abgefangen wie beim bestehenden „Unsaved
Changes"-Dialog (`components/app.rs:208-263`, `pending_action`-Muster); das Modal selbst nach dem
Vorbild von `components/settings_dialog.rs` (Scratch-Buffer, Commit erst beim Bestätigen; dessen
Tab-Umschaltung ist auch die Vorlage, falls der Wizard je mehrschrittig werden soll). Die Geometrie
muss durch `AppCommand::AddNode` → `NodeEditorCommand::AddNode` → `GraphsWorkspaceAction::AddOpticNode`
→ `NewNode` (`opossum_core/src/types/api_types.rs:187-192`) gefädelt werden.

*Empfehlung:* `NewNode` um ein optionales Parameterfeld erweitern statt „anlegen und danach
patchen" — eine atomare Operation, sauber für Undo.

Dazu: Icon-Asset + Eintrag in `NodeType::icon()` (`scenery_editor/node/mod.rs:62-81`). Achtung auf
die Casing-Konvention (Menü sentence case, POST lowercase, Vergleiche lowercase-mit-Leerzeichen).

### V4 — Amp-Umschaltung im Kontextmenü (GUI)

Eintrag in `scenery_editor/node/node_component.rs:140-179`, **nur** für Volumen-Node-Typen
(Lens / Wedge / CylindricLens / neuer Typ). Das heutige Entweder-Oder (`Create reference` XOR
`Group optical nodes`) muss dafür zu einer Sammlung werden — `CxMenu::add_entry` existiert bereits
(`components/context_menu/cx_menu.rs:52`).

**Der Eintrag ist ein Umschalter, keine Einbahnstraße.** Ist die Node heute passiv, heißt er
„As amplifier" und setzt ein aktives Modell; ist sie bereits ein Verstärker, heißt er
„As passive optic" und setzt `GainModel::None` zurück. Ohne den Rückweg wäre eine versehentlich
verstärkende Node nur noch über das Property-Panel (V6) oder gar nicht zu heilen. Welcher der
beiden Beschriftungen gilt, entscheidet der Amp-Marker aus V5 — die Canvas kennt den Zustand
dadurch bereits lokal und muss beim Rechtsklick nichts nachladen. V4 und V5 hängen an dieser
Stelle also zusammen: der Umschalter setzt V5 voraus.

Die Aktion ist ein Property-Patch über den vorhandenen Pfad (`api::update_node_property`,
`api/node.rs:332`) — **kein neuer Endpunkt**. Plumbing ist formelhaft: je eine Variante plus Arm in
`CxtCommand`, `app.rs:265`-Effekt, `NodeEditorCommand`, `node_editor_command.rs:128`,
`GraphsWorkspaceAction`, `workspace_processor.rs:80`. Die Variante trägt das zu setzende
`GainModel`, damit beide Richtungen denselben Weg nehmen.

**Undo/Redo muss den Amp-Zustand mit zurücknehmen.** Das ist der eigentliche Grund, warum die
Amp-Config eine Property ist (Architekturentscheidung 1): `patch_property`
(`opossum_backend/src/nodes/properties.rs`) legt bereits ein `Command::PatchProperty` mit `old`/`new`
an, der Undo-Stack trägt den Zustandswechsel also gratis. Zu tun bleibt die **GUI-Seite**: das
zugehörige `DocumentChange::NodeDetailsChanged` trägt bewusst keine Werte, der Amp-Marker auf der
Canvas ist aber ab V5 Canvas-Zustand und veraltet damit beim Undo. Er muss in
`apply_document_changes` gezielt für diese eine Node nachgelesen werden. Ein Backend-Regressionstest
hält fest, dass ein Undo die `amp config` tatsächlich auf den vorherigen Wert zurückdreht.

### V5 — Amp-Status an der Node (GUI)

Kompakte Statuszeile unter dem Node-Body. `GraphNodeContent` nimmt bereits ein `body: Element`
(`scenery_editor/node/graph_node_components.rs:7-26`); ein Footer-Geschwister reicht. Ports liegen
innerhalb `.node-body`, **Kanten verschieben sich also nicht** — das ist der strukturelle Gewinn.

Zwei Dinge sind dafür nötig:

- `NodeElement::total_height()` einführen und die ~6 Höhen-Konsumenten darauf umstellen
  (`node/mod.rs:156-166`, `graph_view_component.rs:57` und `:100-103`, `graph_store.rs:355-364`
  und `:461`, `workspace_processor.rs:1323`).
- `NodeInfo` (`api_types.rs:107-128`) braucht einen kleinen Marker (z. B. `amp_model:
  Option<String>`). Die Canvas darf **nicht** pro Node die Properties nachladen — das wären N
  Requests pro Render.

Dies ist das erste node-typ-spezifische *Layout* im Code; es gibt kein Vorbild (bisher
unterscheiden sich Node-Typen visuell nur durch Icon und Header-Klasse), aber die Kompositionsnaht
existiert und das Risiko ist gering.

Der Marker ist nicht nur Anzeige: er ist auch die Zustandsquelle für den Umschalter aus V4 und
muss deshalb auf **allen** Wegen aktuell bleiben, auf denen sich die `amp config` ändert —
Kontextmenü, Property-Panel (V6) und Undo/Redo. Beim direkten Patch ist der neue Wert bekannt und
wird ohne Rückfrage gesetzt; beim Undo/Redo ist er es nicht und muss für die betroffene Node
nachgelesen werden.

### V6 — Sidebar-Umschalter + globales Amp-Panel (GUI / Backend)

**GUI-Struktur:** `graph_editor_component.rs:126-134` rendert heute direkt `NodeConfigEditor`.
Daraus wird ein Container mit schmaler vertikaler Auswahlleiste: Ansicht 1 = bestehende
Node-Properties (selektionsgebunden, unverändert), Ansicht 2 = dokumentweite Amp-Config-Liste.

**Backend:** neuer Endpunkt nach dem Vorbild von `get_available_sources`
(`opossum_backend/src/analyzers.rs:277-294`). Dabei `collect_source_ports` (`analyzers.rs:28-57`)
zu einem geteilten `collect_nodes_of_type` in `helper_functions/graph_lookup.rs`
verallgemeinern — es gibt bereits **zwei** divergente rekursive Sammler (`analyzers.rs:28` und
`optic_graph/inspection.rs:371`), ein dritter wäre das falsche Ergebnis. Die Gruppenzuordnung ist
gratis: die Rekursion kennt die Elterngruppe bereits und wirft sie heute nur weg.

**Liste:** Kartenmuster aus `analyzer_node_editor/energy_editor.rs:23-114`; „Springe dorthin" über
`JumpTarget` + `ensure_tab_active` (`workspace_processor.rs:695-760`); Karte gezielt aufklappen
über `PENDING_SOURCE_CARD_OPEN` + `use_source_card_focus`.

> **Bekannte Falle:** `NODE_DETAILS_REFRESH` (`lib.rs:25`) wird nur bei Undo/Redo
> (`workspace_processor.rs:610,613`) und beim Property-Save (`node_config_editor.rs:207`) erhöht.
> Heute fällt das nicht auf, weil das Analyzer-Panel bei Selektionswechsel neu gemountet wird. Ein
> dauerhaft sichtbares Panel veraltet beim normalen Hinzufügen/Löschen auf der Canvas — es braucht
> einen Bump im Add/Remove-Pfad oder einen eigenen Zähler.

**Bearbeitungssemantik:** anders als die Source-Port-Karten, die eine Zuordnung *eines selektierten
Analyzers* bearbeiten, patcht hier jede Karte **ihre eigene** Node. Das Dirty-Tracking
(`node_config_editor.rs:45-51`) muss also außerhalb von `NodeConfigEditor` neu aufgesetzt werden.

### V7 — Zurückgestellt: dynamische Ports / End pumping

Eigenes Teilprojekt. Ports *hinzufügen* wäre billig — `update_surface` legt fehlende Ports
automatisch an (`core_optics/optic_node_ext.rs:260-276`), und Default-Ports werden nicht einmal
serialisiert. Das ist eine Falle: der teure Teil ist alles andere.

Es fehlt:

- `OpticPorts` hat kein `remove()` (`core_optics/optic_ports.rs:137-344`) — End pumping wieder
  abzuschalten könnte das Port-Set nicht schrumpfen.
- Ein Property-Patch löst kein `update_surfaces()` aus (`undo/node_commands.rs:305-320`) — die
  Ports würden sich also gar nicht erst ändern.
- `cleanup_orphan_connections_and_mappings` (`optic_graph/construction.rs:75-161`) macht genau das
  Richtige (rekursiv, Port-Maps und Kanten), ist aber privat mit einem einzigen Aufrufer
  (`delete_node:235`).
- `port_map_cascade.rs` deckt nur Node-Löschung ab, nicht verschwindende Ports.
- `DocumentChange` (`api_types.rs:762-790`) hat keinen Port-Änderungs-Fall; die GUI erführe nicht,
  dass ein Property-Edit Kanten in einem anderen Gruppen-Tab gelöscht hat.

Das ist die einzige Anforderung, die aus einer node-lokalen Property-Änderung eine **graphweite
strukturelle Mutation** macht. Bis dahin bleibt `EndPumping` als Modellvariante außen vor.

---

## Verifikation

- **Regression zuerst** (Teil B, Regel 5 des Root-Plans): vor V1 ein Referenzergebnis eines
  bestehenden Systems festhalten; nach jeder Teilstufe reproduzieren.
- `cargo test` nach jeder Teilstufe; `cargo fmt` vor `cargo clippy`, Clippy mit den CI-Lints.
- Nach V1: `.opm`-Roundtrip in beide Richtungen (mit/ohne Property, Altdatei).
- Nach V2: identische Ergebnisse der drei Volumen-Node-Typen gegen den Stand davor.
- Nach V3–V6 jeweils manuell in der laufenden GUI prüfen (Wizard legt die richtige Geometrie an,
  der Amp-Eintrag erscheint nur bei Volumen-Nodes und wechselt zwischen „As amplifier" und
  „As passive optic", Statuszeile ohne Kantenversatz, Undo/Redo dreht Amp-Zustand *und* Statuszeile
  zurück, globales Panel bleibt bei Add/Delete/Undo/Redo aktuell) — GUI-Tests existieren in diesem
  Crate praktisch nicht.
- Physikalisch muss nach der gesamten Vorbereitungsstufe **alles unverändert** sein: `amp config`
  steht überall auf `None`.
