# Plan: 3D-Netze für Optiken mit Volumen

## Worum es geht

Opossum kann heute keine Optik als 3D-Körper herausgeben. Ein Modell ist ein Graph; wo eine Linse liegt und
welche Form sie hat, steckt verteilt in Flächen (`GeoSurface`), einer Clear Aperture und einer Node-Position, die
es überhaupt erst nach einem Positionierungsrun gibt. Wer das Setup ansehen will, hat nur 2D-Plots im Report.

Dieses Paket baut den Weg von dort zu einer GLB-Datei: Für jede Optik mit Volumen (Linse, Zylinderlinse, Keil)
wird ein geschlossenes Dreiecksnetz mit Normalen berechnet, zusammen mit einem Glas-Material an `optoscene`
gegeben und über einen neuen Backend-Endpoint ausgeliefert. Man kann die Datei dann im three.js-Editor öffnen.

Die Rechnung wird so geschnitten, dass eine **einzelne Fläche über einer Clear Aperture** für sich vernetzt werden
kann. Das ist der Haken, an dem später Detektoren als Fläche hängen. Ebenso wird das Netz im Koordinatensystem der
Node berechnet und die Position getrennt übergeben — damit bei späteren Live-Updates nur Positionen neu gesendet
werden müssen, nicht die Netze.

## Entscheidungen

- **Umfang:** Core, Backend-Adapter, GLB-Endpoint. GUI-Anzeige, Strahlen und Live-Updates kommen später.
- **Commits:** Schritte 0–6 (alles in `opossum_core`) werden committet.
- **Workspace-Einbindung von optoscene** (Schritt 7): `optoscene/` bleibt unverändert ein eigener Workspace.
  Im Opossum-Root steht nur `exclude = ["optoscene"]` — ohne das hält Cargo die optoscene-Crates für Members
  dieses Workspaces und löst ihre geerbten Abhängigkeiten gegen den falschen Root auf. `opossum_backend` bekommt
  eine reine Pfad-Abhängigkeit. Damit beschränkt sich die Änderung auf **eine Zeile pro Datei** und lässt sich
  später mit einem Einzeiler auf eine Git-Abhängigkeit umstellen. Der Core hängt nicht von `optoscene` ab, nur
  `opossum_backend`.
- **Was committet werden kann:** `exclude = ["optoscene"]` schon jetzt — Cargo ignoriert einen `exclude`-Pfad,
  den es nicht gibt. Die Pfad-Abhängigkeit in `opossum_backend/Cargo.toml` nicht: Cargo muss das Verzeichnis
  beim Auflösen lesen, auch bei `optional = true`, also scheitert jeder frische Clone daran. Sie bleibt samt
  `scene_export.rs`, dem Endpoint und der `Cargo.lock`-Änderung im Arbeitsbaum, bis `optoscene` ein eigenes
  Repository hat.
- **Brechungsindex** für die Darstellung bei 1053 nm. Deckt das Dispersionsmodell die Wellenlänge nicht ab, wird
  n = 1.5 verwendet und eine Warnung geloggt.
- **Triangulierung** der Apertur mit `spade::ConstrainedDelaunayTriangulation` (schon Abhängigkeit des Core,
  2.15.1). Funktioniert für Kreis, Rechteck und jedes nicht selbstschneidende Polygon, auch nicht konvexe.
- **Positionierungsrun** vor dem Export, auf einer Kopie. Quellenwahl:
  - Mit Analyzer-ID: dieser Analyzer.
  - Ohne ID, genau ein Ray-Trace- oder Ghost-Focus-Analyzer: dieser.
  - Ohne ID, mehrere solche Analyzer: Fehler.
  - Kein solcher Analyzer: `RayDataBuilder::default()` für jeden Source Port, Abstand 0 zur ersten Optik.

## Wichtige Randbedingungen aus dem Code

Geprüft gegen den Stand vor diesem Paket. Jeder Punkt hat Folgen für die Umsetzung.

**Positionen**

