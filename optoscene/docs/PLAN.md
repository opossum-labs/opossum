# optoscene – Umsetzungsplan v3

glTF/GLB-Pipeline in Rust für die 3D-Darstellung optischer Systeme mit three.js. Das Crate ist eigenständig, transportagnostisch und hat möglichst wenige Abhängigkeiten. Opossum bindet es später über einen dünnen Adapter ein.

**Abgrenzung.** `optoscene` ist ein Export-Crate, kein Geometrie-Crate. Es erzeugt glTF/GLB aus fertig vorliegender Geometrie und macht das für optische Szenen bequem. Es berechnet keine Geometrie aus physikalischen Größen: Tessellierung, Normalen und die echte Strahlform liefert der Aufrufer, weil nur dort Parametrisierung, Aperturen und Vignettierung bekannt sind. Die einzige Ausnahme ist eine bewusst grobe Default-Hülle (Abschnitt 6.3), falls jemand nur Ray-Linien hat und trotzdem ein 3D-Objekt sehen will.

## Nutzung dieses Plans mit Claude Code

1. Diese Datei als `docs/PLAN.md` ins neue Repo legen.
2. Den Regelblock aus Abschnitt 1 als `CLAUDE.md` ins Repo-Root kopieren.
3. Pro Phase eine eigene Session. Ablauf: Im Plan Mode die Phase durchplanen lassen, dann mit Sonnet umsetzen. Beispiel-Prompt:

   > Read docs/PLAN.md and CLAUDE.md. Implement Phase 2 exactly as specified in section 9. Do not start Phase 3. When done, run all checks listed in the acceptance criteria and report: what was implemented, any deviations from the plan and why, open questions.

4. Jede Phase ist so geschnitten, dass sie für sich kompiliert, getestet ist und in einen PR passt.
5. Wo der Plan „prüfen“ sagt (Crate-Versionen, externe APIs): auf docs.rs bzw. crates.io nachsehen, gerne per Sub-Agent. APIs werden nicht geraten.

## 0. Kontext

- **Zweck:** Optiken als echte 3D-Volumen anzeigen: in der Node-Konfiguration (eine Optik) und in der Szenerie nach jedem Positionierungsrun (ganzes System). Dazu Ray-Bundles als Linien und, falls der Aufrufer eine liefert, als Hüllfläche.
- **Kein Rendering während der eigentlichen Simulation.** Die Pipeline läuft nur nach Konfigurationsänderungen bzw. Positionierungsruns.
- **Ausgabewege:** Datei (`.glb` oder `.gltf` + `.bin`) und Datenstrom (Binär-Frames mit GLB-Payload bzw. kleinen Deltas).
- **Transport:** Ist nicht Teil der Bibliothek. Der Kern liefert `Vec<u8>`; wer ihn einbindet, verschickt die Bytes selbst (bei Opossum über das bestehende actix-Backend an das Dioxus-Frontend).
- **Renderer:** three.js mit `GLTFLoader`. Node-Extras landen dort automatisch in `object.userData`.

## 1. Globale Regeln (Inhalt für `CLAUDE.md`)

```markdown
# Rules for this repository

- All code, identifiers, doc comments, README, CHANGELOG, error messages,
  test names and commit messages are written in English.
- Every public item has a doc comment. Each crate root contains
  `#![deny(missing_docs)]` and `#![forbid(unsafe_code)]`.
- Library code must not panic on user input: no `unwrap`/`expect`/indexing
  that can fail outside of tests. Return `Result` with the crate's error type.
- Do not add dependencies beyond those listed in docs/PLAN.md section 2
  without asking first. Disable default features unless they are needed.
- Do not guess external APIs. Verify signatures on docs.rs for the exact
  version in Cargo.toml.
- Binary data is written little-endian via `to_le_bytes` (glTF requirement).
  No `bytemuck`, no `transmute`.
- Output must be deterministic: identical input produces identical bytes.
  Never iterate a `HashMap` when producing output; use insertion order.
- Before finishing a phase, all of these must pass without warnings:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `cargo test --workspace --all-features`
  - `cargo doc --workspace --no-deps --all-features`
- Implement only the requested phase. Stop afterwards and report what was
  done, deviations from the plan, and open questions.
```

## 2. Abhängigkeiten

**`optoscene` (Kern)**

| Crate | Konfiguration | Zweck |
|---|---|---|
| `nalgebra` | **exakt dieselbe Version wie im Opossum-Workspace** (prüfen), `default-features = false`, `features = ["std"]` | `Isometry3`, `Point3`, `UnitQuaternion`. Gleiche Version ist Pflicht, sonst sind die Typen inkompatibel |
| `serde` | `features = ["derive"]` | glTF-Schema, Extras |
| `serde_json` | Standard | JSON-Serialisierung, Extras |
| `optoscene-protocol` | optional, Feature `protocol` | Frame-Format für den Stream |

**`optoscene-protocol`:** nur `serde` (derive) und `serde_json`. Muss für `wasm32-unknown-unknown` kompilieren: kein `std::fs`, keine Threads.

**Dev-Dependencies Kern:** `gltf` (nur für Roundtrip-Tests: exportierte Dateien wieder einlesen). Features so aktivieren, wie die Tests es brauchen (z.B. `extras` und die `KHR_materials_*`-Features, um Werte prüfen zu können).

**Beispiel-Server** (eigener Crate, `publish = false`): `actix-web`, `actix-ws`, `tokio` (nur `sync`, `time`, `macros`). Diese Abhängigkeiten tauchen niemals im Kern auf.

**Bewusst nicht verwendet:**
- `gltf-json`: Stattdessen eigene, minimale Serde-Structs für die benötigte Teilmenge von glTF 2.0. Das ist schlanker, gibt volle Kontrolle über Extensions und ist robust gegen API-Änderungen. Die Korrektheit sichern Roundtrip-Tests mit dem `gltf`-Reader und der Khronos-Validator ab.
- `bytemuck`: `to_le_bytes` reicht und ist auch auf Big-Endian-Plattformen korrekt.
- `thiserror`: Der Fehlertyp wird von Hand implementiert (wenige Varianten).
- Hash-Crates: Für Content-Hashes reicht `std::hash::DefaultHasher`, weil Hashes nur innerhalb eines Prozesses verglichen werden.
- axum, tokio o.ä. im Kern: Transport ist Sache der Anwendung.

**Werkzeug:** Khronos glTF-Validator (CLI-Binary aus den GitHub-Releases des Projekts `KhronosGroup/glTF-Validator` oder die Web-Version) für die manuellen Abnahmen.

## 3. Workspace-Struktur

```
optoscene/
├─ Cargo.toml                    workspace, resolver = "2"
├─ CLAUDE.md
├─ README.md
├─ docs/PLAN.md
├─ crates/
│  ├─ optoscene/
│  │  ├─ Cargo.toml              features: protocol, fixtures
│  │  ├─ src/lib.rs              lints, re-exports, crate docs
│  │  ├─ src/error.rs            Error enum
│  │  ├─ src/model.rs            public input types
│  │  ├─ src/scene.rs            Scene, SceneOptions
│  │  ├─ src/gltf/mod.rs
│  │  ├─ src/gltf/schema.rs      minimal serde structs of glTF 2.0
│  │  ├─ src/gltf/builder.rs     Scene -> (schema::Root, bin buffer)
│  │  ├─ src/gltf/container.rs   GLB container, .gltf + .bin
│  │  ├─ src/rays/mod.rs
│  │  ├─ src/rays/lines.rs       decimated ray lines
│  │  ├─ src/rays/hull.rs        default round envelope (fallback only)
│  │  ├─ src/rays/color.rs       wavelength -> RGB
│  │  ├─ src/diff.rs             feature "protocol"
│  │  ├─ src/fixtures.rs         cfg(any(test, feature = "fixtures"))
│  │  ├─ examples/export_basic.rs
│  │  ├─ examples/export_rays.rs
│  │  └─ tests/                  integration tests
│  └─ optoscene-protocol/
│     ├─ Cargo.toml
│     └─ src/lib.rs
└─ examples/
   └─ actix-viewer/              own crate, publish = false
      ├─ Cargo.toml
      ├─ src/main.rs
      └─ static/index.html, static/viewer.js
```

Features des Kerns: `default = []`, `protocol` (aktiviert `optoscene-protocol` und das Modul `diff`), `fixtures` (Testgeometrien auch für Beispiele und für Opossum-Tests verfügbar).

## 4. Öffentliche API

Alle Längen in **Metern**, alle Koordinaten `f64`. Die Umwandlung nach `f32` passiert erst im Exporter.

```rust
/// Identifier of a material registered in a [`Scene`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MaterialId(u32);

/// Identifier of a mesh registered in a [`Scene`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MeshId(u32);

/// Logical layer a node belongs to. Layers can be exported and replaced individually.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Layer { Optics, Rays, Aux }
// Layer names in glTF extras and protocol: "optics", "rays", "aux".

/// A triangulated surface patch with its own vertices and material.
/// Patches do not share vertices, so edges between patches stay sharp.
pub struct SurfacePatch {
    pub name: Option<String>,
    /// Vertex positions in the local frame of the component, in meters.
    pub positions: Vec<Point3<f64>>,
    /// Optional vertex normals, in the same frame as `positions`.
    /// Not generated: if `None`, no `NORMAL` attribute is written and
    /// viewers fall back to flat shading.
    pub normals: Option<Vec<Vector3<f64>>>,
    /// Optional linear RGBA vertex colors.
    pub colors: Option<Vec<[f32; 4]>>,
    pub indices: Vec<[u32; 3]>,
    pub material: MaterialId,
}

/// A mesh consisting of one or more surface patches.
pub struct TriMesh { pub patches: Vec<SurfacePatch> }

/// Material presets mapped to glTF PBR materials and extensions.
pub enum Material {
    /// Refractive material; `thickness` is the typical thickness in meters.
    Glass { color: [f32; 3], ior: f32, thickness: f32 },
    Mirror { color: [f32; 3], roughness: f32 },
    Opaque { color: [f32; 4], metallic: f32, roughness: f32 },
    /// Lighting-independent color, used for rays and helpers.
    Unlit { color: [f32; 4] },
    /// Semi-transparent, double-sided surface, used for bundle envelopes.
    Translucent { color: [f32; 4] },
}

/// A placed instance of a mesh.
pub struct SceneNode {
    /// Stable identifier, unique within the scene (e.g. Opossum node UUID).
    pub uid: String,
    pub name: String,
    pub mesh: Option<MeshId>,
    /// Transformation from the local mesh frame to world coordinates.
    pub transform: Isometry3<f64>,
    pub layer: Layer,
    /// Arbitrary metadata, exported to glTF extras.
    pub data: Option<serde_json::Value>,
}