- Nodes haben erst nach einem Positionierungsrun eine Position. Der Run speichert sie in der Node
  (`NodeAttr.isometry`) und sie wird mit dem Dokument gespeichert. Ein späterer Run rechnet Nodes, die schon eine
  Position haben, nicht neu (`calculate_single_node_position`, `nodes/node_group/analysis_raytrace.rs`). Nichts
  setzt sie zurück: `reset_data()` und `clear_edges()` lassen sie stehen. Der Positionierungsrun für den Export
  darf deshalb nicht auf dem Dokument im Backend laufen, sondern nur auf einer Kopie.
- `SourcePort::default()` setzt `Isometry::identity()`. Quellen gelten damit immer als „schon platziert“ — die
  erste Optik landet ohne Analyzer im Ursprung.
- Node-Referenzen werden nicht gezeichnet. Eine Referenz hat keine eigene Position
  (`NodeReference::isometry()` gibt die der referenzierten Node zurück); sie ist ein weiterer Durchgang durch
  dieselbe Optik. Nodes, die der Run nicht platzieren kann, werden mit einer Warnung übersprungen und nicht im
  Ursprung gezeichnet.

**Kopie des Dokuments**

- `OpmDocument::clone()` ist keine Kopie: `OpticRef` ist ein `Arc`, der Klon teilt die Nodes mit dem Original.
- `OpmDocument::clone_deep()` existiert, taugt hier aber nicht: `OpticGraph::clone_deep` löst `NodeReference`s
  nicht neu auf, deren `Weak`s zeigen weiter auf die **Originalnodes**.
- Eine echte Kopie entsteht über `to_opm_file_string()` + `from_string()` — nur `from_string()` ruft
  `resolve_all_references()`. So macht es `with_rollback` in `opossum_backend/src/document.rs` schon.

**Quellen und Analyzer**

- Eine Quelle trägt ihren `RayDataBuilder` **nicht** selbst. Er liegt nur in
  `RayTraceConfig::source_map: HashMap<Uuid, RayDataBuilder>`. Source-Nodes haben den Typ `"source port"`;
  `NodeGroup::find_source_ports() -> OpmResult<Vec<Uuid>>` findet sie rekursiv.
- Eine Quelle kann nur positioniert werden, wenn der Analyzer Quelldaten für sie hat
  (`SourcePort::calc_node_positions`). Die Wellenlänge des Achsenstrahls beeinflusst die Positionen hinter Linsen.

**Geometrie**

- `Aperture` ist ein Struct (`shape: ApertureShape`, `a_type`, `isometry: Option<Isometry>`); die
  Fallunterscheidung läuft über `ApertureShape`. Als Clear Aperture sind nur Kreis, Rechteck und Polygon erlaubt
  (`ApertureShape::is_binary()`).
- `cross_section_outline` (`geometry/body/mod.rs`) liefert ausdrücklich **Extrempunkte für die Bounding Box**,
  keine Randkurve. Der Rand muss neu gebaut werden.
- Die Earcut-Triangulierung in `PolygonShape` ist privat, erzeugt keine inneren Punkte und gilt nur für Polygone.
- `ellipse` (`utils/math_distribution_functions.rs`) liefert `Vec<Point2<f64>>` und nimmt `num_points: u32` —
  einheitenlos, Umrechnung über `.value` / `meter!`.
- `curved_local_z` und `is_behind_curvature` sind `pub(super)` in `crate::geometry`.
- Die Felder von `SurfaceBoundedBody` sind privat für `geometry::body`. Eine Methode darauf muss in
  `geometry/body/` liegen.
- Eine Node mit einer einzigen Fläche gibt denselben `GeoSurfaceRef` zweimal heraus (dokumentiert in
  `geometry/body/mod.rs`). Nie zwei Guards gleichzeitig halten.
- `Body::bounding_box()` liefert Bereiche im Koordinatensystem des Bodys; `z_range()` ist die Dicke fürs
  Glasmaterial.

**spade 2.15.1**

- `try_bulk_load_cdt(vertices, edges, on_conflict_found)` **dedupliziert** die Eingabepunkte. Nur ohne Duplikate
  gilt „Eingabeindex == `FixedVertexHandle`-Index“; danach behalten die Randpunkte die Indizes `0..n` und `refine`
  hängt Steiner-Punkte hinten an.