/// Ray positions at consecutive surfaces ("stations").
/// `stations[k][i]` is ray `i` at station `k`; `None` marks a lost ray.
/// Rays travel in straight lines between stations.
pub struct RayTrace {
    pub uid: String,
    pub stations: Vec<Vec<Option<Point3<f64>>>>,
    /// Wavelength in meters, used for coloring.
    pub wavelength: Option<f64>,
}

/// Visual representation of a ray trace.
pub struct RayStyle {
    /// Maximum number of rays drawn as lines. 0 disables lines. Default 200.
    pub max_lines: usize,
    /// Line color; derived from the wavelength if `None`.
    pub line_color: Option<[f32; 4]>,
    /// Envelope surface of the bundle. Default `Envelope::None`.
    pub envelope: Envelope,
    /// Color used for `Envelope::DefaultRound`. Ignored for `Envelope::Mesh`,
    /// which carries its own materials. Default `[0.2, 0.6, 1.0, 0.25]`.
    pub envelope_color: [f32; 4],
}

/// Envelope surface of a ray bundle.
pub enum Envelope {
    /// No envelope surface.
    None,
    /// Envelope provided by the caller, in the same frame as the ray positions.
    /// This is the accurate option: only the caller knows apertures, clipping
    /// and the true bundle shape.
    Mesh(TriMesh),
    /// Crude round envelope derived from the ray lines, for callers that only
    /// have ray positions. Not a physically accurate beam shape.
    /// Defaults: sectors 32, steps 16.
    DefaultRound {
        /// Number of points per cross section ring.
        sectors: u32,
        /// Number of cross sections per segment between two stations.
        steps: u32,
    },
}

/// Global export options.
pub struct SceneOptions {
    pub name: String,
    /// Reference point subtracted from all world positions before conversion
    /// to f32. `None`: center of the world-space AABB of all nodes.
    pub origin: Option<Point3<f64>>,
    /// Rotation applied to the root node. Default identity.
    pub root_rotation: UnitQuaternion<f64>,
}

impl Scene {
    pub fn new(options: SceneOptions) -> Self;
    /// Registers a material; identical materials return the same id.
    pub fn add_material(&mut self, material: Material) -> MaterialId;
    /// Validates and registers a mesh; identical meshes return the same id.
    pub fn add_mesh(&mut self, mesh: TriMesh) -> Result<MeshId, Error>;
    /// Adds a node; fails on duplicate uid or unknown mesh.
    pub fn add_node(&mut self, node: SceneNode) -> Result<(), Error>;
    /// Adds line and envelope nodes for a ray trace to the `Rays` layer.
    pub fn add_ray_trace(&mut self, trace: &RayTrace, style: &RayStyle) -> Result<(), Error>;

    pub fn to_glb(&self) -> Result<Vec<u8>, Error>;
    pub fn write_glb<W: std::io::Write>(&self, writer: W) -> Result<(), Error>;
    /// Returns the JSON document and the binary buffer for a `.gltf` + `.bin` pair.
    pub fn to_gltf_separate(&self, bin_uri: &str) -> Result<(String, Vec<u8>), Error>;
    /// Writes `<stem>.gltf` and `<stem>.bin` into `dir`.
    pub fn write_gltf(&self, dir: &std::path::Path, stem: &str) -> Result<(), Error>;
    /// Exports only one layer (same root/layer structure).
    pub fn layer_to_glb(&self, layer: Layer) -> Result<Vec<u8>, Error>;
    /// Exports only the given nodes (same root/layer structure).
    pub fn nodes_to_glb(&self, uids: &[&str]) -> Result<Vec<u8>, Error>;
}
```

`Default` implementieren für `SceneOptions`, `RayStyle` und `Envelope` (`None`) mit den oben dokumentierten Werten. `Envelope::DefaultRound` bekommt eine `Envelope::default_round()`-Konstruktorfunktion mit den Standardwerten.

**Fehlertyp** (von Hand implementiert: `Display`, `std::error::Error`, `From<std::io::Error>`, `From<serde_json::Error>`):

```rust
pub enum Error {
    InvalidMesh(String),
    InvalidRayTrace(String),
    UnknownMaterial(MaterialId),
    UnknownMesh(MeshId),
    DuplicateUid(String),
    UnknownUid(String),
    Io(std::io::Error),
    Json(serde_json::Error),
}
```

**Mesh-Validierung in `add_mesh`:** mindestens ein Patch mit mindestens einem Dreieck; alle Indizes kleiner als die Vertexzahl; `normals`/`colors` haben, falls vorhanden, genauso viele Einträge wie `positions`; alle Werte endlich; Materialien registriert.

Das Crate verändert die Geometrie nicht: Positionen, Indizes und Normalen gehen unverändert nach glTF, abgesehen von der Ursprungsverschiebung der Nodes und dem `f64` → `f32`-Cast.

**Deduplizierung:** Materialien und Meshes werden über einen Content-Hash (`DefaultHasher`, `f32`/`f64` über `to_bits`) erkannt. Die Lookup-Tabelle darf eine `HashMap` sein, die Ausgabe folgt aber immer der Einfügereihenfolge.

## 5. glTF-Export-Spezifikation

### 5.1 Schema-Teilmenge

Eigene Serde-Structs in `gltf/schema.rs` mit `#[serde(rename_all = "camelCase")]`. Leere Felder werden weggelassen (`skip_serializing_if` für `Option::is_none` bzw. `Vec::is_empty`). Benötigt werden:

- `asset`: `version: "2.0"`, `generator: "optoscene <crate version>"`
- `scene`, `scenes`, `nodes`, `meshes`, `materials`, `accessors`, `bufferViews`, `buffers`
- `extensionsUsed` (nur die tatsächlich verwendeten; **niemals** `extensionsRequired`, damit andere Viewer mit Fallback laden)
- `extras` auf Nodes als `serde_json::Value`

### 5.2 Node-Hierarchie

```
optoscene_root          rotation = root_rotation
                        extras = { "origin": [x, y, z] }   (f64, Meter)
├─ layer:optics         extras = { "layer": "optics" }
│  ├─ <node.name>       mesh, translation, rotation
│  │                    extras = { "uid": ..., "layer": "optics", "data": ... }
│  └─ ...
├─ layer:rays
└─ layer:aux
```

- Layer-Gruppen werden nur angelegt, wenn der Layer Nodes enthält.
- Node-Translation = `(world_translation − origin)` als `f32`.
- Node-Rotation = `transform.rotation` als `[x, y, z, w]` `f32`, normiert. nalgebra speichert die Quaternion-Koordinaten in der Reihenfolge `i, j, k, w`, das entspricht glTF `x, y, z, w` (prüfen).
- `"data"` fehlt, wenn `SceneNode::data` `None` ist.
- Keine weitere Verschachtelung in v1 (alle Nodes haben Welt-Transformationen).

### 5.3 Meshes und Buffer

- Ein glTF-Mesh pro `MeshId`, ein Primitive pro Patch.
- Attribute:
  - `POSITION`: VEC3, `componentType` 5126 (FLOAT). `min`/`max` im Accessor sind **Pflicht**.
  - `NORMAL`: VEC3 FLOAT, Einheitslänge.
  - `COLOR_0` (optional): VEC4 FLOAT.
- Indizes: `componentType` 5123 (UNSIGNED_SHORT), wenn der Patch höchstens 65 535 Vertices hat, sonst 5125 (UNSIGNED_INT). Der Wert 65 535 selbst kommt so nie als Index vor (Primitive Restart in WebGL2).
- `mode`: 4 (TRIANGLES) für Flächen, 1 (LINES) für Strahllinien.
- **Ein** Buffer. Jedes Attribut und jeder Index-Block bekommt eine eigene BufferView. Offsets werden auf 4 Byte aufgerundet (Padding mit `0x00`). `byteLength` der BufferView ist die exakte Datenlänge ohne Padding. `target`: 34962 (ARRAY_BUFFER) für Attribute, 34963 (ELEMENT_ARRAY_BUFFER) für Indizes.
- **Normalen** werden nicht erzeugt. Sind `normals` vorhanden, werden sie beim Cast nach `f32` normiert (die Spec verlangt Einheitslänge) und als `NORMAL` geschrieben. Hat ein übergebener Vektor die Länge ≈ 0, ist das `Error::InvalidMesh`. Fehlen die Normalen, wird kein `NORMAL`-Attribut geschrieben; Viewer schattieren das Primitive dann flach.

### 5.4 Material-Mapping

| `Material` | glTF |
|---|---|
| `Glass` | `baseColorFactor [r,g,b,1]`, `metallicFactor 0`, `roughnessFactor 0.05`; Extensions `KHR_materials_transmission {transmissionFactor: 1}`, `KHR_materials_ior {ior}`, `KHR_materials_volume {thicknessFactor: thickness}` |
| `Mirror` | `baseColorFactor [r,g,b,1]`, `metallicFactor 1`, `roughnessFactor` wie angegeben |
| `Opaque` | Werte wie angegeben |
| `Unlit` | `baseColorFactor`, Extension `KHR_materials_unlit {}`; `alphaMode "BLEND"`, falls alpha < 1 |
| `Translucent` | `baseColorFactor` mit alpha, `metallicFactor 0`, `roughnessFactor 0.5`, `alphaMode "BLEND"`, `doubleSided true` |

Jedes Material bekommt einen `name` (z.B. `"glass"`, `"mirror"`, …).

### 5.5 Container

**GLB** (`container.rs`):

| Teil | Inhalt |
|---|---|
| Header (12 Byte) | magic `0x46546C67`, version `2`, Gesamtlänge (alle `u32` LE) |
| JSON-Chunk | Länge (auf 4 aufgerundet), Typ `0x4E4F534A`, JSON mit Leerzeichen (`0x20`) aufgefüllt |
| BIN-Chunk (nur wenn Buffer nicht leer) | Länge (auf 4 aufgerundet), Typ `0x004E4942`, Daten mit `0x00` aufgefüllt |

`buffers[0]` hat im GLB keine `uri`; `byteLength` ist die ungepaddete Länge.

**`.gltf` + `.bin`:** identisches JSON, aber `buffers[0].uri = bin_uri`.

### 5.6 Ursprung

Der effektive Ursprung ist `options.origin` oder, falls `None`, die Mitte der Welt-AABB aller Node-Positionen: die Mesh-Vertices, transformiert mit der jeweiligen Isometrie. Sind keine Nodes vorhanden, ist er `[0, 0, 0]`. `Scene::effective_origin()` wird öffentlich, weil der Diff sie braucht.