- `FaceHandle::vertices()` gibt die drei Ecken gegen den Uhrzeigersinn zurück — die Dreiecksorientierung kommt
  geschenkt.
- `refine` liefert `RefinementResult { excluded_faces, refinement_complete }`; Parameter über
  `with_max_allowed_area`, `with_max_additional_vertices`, `keep_constraint_edges()` (ohne Argument) und
  `exclude_outer_faces(bool)`. Der Vertex-Typ braucht `HasPosition + From<Point2<Scalar>>`.
- spade sagt zu, dass sich der Verfeinerungsablauf in jedem Patch-Release ändern darf. Determinismus gilt also je
  Version — Tests prüfen „zweimal derselbe Aufruf“, keine eingefrorenen Sollwerte.

**Backend**

- Es gibt keine zentrale utoipa-`paths(...)`-Liste; `utoipa_actix_web` sammelt die Pfade aus `cfg.service(...)`.
- `opossum_backend` hat weder `serde_json` noch `[dev-dependencies]`.
- In `opossum_core` gibt es keine Referenzwellenlängen-Konstante (`WAVELENGTH_D_LINE_NM` liegt in
  `opossum_registry`, von dem der Core nicht abhängt).

## Was schon existiert und genutzt wird

| Vorhandenes | Ort | Wofür |
|---|---|---|
| `Volumetric::volume_body()` | `core_optics/volumetric.rs` | liefert `SurfaceBoundedBody` für jede Volumen-Node — kein Code pro Node-Typ |
| `GeoSurface::local_z_at` | `geometry/geo_surface.rs` | Punkt einer Fläche über einer Stelle |
| `SurfaceBoundedBody::surface_z_range` | `geometry/body/mod.rs` | enthält die Umrechnung Fläche → Body-Frame samt Prüfungen |
| `curved_local_z` | `geometry/geo_surface.rs` | gemeinsame Sagitta von Sphere und Cylinder |
| `ellipse` | `utils/math_distribution_functions.rs` | Punkte auf einem Kreis |
| `Aperture::apodize` | `apertures/mod.rs` | „liegt der Punkt in der Apertur“ — für Tests |
| `Body::contains` | `geometry/body/mod.rs` | Normalenrichtung prüfen |
| `NodeGroup::collect_all_nodes_recursive()` | `nodes/node_group/mod.rs` | alle Nodes, auch aus verschachtelten Gruppen |
| `NodeGroup::find_source_ports()` | `nodes/node_group/mod.rs` | alle Source-Port-UUIDs, rekursiv |
| `RayTraceConfig::for_positioning()` / `map_source()` | `analyzers/raytrace.rs` | Positionierungs-Konfiguration |
| `AnalysisRayTrace::calc_node_positions` | `analyzers/raytrace.rs` | der Positionierungsrun |
| `OpmDocument::to_opm_file_string` / `from_string` | `opm_document.rs` | echte Kopie inkl. Referenzauflösung |
| `node_types()` / `create_node_ref()` | `nodes/mod.rs` | Registry-Rundlauf über alle Node-Typen |
| `Isometry::get_transform()` | `utils/geom_transformation.rs` | `nalgebra::Isometry3<f64>` in Metern, direkt für optoscene |
| `glb_json` | `optoscene/crates/optoscene/tests/export_features.rs` | JSON-Chunk eines GLB auslesen |

## Vermerk: Die Kopie des Dokuments muss langfristig anders gelöst werden

Der String-Umweg ist eine Übergangslösung. Dahinter steckt ein bestehendes Problem: berechnete und vom Nutzer
gesetzte Positionen liegen im selben Feld `NodeAttr.isometry`, deshalb lässt sich die Positionierung nicht
zurücksetzen. Langfristig bräuchte es entweder eine echte tiefe Kopie oder getrennt abgelegte, nicht gespeicherte
Laufzeit-Positionen.

Solange das offen ist, gilt für den Export: Stehen im Backend-Dokument schon berechnete Positionen, werden sie
mitkopiert und nicht neu berechnet — der Export zeigt dann veraltete Positionen.

## Schritte

Jeder Schritt ist ein Commit.

### Schritt 0: Plan im Repo ablegen

Diese Datei ins Repo-Root, wie `optoscene_plan.md` es vorlebt. Kein Code.

### Schritt 1: Randpunkte einer Apertur

**Warum:** Der Rand begrenzt die Triangulierung (Schritt 3) und ist die Kante der Randfläche eines Volumens.
Bisher gibt es nur Extrempunkte für die Bounding Box, und Rechteck-Ecken werden an zwei Stellen ad hoc
nachgerechnet.

**Was:** `Aperture::outline_points(segments) -> OpmResult<Vec<Point2<Length>>>` in `apertures/mod.rs`: der
geschlossene Rand als Polygon, gegen den Uhrzeigersinn, ohne Wiederholung des ersten Punkts, mit angewandter
Isometrie der Apertur.

- Kreis: `segments` Punkte über `ellipse`.
- Rechteck: die vier Ecken plus Zwischenpunkte auf den Seiten, nach Seitenlänge verteilt, insgesamt etwa
  `segments`. Die Ecken sind immer dabei.
- Polygon: die Eckpunkte plus Zwischenpunkte wie beim Rechteck. Im Uhrzeigersinn angegebene Polygone werden
  umgedreht (Vorzeichen der Shoelace-Fläche).
- `Open`, `Gaussian`, `Stack`: Fehler. Bei einer Clear Aperture kommt das nicht vor.

Der Formcode kommt je als Methode an `CircleShape`, `RectangleShape`, `PolygonShape`.

**Tests:** Kreispunkte liegen auf dem Radius. Rechteck und Polygon enthalten alle Ecken, alle Punkte liegen auf
dem Rand. Die Polygonfläche ist positiv, auch bei im Uhrzeigersinn angegebener Eingabe. Verschiebung und Drehung
der Apertur werden angewendet. `segments < 3` gibt einen Fehler.

### Schritt 2: Normale einer Fläche

**Warum:** Die Normalen sollen exakt aus der Flächenform kommen. Über `calc_intersect_and_normal` geht das nicht
zuverlässig: Die Trait-Doku garantiert nur „gegen die Strahlrichtung“, nicht eine feste geometrische Orientierung;
man bräuchte einen passenden Startstrahl, und `Parabola::calc_intersect_and_normal_do` hält die Zusage ohnehin
nicht ein. Genau darum gibt es `local_z_at`.

**Was:** Neue **Pflichtmethode** `GeoSurface::local_normal_at(&Point2<Length>) -> Option<Vector3<f64>>` neben
`local_z_at` — Länge 1, lokales Koordinatensystem, Richtung lokales +z, `None` wo auch `local_z_at` `None` gibt.
Keine Default-Implementierung, damit jede künftige Fläche die Frage beantworten muss.

- `Plane`: `(0, 0, 1)`
- `Sphere`: `-(x, y, z) / R` mit z aus `local_z_at` (lokaler Ursprung ist der Krümmungsmittelpunkt)
- `Cylinder`: `-(x, 0, z) / R` (Achse entlang lokal y)
- `Parabola`: `(-x, -y, 2f) · sign(f)`, normiert

`Sphere` und `Cylinder` bekommen eine gemeinsame Hilfsfunktion neben `curved_local_z`.

**Tests:** Je Fläche an mehreren Stellen, auch nahe am Rand: Länge 1, z-Anteil positiv, senkrecht auf kleine
Schritte entlang der Fläche (aus `local_z_at`), bis aufs Vorzeichen gleich der Normale aus
`calc_intersect_and_normal_do`. Außerhalb des Radius `None`.

### Schritt 3: Dreiecksnetz der Apertur in der Ebene

**Warum:** Gekrümmte Flächen brauchen Punkte im Inneren, sonst folgt das Netz der Krümmung nicht. Die Dreiecke
müssen für jede erlaubte Clear Aperture stimmen, auch für nicht konvexe Polygone.

**Was:** Neuer Typ `ApertureMesh` (Punkte `Point2<Length>`, Dreiecke `[u32; 3]` gegen den Uhrzeigersinn, Indizes
der Randpunkte in Randreihenfolge; Felder privat, Zugriff über Methoden) in neuer Datei `apertures/mesh.rs`, dazu
`Aperture::triangulate(segments) -> OpmResult<ApertureMesh>`.