## 6. Strahlen

`add_ray_trace` validiert (mindestens 2 Stationen, alle Stationen gleich lang, alle Koordinaten endlich) und erzeugt bis zu zwei Nodes im Layer `Rays`:

- `"<trace.uid>/lines"`: Primitive im Modus LINES mit `Unlit`-Material
- `"<trace.uid>/envelope"`: Dreiecks-Mesh, je nach `RayStyle::envelope`

**Präzision:** Die Positionen beider Meshes werden relativ zur Mitte der AABB aller gültigen Ray-Punkte des Traces gespeichert. Diese Mitte wird zur Node-Translation. Für Rays gilt damit dieselbe `f32`-Präzision wie für Optiken. Ein übergebenes `Envelope::Mesh` wird genauso behandelt, damit Linien und Hülle exakt zueinander passen.

### 6.1 Linien (`lines.rs`)

- Auswahl: die Rays, die an Station 0 gültig sind, davon höchstens `max_lines` in gleichmäßigen Indexabständen (`i * n_valid / max_lines`).
- Für jedes Stationspaar `(k, k+1)` entsteht ein Liniensegment, wenn der Ray an beiden Stationen gültig ist.
- Farbe: `line_color`, sonst `color::wavelength_to_rgb(wavelength)`, sonst Fallback `[0.8, 0.1, 0.8, 1.0]`.

### 6.2 Farbe (`color.rs`)

`wavelength_to_rgb(lambda_m: f64) -> Option<[f32; 3]>`: stückweise lineare Näherung des sichtbaren Spektrums von 380 bis 780 nm mit Abschwächung an den Rändern; außerhalb `None`. Die Werte werden als lineares RGB behandelt, weil glTF-Faktoren linear sind.

### 6.3 Hülle

**`Envelope::Mesh`** ist der Normalfall für Opossum. Das Mesh wird wie jedes andere behandelt: keine Analyse, keine Umformung, Materialien kommen aus dem Mesh selbst. Das Crate weiß nichts über Strahlformen, und genau deshalb bildet es sie auch nicht ein zweites Mal nach.

**`Envelope::DefaultRound`** ist ein bewusst grober Fallback für Aufrufer, die nur Ray-Positionen haben (`hull.rs`). Er erzeugt einen Rotationskörper um den Schwerpunktpfad des Bündels:

Für jedes Segment `k` (Stationen `k` und `k+1`):

1. **Gültige Rays:** `V` = alle Rays, die an beiden Stationen gültig sind. Ist `|V| < 3`, wird das Segment übersprungen (kein Fehler).
2. **Achse:** `d = normalize(Σ normalize(S[k+1][i] − S[k][i]))` über `V`. Ist `d` degeneriert, wird das Segment übersprungen. `u` und `v` spannen die Ebene senkrecht zu `d` auf; `u = normalize(d × a)` mit `a` als der Koordinatenachse, die am wenigsten parallel zu `d` liegt, und `v = d × u`.
3. **Querschnitte:** `steps + 1` Ringe bei `s = j / steps`, `j = 0..=steps`. Die feste Unterteilung ist wichtig, weil sonst ein Fokus zwischen zwei Stationen verschwinden würde: Von 5 mm auf 5 mm sähe wie ein Zylinder aus, obwohl der Strahl dazwischen durch einen Punkt geht.
4. **Ring bei `s`:**
   - `p_i(s) = S[k][i] + s·(S[k+1][i] − S[k][i])`
   - Schwerpunkt `c(s)` der `p_i(s)`.
   - Radius `r(s) = max_i ‖(p_i(s) − c(s)) − ((p_i(s) − c(s))·d)·d‖`, also der größte Abstand senkrecht zur Achse.
   - Ringpunkte `c(s) + r(s)·(cos θ_j·u + sin θ_j·v)` mit `θ_j = 2πj/sectors`.
5. **Loft:** Aufeinanderfolgende Ringe werden je Sektorpaar `(j, j+1 mod sectors)` mit zwei Dreiecken verbunden. Keine Deckel, keine Normalen (flache Schattierung genügt für eine Näherungsfläche).
6. **Segmente:** Jedes Segment ist ein eigener Schlauch, alle landen in einem Patch. Verlorene Rays führen dazu, dass der Radius in den folgenden Segmenten kleiner wird, Vignettierung ist also grob sichtbar.

Die Doku dieser Variante sagt ausdrücklich, dass sie rotationssymmetrisch nähert. Wer die echte Form braucht, liefert `Envelope::Mesh`.

## 7. Protokoll

### 7.1 `optoscene-protocol`

**Frame-Layout** (alle Zahlen LE):

| Offset | Größe | Inhalt |
|---|---|---|
| 0 | 4 | magic `b"OSCN"` |
| 4 | 1 | Protokollversion `1` |
| 5 | 4 | Header-Länge `H` (`u32`) |
| 9 | H | Header als UTF-8-JSON |
| 9+H | Rest | Payload (GLB oder leer) |