1. Randpunkte über `outline_points(segments)`; doppelte Punkte ablehnen (sonst verschiebt spades Deduplizierung
   die Indizes).
2. `try_bulk_load_cdt` mit den Randkanten als Zwangskanten. Meldet der `on_conflict_found`-Callback etwas,
   schneidet sich der Rand → Fehler.
3. `refine` mit `with_max_allowed_area` = Fläche eines gleichseitigen Dreiecks mit dem mittleren Abstand
   benachbarter Randpunkte als Seitenlänge, dazu `keep_constraint_edges()` (damit der Rand genau die Randpunkte
   bleibt), `exclude_outer_faces(true)` und `with_max_additional_vertices`. Ist `refinement_complete == false`,
   Fehler.
4. Alle Dreiecke übernehmen, die nicht in `excluded_faces` stehen.

**Tests:** Kreis, Rechteck, ein L-förmiges Polygon und ein fünfzackiger Stern — Summe der Dreiecksflächen gleich
der Aperturfläche, Schwerpunkt jedes Dreiecks in der Apertur (`apodize > 0`). Kein Dreieck größer als erlaubt.
Die Randindizes ergeben genau `outline_points`, in derselben Reihenfolge. Ein selbstschneidendes Polygon gibt
einen Fehler. Zweimal dieselbe Eingabe ergibt genau dasselbe Netz (wichtig, damit gleiche Optiken im Export ein
gemeinsames Mesh bekommen).

### Schritt 4: Netz einer einzelnen Fläche

**Warum:** Eine Linse braucht das zweimal, Vorder- und Rückseite. Ein Detektor mit Clear Aperture braucht es
später einmal — das ist die Vorbereitung, Detektoren als Fläche darzustellen.

**Was:** Neue Datei `geometry/mesh.rs` mit `SurfaceMesh` (Punkte `Point3<Length>`, Normalen `Vector3<f64>`,
Dreiecke `[u32; 3]`; Felder privat) und `SurfaceMesh::flipped()` (dreht Normalen und Dreiecksreihenfolge um).
Dazu `GeoSurfaceRef::mesh_over(aperture_mesh, frame) -> OpmResult<SurfaceMesh>`, das das ebene Netz aus Schritt 3
im Koordinatensystem `frame` (der Node-Position) auf die Fläche legt:

1. Je Punkt des ebenen Netzes Punkt und Normale über `local_z_at` / `local_normal_at` bestimmen und ins
   Node-Koordinatensystem umrechnen. **Diese Umrechnung samt Prüfungen wird aus `surface_z_range` in eine
   gemeinsame Funktion gezogen; `surface_z_range` nutzt danach dieselbe.** Kern ist
   `Isometry::new_from_transform(body_inv * surface_transform)`.
2. Dreiecke aus dem ebenen Netz übernehmen.
3. Normalen zeigen nach +z der Node, Dreiecke von +z aus gesehen gegen den Uhrzeigersinn.

Fehler, wenn die Fläche nicht bis zum Rand reicht oder gekrümmt **und** gegen die Node gekippt ist — dieselbe
Grenze wie bei der Bounding Box.

**Tests:** Alle Punkte liegen auf der Fläche, alle Normalen haben Länge 1 und zeigen nach +z, das Kreuzprodukt
zweier Dreieckskanten zeigt in Normalenrichtung. Verschiebt man die Node, bleibt das Netz gleich (es wird im
Node-Frame gerechnet). Beide Fehlerfälle werden ausgelöst.

### Schritt 5: Geschlossenes Netz eines Volumens

**Warum:** Das ist das eigentliche Netz für Linse, Zylinderlinse und Keil. Jede künftige Volumen-Node bekommt es
automatisch, weil alle über `volume_body()` gehen.

**Was:** Neues Kindmodul `geometry/body/mesh.rs` (dort, weil die Felder von `SurfaceBoundedBody` privat für
`geometry::body` sind) mit `BodyMesh { entrance, exit, edge }` und
`SurfaceBoundedBody::triangulate(segments) -> OpmResult<BodyMesh>`. Das ebene Netz der Clear Aperture wird
**einmal** berechnet und für beide Flächen verwendet.

- `entrance`: Netz der Eintrittsfläche, umgedreht (`flipped()`), weil die Außenseite nach −z zeigt.
- `exit`: Netz der Austrittsfläche.
- `edge`: die Randfläche dazwischen, aus den Randpunkten des ebenen Netzes. Je zwei benachbarte Randpunkte
  ergeben zwei Dreiecke. Die Punkte kommen aus derselben Funktion wie in Schritt 4, dadurch passen die Kanten
  exakt aufeinander.
- Normalen der Randfläche zeigen in der xy-Ebene nach außen. Knickt der Rand um weniger als 30°, werden die
  Normalen der beiden angrenzenden Abschnitte gemittelt (damit ein Kreis rund aussieht); bei stärkeren Knicken
  wird der Punkt verdoppelt (damit Ecken scharf bleiben).

Fehler, wenn die Austrittsfläche an irgendeinem Punkt des ebenen Netzes vor der Eintrittsfläche liegt — dann
schneiden sich die Flächen innerhalb der Apertur, etwa bei einer Linse mit zu dünnem Rand. Weil beide Flächen
dieselben Punkte haben, wird das an jedem Punkt geprüft.

**Tests:** Bikonvexe, Meniskus- und plankonvexe Linse, Zylinderlinse, Keil und eine Linse mit L-förmiger Clear
Aperture, je auch verschoben und gedreht:
- Lage und Richtung der Normalen über `Body::contains`: ein Punkt kurz innerhalb der Fläche (entgegen der
  Normale) liegt im Körper, einer kurz außerhalb nicht.
- Geschlossenheit: nach Zusammenfassen gleicher Punkte gehört jede Kante zu genau zwei Dreiecken.
- Zu dünner Rand gibt einen Fehler.
- Jede Node-Art aus `node_types()` mit Volumen lässt sich im Default-Zustand vernetzen — Vorbild ist
  `the_volume_capability_matches_the_volume_properties` in `volumetric.rs`.

### Schritt 6: Positionierte Kopie des Dokuments

**Warum:** Ohne Positionierungsrun haben die Nodes keine Position. Der Run darf das Dokument im Backend nicht
verändern, sonst würden die Positionen gespeichert und später nicht mehr neu berechnet.

**Was:** `OpmDocument::positioned_copy(analyzer_id: Option<Uuid>) -> OpmResult<OpmDocument>`:

1. Kopie über `to_opm_file_string()` + `from_string()`.
2. Quellen wählen:
   - Mit ID: dieser Analyzer; ist er weder Ray Trace noch Ghost Focus → Fehler.
   - Ohne ID, genau ein Ray-Trace-/Ghost-Focus-Analyzer: dieser.
   - Ohne ID, mehrere: Fehler mit dem Hinweis, dass eine ID nötig ist.
   - Keiner: für jede UUID aus `find_source_ports()` ein `RayDataBuilder::default()` in einen frischen
     `RayTraceConfig` (`map_source`). Hat das Modell keinen Source Port, wird in der Kopie einer angelegt und mit
     Abstand 0 an die erste Optik angeschlossen — die erste Node aus `scenery().nodes()`, die kein Source Port ist
     und deren Eingang unverbunden ist.
3. Positionierungsrun auf der Kopie mit genau der Konfiguration, die der Analyzer selbst verwendet, danach
   `reset_data()`. Dafür kommt `AnalyzerType::positioning_config() -> Option<RayTraceConfig>` dazu: Ray Trace →
   `config.for_positioning()`; Ghost Focus → was `GhostFocusAnalyzer::analyze` heute selbst baut
   (`RayTraceConfig::default()` + `set_source_map` + `set_positioning_run(true)`); Energy → `None`.
   **`GhostFocusAnalyzer` nutzt danach diese Methode**, damit die 3D-Ansicht dieselben Positionen zeigt wie die
   Analyse und die beiden nicht auseinanderlaufen.

**Tests:** Das Original hat nach dem Aufruf keine gesetzten Positionen, die Kopie schon. Mit Analyzer entsprechen
die Positionen den Abständen der Verbindungen. Mehrere Analyzer ohne ID geben einen Fehler. Ohne Analyzer und ohne
Quelle liegt die erste Optik im Ursprung. Ohne Analyzer, aber mit Quelle, wird von dieser aus positioniert.