```rust
/// Message header, serialized as JSON with a "type" tag.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Header {
    /// Payload: complete scene as GLB.
    FullScene,
    /// Payload: GLB containing only this layer.
    ReplaceLayer { layer: String },
    /// Payload: GLB containing the new or changed nodes.
    UpsertNodes { uids: Vec<String> },
    /// No payload.
    UpdateTransforms { nodes: Vec<TransformEntry> },
    /// No payload.
    RemoveNodes { uids: Vec<String> },
}

/// New transformation of a node, relative to the scene origin.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransformEntry {
    pub uid: String,
    pub translation: [f32; 3],
    /// Quaternion [x, y, z, w].
    pub rotation: [f32; 4],
}

pub fn encode(header: &Header, payload: &[u8]) -> Result<Vec<u8>, FrameError>;
/// Returns the header and a borrowed slice of the payload.
pub fn decode(frame: &[u8]) -> Result<(Header, &[u8]), FrameError>;
```

`FrameError` (von Hand implementiert) hat die Varianten `BadMagic`, `UnsupportedVersion(u8)`, `Truncated`, `HeaderTooLarge` und `Json(serde_json::Error)`.

### 7.2 Diff im Kern (Feature `protocol`)

```rust
pub struct SceneMessage { pub header: Header, pub payload: Vec<u8> }

impl SceneMessage {
    pub fn to_frame(&self) -> Result<Vec<u8>, Error>;
}

pub fn full_scene(scene: &Scene) -> Result<SceneMessage, Error>;
pub fn diff(old: &Scene, new: &Scene) -> Result<Vec<SceneMessage>, Error>;
```

**Regeln für `diff`**, in dieser Reihenfolge:

1. Unterscheiden sich `effective_origin()` oder `root_rotation`, ist das Ergebnis nur `[FullScene]`.
2. Nodes im Layer `Rays` werden nur als Ganzes betrachtet: Weicht die Menge ihrer uids oder irgendein Content-Hash ab, entsteht `ReplaceLayer { layer: "rays" }` mit `layer_to_glb(Rays)`. Diese Nodes fallen aus den Regeln 3 bis 5 heraus.
3. uids, die nur in `old` vorkommen, gehen gesammelt in ein `RemoveNodes`.
4. uids, die neu sind oder deren Inhalt sich geändert hat (Mesh-Hash inkl. Materialien, `name`, `layer`, `data`), gehen gesammelt in ein `UpsertNodes` mit `nodes_to_glb`, in Einfügereihenfolge von `new`.
5. Bei allen übrigen Nodes, deren Transformation sich geändert hat (Translation > 1e-9 m oder Rotationswinkel > 1e-9 rad), entsteht ein gesammeltes `UpdateTransforms` mit Translation relativ zum Ursprung als `f32`.
6. Die Ausgabereihenfolge ist `RemoveNodes`, `UpsertNodes`, `UpdateTransforms`, `ReplaceLayer`. Ohne Änderungen ist das Ergebnis ein leerer `Vec`.

### 7.3 Client-Semantik

Diese Semantik gehört in die README und wird im Viewer umgesetzt:

- `FullScene`: alles entfernen und disposen, GLB laden.
- `ReplaceLayer`: vorhandene Layer-Gruppe entfernen und disposen, Gruppe aus dem geladenen GLB einhängen.
- `UpsertNodes`: im geladenen GLB alle Objekte mit `userData.uid` suchen, bestehende Objekte gleicher uid ersetzen, neue unter der passenden Layer-Gruppe einhängen.
- `UpdateTransforms`: `position` und `quaternion` setzen.
- `RemoveNodes`: entfernen und disposen.

Beim Disposen immer Geometrien und Materialien freigeben (`geometry.dispose()`, `material.dispose()`), sonst wächst der GPU-Speicher mit jedem Update.

## 8. Beispiel: actix-Server und three.js-Viewer

Das Beispiel zeigt nur das Einbindungsmuster. Mit axum, Tauri oder Dioxus-IPC funktioniert es genauso.

**`examples/actix-viewer`** (Versionen von `actix-web`, `actix-ws` und `tokio` prüfen):

- Zustand: `Arc<Mutex<Scene>>` für die aktuelle Szene und `tokio::sync::broadcast::Sender<Vec<u8>>` für Frames.
- Routen:
  - `GET /` und `GET /viewer.js` liefern statische Dateien per `include_str!`.
  - `GET /scene.glb` liefert die aktuelle Szene als Datei.
  - `GET /ws` macht das WebSocket-Upgrade mit `actix-ws`: zuerst den `FullScene`-Frame senden, danach Broadcast-Frames als Binary weiterleiten; bei Sendefehler beenden.
- Demo-Schleife (alle 2 s): eine Linse aus `fixtures` entlang z verschieben, den Ray-Trace eines fokussierten Bündels neu berechnen, `diff(old, new)` bilden und die Frames broadcasten. Das simuliert einen Positionierungsrun.

**`static/index.html` und `viewer.js`:**

- three.js als ES-Module per Import-Map von einem CDN, mit fest angegebener Version (aktuelle Version prüfen). Benötigt werden `GLTFLoader`, `OrbitControls`, `RoomEnvironment` und `PMREMGenerator`. Die Environment-Map ist nötig, sonst wirkt Glas mit Transmission dunkel.
- `decodeFrame(ArrayBuffer)` in JS, exakt nach 7.1.
- `Map uid → Object3D` und je eine Gruppe pro Layer; Umsetzung der Client-Semantik aus 7.3.
- Jede empfangene Nachricht wird mit ihrem Typ in der Konsole geloggt (für die Abnahme).

## 9. Phasen

### Phase 1: Workspace und minimaler GLB-Export