---

*Ab hier hängt der Code an `optoscene` — siehe „Was committet werden kann” unter „Entscheidungen”.*

### Schritt 7: optoscene einbinden und die Szene bauen

**Manifeste:** `optoscene/` selbst wird nicht angefasst.
- Root-`Cargo.toml`: `exclude = ["optoscene"]`.
- `opossum_backend/Cargo.toml`: `optoscene = { path = "../optoscene/crates/optoscene" }` und `serde_json` als
  Dev-Dependency für den GLB-Test.

**Code:**
- Core: `material::default_reference_wavelength()` → 1053 nm.
- Neues Backend-Modul `opossum_backend/src/scene_export.rs` mit
  `volume_scene(document, analyzer_id, wavelength) -> Result<optoscene::Scene>`:
  1. `document.positioned_copy(analyzer_id)`.
  2. Über `collect_all_nodes_recursive()` laufen; Nodes ohne Volumen überspringen, Nodes ohne Position mit
     Warnung überspringen.
  3. Je Volumen-Node in einer eigenen Funktion `add_volume_node(scene, node)` — eigene Funktion, damit später
     eine Ansicht mit nur einer Optik sie nutzen kann:
     - `volume_body()` + `triangulate(64)`, die drei Teilnetze als `SurfacePatch` (Punkte in Metern).
     - Material `Glass { color: [0.9, 0.95, 1.0], ior, thickness }`; `ior` aus `Material::refractive_index` bei
       der Wellenlänge, sonst 1.5 mit Warnung; `thickness` aus `bounding_box()?.z_range()`.
     - `SceneNode` mit `uid` = UUID der Node, Name der Node, `transform = Isometry::get_transform()`,
       `layer: Layer::Optics`.
  4. Schlägt eine einzelne Node fehl: Warnung mit ihrem Namen, weiter mit der nächsten.

**Tests:** Ein Dokument mit Source Port, Linse, Keil, Zylinderlinse, einer Referenz und einem Detektor, mit
Abständen verbunden, plus Ray-Trace-Analyzer: Die Szene enthält genau drei Nodes. Zwei gleiche Linsen ergeben im
GLB nur ein Mesh (`Scene::add_mesh` dedupliziert inhaltsbasiert). Die Materialien enthalten `KHR_materials_ior`.
Das Dokument im Backend ist danach unverändert. Zum Auslesen des GLB dient `glb_json` als Vorbild.

### Schritt 8: Endpoint

`GET /api/document/scene.glb` in `opossum_backend/src/document.rs`, optionaler Query-Parameter `analyzer` (UUID).
Ruft `volume_scene` mit `default_reference_wavelength()` auf und gibt `to_glb()` mit Content-Type
`model/gltf-binary` zurück. Fehler aus der Analyzer-Wahl werden 400 mit Text. Registrierung: **eine** Zeile
`cfg.service(get_scene_glb);` in `document::config` — `utoipa_actix_web` sammelt den Pfad aus der
`#[utoipa::path]`-Annotation, `server.rs` wird nicht angefasst. Vorbild für Content-Type-Handling ist
`get_document`, Body als `Vec<u8>` über `HttpResponse::Ok().content_type(...).body(...)`.

**Tests:** 200, Content-Type stimmt, die Daten beginnen mit `glTF`. Mehrere Analyzer ohne Parameter geben 400. Ein
leeres Dokument gibt eine gültige, leere GLB-Datei. Teststil wie `test_get_document_returns_ron_format`.

## Prüfung

- **Während eines Schritts:** gezielte Tests, z. B. `cargo test -p opossum_core local_normal_at`.
- **Am Ende des Pakets:** `cargo fmt -p opossum_core -p opossum_backend` (ab Schritt 7 zusätzlich
  `-p optoscene -p optoscene-protocol`) — nie ohne `-p`, sonst formatiert es die Dioxus-Makros in der GUI um.
  Danach Clippy mit den CI-Lints (`-D warnings -W clippy::pedantic -W clippy::nursery -W rust-2018-idioms`) und
  die Ausgabe **nur für die geänderten Dateien** auswerten; lokal stehen viele alte Warnungen drin. Danach
  `cargo test -p opossum_core`, ab Schritt 7 dazu `cargo test -p optoscene` und `cargo test -p opossum_backend`.