**Aufgaben**
- Workspace und beide Crates anlegen (Protokoll-Crate zunächst nur als Gerüst), `CLAUDE.md`, README-Gerüst.
- `error.rs`, `model.rs` (alle Typen aus Abschnitt 4), `scene.rs` mit `add_material`, `add_mesh` (inkl. Validierung), `add_node`.
- `gltf/schema.rs`, `gltf/builder.rs`, `gltf/container.rs`: `POSITION`, `NORMAL` (falls übergeben), Indizes, Material nur `Opaque`, flache Node-Liste, noch ohne Root- und Layer-Gruppen.
- `fixtures.rs`: `cube(size)` und `biconvex_lens(diameter, center_thickness, r1, r2, segments)`. Die Linse besteht aus drei Patches (Vorderseite, Rückseite, Zylindermantel) mit analytischen Normalen.
- `examples/export_basic.rs` schreibt `target/out/basic.glb` mit Würfel und Linse.

**Abnahme**
- Alle Checks aus `CLAUDE.md` bestehen.
- Roundtrip-Test: `to_glb` → mit `gltf` einlesen; Anzahl Meshes/Nodes stimmt, Positionen stimmen auf `f32`-Genauigkeit.
- Test: GLB-Gesamtlänge im Header entspricht der Byte-Länge; Chunk-Längen sind Vielfache von 4.
- Test: zwei Exporte derselben Szene sind byte-identisch.
- Test: Index außerhalb des Bereichs führt zu `Error::InvalidMesh`; doppelte uid zu `Error::DuplicateUid`; eine Normale der Länge 0 zu `Error::InvalidMesh`.
- Test: Ein Patch ohne Normalen erzeugt ein Primitive ohne `NORMAL`-Attribut. Ein Patch mit Normalen gibt genau diese Richtungen wieder aus (±1e-6 nach Normierung), es wird nichts geglättet.
- Manuell: `basic.glb` hat im Khronos-Validator 0 Fehler und öffnet im three.js-Editor.

### Phase 2: Materialien, Metadaten, Instancing, Layer, `.gltf`

**Aufgaben**
- Alle Materialien nach 5.4, `extensionsUsed`.
- Hierarchie nach 5.2 (Root, Layer-Gruppen, Extras), Ursprung nach 5.6.
- Deduplizierung von Materialien und Meshes.
- `COLOR_0`, Wahl zwischen u16- und u32-Indizes.
- `to_gltf_separate`, `write_gltf`, `layer_to_glb`, `nodes_to_glb`.
- `export_basic.rs` erweitern: je ein Objekt pro Materialtyp und eine Linse zweimal instanziert.

**Abnahme**
- Test: Das Glas-Material enthält alle drei Extensions, `extensionsUsed` listet sie auf, `extensionsRequired` fehlt.
- Test: Dieselbe `TriMesh` zweimal hinzugefügt ergibt dieselbe `MeshId`, ein glTF-Mesh und zwei Nodes, die darauf verweisen.
- Test: Ein einzelner Node bei Welt-Position (1000, 0, 0) mit `origin: None` hat eine exportierte Translation ≈ 0, und Root-Extras enthalten `origin = [1000, 0, 0]`.
- Test: `uid` und `data` sind über den `gltf`-Reader in den Node-Extras lesbar.
- Test: u16-Indizes bei 65 535 Vertices, u32 bei 65 536.
- Test: `layer_to_glb(Optics)` enthält keine Nodes anderer Layer.
- Manuell: Validator 0 Fehler; im three.js-Editor sind Glas, Spiegel und Transparenz erkennbar.

### Phase 3: Strahlen

**Aufgaben**
- `rays/color.rs`, `rays/lines.rs`, `rays/hull.rs` (nur `Envelope::DefaultRound`), `Scene::add_ray_trace`.
- Primitive-Modus LINES im Builder.
- Fixtures: Hilfsfunktionen, die `RayTrace`s für die Testfälle unten erzeugen.
- `examples/export_rays.rs`: Linse plus fokussiertes Bündel, einmal mit `Envelope::default_round()` und einmal mit einem von Hand gebauten `Envelope::Mesh`, damit beide Wege abgedeckt sind.

**Abnahme**, jeweils mit einem Bündel aus konzentrischen Ringen mit Außenring aus 256 Rays, Radius `r0 = 5 mm`:
- **Linien:** `max_lines = 10` ergibt höchstens 10 Rays × Segmente an Liniensegmenten; `max_lines = 0` erzeugt keinen Linien-Node.
- **`Envelope::Mesh`:** Das übergebene Mesh erscheint unverändert im Export (Vertexzahl und Positionen identisch bis auf die Ursprungsverschiebung und `f32`), mit den Materialien aus dem Mesh.
- **`Envelope::None`:** kein Hüllen-Node.
- **DefaultRound, kollimiert:** Stationen bei z = 0 und z = 0.1 m. Alle Ringpunkte haben Radius `r0 ± 1 %`.
- **DefaultRound, Fokus:** alle Rays durch (0, 0, 0.03), Stationen bei z = 0 und z = 0.1. Der kleinste Ringradius über alle Querschnitte ist < 0.2·r0, der Fokus verschwindet also nicht.
- **DefaultRound, 45°-Spiegel:** Station 1 liegt auf der Ebene z = 0.05 + x, danach Propagation in +x bis x = 0.05. Es entstehen zwei Schläuche mit jeweils `steps + 1` Ringen.
- **DefaultRound, Vignettierung:** die Hälfte der Rays an Station 2 `None`. Der maximale Ringradius in Segment 1 ist kleiner als in Segment 0.
- **Weniger als 3 gültige Rays** in einem Segment: kein Fehler, Segment fehlt.
- Manuell: `export_rays.glb` Validator 0 Fehler, Fokus sichtbar.