- **Von Hand (nach Schritt 8):** Backend starten (`cargo run -p opossum_backend`), ein `.opm` mit Linse, Keil und
  Zylinderlinse laden, `http://localhost:8001/api/document/scene.glb` herunterladen und im three.js-Editor sowie
  im Khronos glTF Validator öffnen. Erwartet: keine Fehler im Validator, geschlossene Glaskörper an den richtigen
  Positionen, Normalen nach außen.

## Grenzen

- Selbstschneidende Polygone geben einen Fehler. `PolygonShape` sieht solche ohnehin nicht vor.
- Flächen, die gekrümmt **und** gegen die Node gekippt sind, geben einen Fehler — dieselbe Grenze wie bei der
  Bounding Box, und keine heutige Node erzeugt solche Flächen.

## Danach (nicht Teil dieses Plans)

- **Handbuch** (`doc/book`): eigener Plan für die Doku zum 3D-Export.
- **Detektoren als Fläche:** Detektor-Nodes bekommen eine Clear-Aperture-Property; das Lesen der Clear Aperture
  (heute die private `cross_section` in `volumetric.rs`, nur für Volumen-Nodes) wird für alle Nodes verfügbar;
  der Export bekommt einen zweiten Fall „Node ohne Volumen, aber mit Clear Aperture“ über `triangulate`
  (Schritt 3) und `mesh_over` (Schritt 4). Dabei muss
  `the_volume_capability_matches_the_volume_properties` angepasst werden: es verlangt heute, dass genau die
  Volumen-Nodes eine Clear Aperture haben; künftig gilt nur noch, dass jede Volumen-Node eine hat.
- **Kopie des Dokuments:** siehe Vermerk oben.
- **Live-Updates:** Netze im Node-Frame, Position getrennt — ändern sich nur Positionen, müssen nur diese neu
  gesendet werden.

## Nebenbefunde (nicht Teil dieses Plans)

- **Positionen zwischen Analyzern:** In `OpmDocument::analyze` bekommt jeder Analyzer einen eigenen
  Positionierungsrun, aber die Positionen des ersten bleiben stehen. Alle weiteren Analyzer verwenden also die
  Positionen des ersten, auch bei anderer Quelle, Richtung oder Wellenlänge. Sehr wahrscheinlich ein Fehler;
  gehört zusammen mit dem Vermerk zur Dokumentkopie behoben.
- `simulate` in `opossum_backend/src/document.rs` analysiert `data.document.lock().clone()` — ein flacher Klon,
  der die Nodes mit dem Live-Dokument teilt, schreibt also Positionen dorthin zurück. Aktuell schläft das
  Problem: der Handler ist nicht registriert (`// cfg.service(simulate);`).
- `PolygonShape::add_points` / `delete_point` ändern die Punkte, rechnen aber `triangle_indices` nicht neu —
  danach ist `in_polygon` falsch.
- `curved_local_z` gibt **exakt** am Rand (`d == |R|`) `None` zurück, obwohl der Punkt noch zur Fläche gehört.
  Das `mul_add` in `radius.mul_add(radius, -(d*d))` rundet anders als die getrennte Multiplikation daneben, das
  Ergebnis landet eine Ulp unter null. Folge für Schritt 4/5: Eine Clear Aperture, deren Radius genau dem
  Krümmungsradius entspricht (Halbkugel), lässt sich am Rand nicht vernetzen. `surface_z_range` verhält sich heute
  schon genauso („does not reach as far out as the cross section"), der Fall ist also nicht neu.
- `Parabola::calc_intersect_and_normal_do` dreht die Normale nicht zum Strahl, obwohl die Trait-Doku das verlangt.
- `Sphere::new` und `Sphere::set_isometry` behandeln die Isometrie unterschiedlich (`set_isometry` hängt
  (0, 0, R) an, `new` nicht). Dasselbe bei `Cylinder`.
- Beim Plotten einer Polygon-Apertur wird deren Isometrie ignoriert.