### Phase 4: Protokoll und Diff

**Aufgaben**
- `optoscene-protocol` vollständig nach 7.1.
- `diff.rs` nach 7.2, Feature `protocol` im Kern.
- `Scene::effective_origin()` öffentlich.

**Abnahme**
- Roundtrip `encode` → `decode` für jede Header-Variante, mit und ohne Payload.
- `decode` liefert die passenden Fehler bei falschem Magic, unbekannter Version und abgeschnittenem Frame.
- Diff-Tests:
  - identische Szenen → leer
  - Node verschoben → nur `UpdateTransforms`
  - Mesh geändert → `UpsertNodes`
  - Node entfernt → `RemoveNodes`
  - Ray-Trace geändert → `ReplaceLayer("rays")`
  - `origin` geändert → nur `FullScene`
  - Reihenfolge der Nachrichten nach 7.2
- `cargo build -p optoscene-protocol --target wasm32-unknown-unknown` gelingt (Target vorher installieren).
- `cargo tree -p optoscene --no-default-features` zeigt nur `nalgebra`, `serde`, `serde_json` und deren Abhängigkeiten.

### Phase 5: Beispiel-Server und Viewer

**Aufgaben**
- `examples/actix-viewer` nach Abschnitt 8.
- README: Abschnitt „Integrating with a web backend“ mit der Client-Semantik aus 7.3 und einem Hinweis, dass das Muster transportunabhängig ist.

**Abnahme**
- `cargo run -p actix-viewer` startet; im Browser sind Linse (Glas), Ray-Linien und Hülle zu sehen.
- Die Linse bewegt sich alle 2 s, ohne dass die Seite neu lädt. Die Konsole zeigt `update_transforms` und `replace_layer`, aber kein erneutes `full_scene`.
- `renderer.info.memory.geometries` bleibt über 20 Updates konstant.

### Phase 6: Opossum-Adapter (im Opossum-Repo, eigener Plan)

Nur als Orientierung, nicht Teil dieses Repos:

- Abhängigkeit auf `optoscene` mit Feature `protocol` nur im Backend-Crate.
- Opossum-Node → `TriMesh`: Tessellierung aus der Parametrisierung mit **analytisch berechneten Normalen**; ein Patch pro Fläche. Die Normalen kommen aus `∂f/∂u × ∂f/∂v`, das Export-Crate erzeugt bewusst keine.
- `uid` = Node-UUID; `transform` = Alignment-Isometrie aus dem Positionierungsrun; `data` = relevante Node-Parameter.
- Referenzierte Nodes teilen sich dieselbe `MeshId`.
- Glas: `ior` bei der Designwellenlänge aus der Glasdatenbank, `thickness` = Mittendicke.
- Ray-Trace des Positionierungsruns → `RayTrace` (eine Station pro durchlaufener Fläche).
- **Hülle:** Opossum baut sie selbst als `Envelope::Mesh`, weil nur dort Apertur, Clipping und die echte Bündelform bekannt sind. Ein möglicher Ansatz, falls die Konturen nicht ohnehin schon vorliegen: pro Querschnitt eine Polarkontur mit fester Sektorzahl (maximaler Radius je Winkelsektor um den Schwerpunkt, leere Sektoren interpoliert), Schnitte adaptiv verfeinern, wo sich die Querschnittsfläche stark ändert, und die Ringe zu einem Schlauch loften. Das ist Geometriearbeit für den Adapter, nicht für `optoscene`.
- Für schnelle Übersichten oder frühe Entwicklungsstände reicht `Envelope::default_round()`.
- Das Backend hält die letzte `Scene` pro Session, bildet nach jedem Positionierungsrun `diff` und sendet die Frames über den bestehenden actix-WebSocket.
- Das Dioxus-Frontend hostet den Canvas und `viewer.js` (JS dekodiert die Frames) oder dekodiert in Rust/WASM mit `optoscene-protocol` und reicht die GLB-Payload per JS-Interop weiter.
- Die Node-Konfiguration nutzt denselben Pfad mit einer Szene, die nur die eine Optik enthält.

## 10. Nicht im Scope von v1

- Verschachtelte Node-Hierarchien
- Texturen und Fluenz-Einfärbung
- Ray-Gewichte/Energie
- Animationen
- Kompression (meshopt, Draco, `KHR_mesh_quantization`)
- Picking-UI
- Andere Einheiten als Meter

**Ausdrücklich außerhalb der Verantwortung des Crates**, dauerhaft und nicht nur in v1:

- Tessellierung parametrischer Flächen
- Erzeugen oder Glätten von Normalen
- Rekonstruktion der echten Strahlform aus Ray-Daten (über den groben `DefaultRound`-Fallback hinaus)
- Physikalische Modellierung jeder Art

Das alles gehört in den Aufrufer, der die Parametrisierung und die Physik kennt.

Werden die Scope-Punkte oben später gebraucht, bekommen sie eigene Phasen in einer v2 dieses Plans.
