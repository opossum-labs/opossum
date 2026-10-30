# Component geometry

This page follows the shape of a component from the properties a user enters to the surfaces rays are traced at. It
is written for developers. The first part explains the concepts. The second part follows two components through the
code from start to end, and the last part lists where to change what.

For how components are placed along the optical axis, see [Model geometry](../geometry.md). For what the optic shape
means to a user, see [Optic shape](../../reference/nodes.md#optic-shape).

## Part 1: Concepts

### Defined, derived, used

The chart shows what depends on what. Its three bands separate what the user defines from what is derived from it
and from what uses the result; the rest of this page explains each box.

![What a component's geometry depends on](../../images/component_geometry_flow.svg)

1. **Defined by the user.** What the user enters in the GUI and the `.opm` file stores:
   - the *node properties*: the node type, which acts as a preset (a lens is a *singlet* whose parameters are the
     properties of the lens), the shape (curvatures, thickness, optic shape and so on), and one surface property per
     face with its coating and its laser induced damage threshold (LIDT);
   - the *placement*: an absolute position, or an automatic one that the alignment run fills in, and the alignment
     (decenter, tilt);
   - the *connections* between ports, each with its distance.
2. **Derived.** None of it is written to the file:
   - The node derives its *geometry* from the shape (`create_geometry`), stated relative to the node. It is derived
     again whenever it is needed, so it cannot fall out of step with the properties. It holds the numbered *faces*,
     each with its *optic surface*, where each face ends sideways, and, for a solid, the *body* of material between
     its faces, with the material it is filled with. The body has no surfaces of its own; it is bounded by the very
     faces rays are traced at.
   - `effective_node_iso` states where the node sits and how it is turned: its position followed by its alignment.
   - Installing the geometry (`install_geometry`) moves it by the node's placement: every face and every rim moves,
     nothing else changes. Every face then takes the coating and the LIDT of its surface property, and every port
     names the face it sits on. The result is the node's *runtime geometry*. For a volume node, its body is the
     node's *medium*, which holds the inversion a gain model builds in it. Unlike the geometry, the runtime geometry
     is kept until it is rebuilt (see [When surfaces are rebuilt](#when-surfaces-are-rebuilt)).
3. **Used.**
   - Ray tracing and ghost focus analysis intersect rays with the faces and let the surface act on them. A node
     finds the face under the port the light arrives at or leaves through.
   - Gain and absorption act in the body: along the chord of each ray in a ray trace, and without rays in an energy
     flow analysis.
   - The alignment run traces the axis ray through the faces. Where the ray leaves a node, carried on by the distance
     of the connection, is the automatic position of the next node.
   - The 3D view meshes the geometry in the node's own frame (`OpticNodeExt::geometry_mesh`) and puts the mesh at
     the node's placement; it never reads the runtime geometry.

All of it comes from one description: the surface a ray hits, the edge that decides whether it hits, and the medium a
gain model acts in. They therefore cannot disagree.

### Surface or solid

The first thing a geometry states is what kind of component it is:

- A **surface** has no thickness: a thin mirror, a grating, a filter, a detector.
- A **solid** encloses material between its faces: a lens, a wedge. It holds the material it is filled with, which
  the node takes from its material property when it derives the geometry. There are three kinds of solid, see below.

Whether a node has a *medium* — a material, absorption and possibly gain — is therefore answered by its geometry
alone (`OpticNodeExt::encloses_volume`): a node whose geometry is a solid has one, every other node has none.

Separate from its geometry, every node type has an *appearance*: whether and how it is drawn in the 3D view. The node
does not state it. The 3D view looks it up by node type (`Appearance::of_node_type` in `opossum_scene`), so it never
reaches the core and never affects a simulation. Some nodes are traced at a plane but have no appearance, so they are
not drawn at all.

| Group | Nodes | Geometry preset | Appearance |
|---|---|---|---|
| Volume components | lens, cylindric lens, wedge | `singlet`, `cylindrical_singlet`, `wedge` (all extruded solids) | glass |
| Surface components | mirror, parabolic mirror | `surface` | mirror |
| | beam splitter | `plane` | mirror |
| | grating | `plane` | iridescent |
| | ideal filter, paraxial surface | `plane` | transmissive |
| | energy meter, fluence detector, spectrometer, spot diagram, wavefront monitor | `detector` | detector |
| Traced, not drawn | source, dummy, ray propagation visualizer | `plane` | none |
| No shape of its own | group, reference | none | none |

Every node type has to state its geometry: `create_geometry` has no default, so the compiler forces it. Its appearance
is an entry in the look table of `opossum_scene`, and a test over the node registry fails for a type without one
(`every_registered_node_type_has_a_look`). Glass takes its refractive index from the material of the solid, so only a
solid can be drawn as glass.

### Three kinds of solid

- **Extruded.** Two faces cap a profile that is extruded along an axis. The profile is the optic shape. The center
  thickness is not stored separately: it is where the vertex of the rear face sits, together with any tilt of that
  face (a wedge). Lens, cylindric lens and wedge are extruded solids.
- **Parametric solid.** The intersection of the inner sides of several faces. Built from flat faces only, it is
  always convex: prisms, corner cubes, roof prisms. Curved faces whose inner side is their outside also allow concave
  shapes.
- **Assembly.** Several solids (*parts*), each with its own placement and material, joined by *interfaces*. An
  interface is a pair of faces of different parts that lie on each other, such as the cemented surface of a doublet
  or the diagonal of a beam splitter cube. A part has to enclose a volume and must not be an assembly itself; this
  is checked when the part is created. The assembly checks its interfaces when it is created:
  - an interface joins two different parts,
  - the faces it refers to exist,
  - they are the same surface at the same place,
  - the two parts lie on opposite sides of it.

All kinds are moved the same way. Each says whether a point lies inside it, how far a ray travels through it and
which side of each of its faces it lies on (the `Body`), and where its faces end sideways (`GeometryKind::includes`).
No node type uses a parametric solid or an assembly so far; they are described already so that the data model is
settled before the first component needs them. Four limits are stated, not hidden:

- The bounding box of a parametric solid or an assembly is not determined yet, so a gain grid over such a medium is
  refused.
- A face is the part of its surface on the sheet through its vertex, as for every component. A face that is a whole
  closed surface, such as a ball, cannot be traced yet.
- A parametric solid cannot be meshed yet: its faces end where they meet each other, which no outline states. The
  3D view lists such a node as skipped instead of drawing it.
- An assembly is filled with the materials of its parts and names no single one, so light is not propagated through
  it as one medium yet; this is refused.

### Faces, optic shape and rim

- A **face** describes one optical surface: its *surface shape* (plane, sphere, cylinder or parabola) and its
  *frame*. The frame sits at the vertex of the surface, its z axis along the axis of the surface, and the shape is
  computed in it; a sphere or a cylinder is curved around its center of curvature, one radius along z from the
  vertex. The faces of a geometry are numbered: an extruded solid has its front face as 0 and its rear face as 1, a
  surface has the one face 0, a parametric solid counts its faces in the order they are given, and an assembly
  counts the faces of its parts one after the other. What a face means, for instance which one light enters
  through, is not decided there but by the node.
- The **optic shape** is the property that states how far the component reaches sideways: an *outline*, an edge
  and, on a bored mirror, a bore through it (see [Bores](#bores)), in the plane perpendicular to the axis of the
  component. An open optic shape has no outline, and the component is unbounded.
- A **rim** places the outline in space. It holds the outline and a frame: the outline lies in the xy plane of that
  frame and is extruded along its z axis.

An extruded solid holds exactly such a rim next to its two faces and ends there. A surface states what its rim means,
its *extent*:

- **open**: there is no rim, and the surface is unbounded;
- **edge**: the component ends at the rim, so a ray outside misses it;
- **window** (detectors only): all light passes, but only the light inside the rim is recorded.

A curved face has to reach the edge of its optic shape. A sphere or a cylinder ends at its radius of curvature, so a
radius smaller than the largest distance of the edge from the axis is rejected when the geometry is derived. A
hemisphere is still allowed.

### Where a face ends

The surface of a face is unbounded: a plane reaches in every direction, a sphere closes on itself. Where the face of
a component ends is a question every kind of geometry answers for itself (`GeometryKind::includes`):

| Kind | A point on the surface belongs to the face if |
|---|---|
| Extruded solid | it lies within the rim of the profile, measured along the z axis of the rim |
| Surface | the same, if its extent is an edge. A window is no edge: the surface reaches as far as light does |
| Parametric solid | it lies on the inner side of every *other* face |
| Assembly | the part the face belongs to says so |

On a closed surface such as a sphere, the point also has to lie on the part that contains the vertex. A rim is thus
one way to state an edge, the way an extrusion does. The faces of a parametric solid end at each other and need none.
A face reaches into a bore as well: only its edge ends it.

### Bores

Only the optic shape of a bored mirror has a bore; the validator of every other optic shape refuses one
(`Validator::OpticShape`). A bore is a prism along its own axis (`Bore::axis`): its shape is the cross section across
that axis, and its center is where the axis crosses the plane of the outline. Untilted, it runs along the z axis of
the rim.

- **Hit test.** The bore lies within the component, it only holds no material. A ray through it meets the face, and
  `Geometry::intersect` returns a `SurfaceHit` that is not on the material (`SurfaceHit::on_material`) and is not
  recorded. The kind says which face has a bore (`GeometryKind::bored_rim`), and the rim tests the point in the prism
  (`Rim::in_bore`), so a tilted bore is found where it actually meets each face.
- **Effect.** At a hit in the bore, `Geometry::interact_with_rays` leaves the ray untouched, whatever the
  missed-surface strategy, unless the effect acts there as well (`SurfaceEffect::acts_in_bores`). Only
  `SplittingConfig`, with which the axis is split during positioning, does, so the axis sees only the edge.
- **Body.** The material of a body is its outline without the bore (`Rim::contains`), so the chord of a ray through the
  bore holds none. `Extruded::new` checks that the bore lies within the edge where it meets each face
  (`Outline::check_bore_on`).
- **Splitting.** A bored mirror splits the light as a beam splitter does (`Splitter` in `nodes/splitter.rs`). Light
  enters at the face of its input port and leaves at the face of the output port that collects the transmitted part.
  On a body, the part transmitted into the material is not traced yet and is dropped with a warning. So is light in
  the bore that does not meet the exit face in the bore as well: it has run into the wall of the bore.
- **Axis.** The transmitted axis starts anew (`Ray::restart_at`, which clears its position history) at the center of
  the bore on the entry face (`Rim::bore_center_on`). From there it runs parallel to the arriving axis to the exit
  face.
- **Drawing.** Each face is meshed over the outline where the bore meets it (`Outline::triangulate_on`). The wall of
  the bore is drawn between those two rings along the axis of the bore (`Outline::rim_axes`, `SurfaceMesh::between`).

### Ports and surfaces

Ports belong to the graph and nothing else. Edges connect them; a port has a name and carries no optical property,
no coating, no LIDT, no aperture. Ports are not written to the `.opm` file either: a node creates them with its
geometry.

Surfaces belong to the geometry. Every face holds its *optic surface*:

- the face itself, its shape at the frame of its vertex,
- the coating and the LIDT,
- per-surface ray caches and a hit map, which an analysis fills.

The coating and the LIDT are properties of the node, one per face and named after it (`GeometryKind::face_name`):
`surface` for a single surface, `front surface` and `rear surface` for an extruded solid, `surface 1`, `surface 2`, ...
for a parametric solid or an assembly. Each holds a `SurfaceConfig`: by default an ideal antireflective coating, or a
`Separator` of 50 % on a splitting face (see below), and an infinite LIDT. A node creates these properties when it is
made (`create_default_surface_properties`), and every face gets its own, whether a port sits on it or not.

The node decides which port sits on which face (`OpticNode::surface_ports`): a list of port names, each with the
number of its face. It also names the faces at which it follows both parts of the light, the transmitted and the
reflected one (`SurfacePorts::splitting`; today only the face of a beam splitter). The surface property of such a
face takes only a `Separator` coating, that of every other face any coating but a `Separator`
(`Validator::SurfaceCoating`), so neither a file nor the node editor gives a face a coating its node is not made
for. A lens puts its input port on the front face and its output port on the rear face; what is "in"
and what is "out" is thus the node's decision, not the geometry's. A mirror or a beam splitter has one face, so all of
its ports name that face, and they share its one optic surface: its coating, its caches and its hit map. A face
without a port still exists and keeps its coating.

What a node keeps while it is traced is its *runtime geometry*: the placed geometry with the optic surfaces of its
faces and, for a volume node, the body as the node's *medium* with the inversion a gain model builds in it
(`Body::inversion`). None of this is written to the `.opm` file; it is built again after loading. A port on a face
the geometry does not have is an error that names the port, the face and the number of faces.

### Surface effects

To trace rays at a surface, a node lets its runtime geometry act on them:
`Geometry::interact_with_rays(face, &mut rays, effect, part, missed)`, with the face under the port the light arrives
at (`OpticNodeExt::face_of_port`). The component acts on the light, not the other way round. The coating of the face
is built once for the whole bundle (`Box<dyn Coating>`), so a `Separator` builds its spectrum once, not per ray. For
every valid ray:

1. The geometry answers where the ray hits the face (`Geometry::intersect`, which returns a `SurfaceHit`). A ray that
   misses follows the missed-surface strategy.
2. At a hit, the *surface effect* acts on the ray (`SurfaceEffect::act_at`): it moves the ray to the hit, lets it
   continue as the transmitted part and returns a reflected part, if any. The effect calls the physics of a single
   ray, which needs nothing geometric any more.
3. The effect acts on the helper rays of the ray as well, each where it hits the face; a helper ray that misses
   follows the missed-surface strategy like any ray.
4. Once every ray is answered, the optic surface of the face records the hits in its hit map.

What a surface does is one effect per kind of surface. Each lives next to the type that defines it, so a new one does
not touch the loop:

| Effect | Where | At a hit |
|---|---|---|
| `Refraction` | `core_optics/surface_effect.rs` | Snell's law into the index behind the surface; the coating splits off a reflected part (`Ray::refract_at`). Without an index the surface is passive and keeps each ray's direction |
| `SplittingConfig` | `coatings/separator.rs` | a passive refraction with the reflectivity `1 − T(λ)` of the splitting configuration instead of the surface's coating; only the positioning of a beam splitter or a bored mirror uses it, for a neutral 50:50 split. It is the one effect that acts in a bore (see [Bores](#bores)) |
| `FilterType` | `nodes/ideal_filter/filter_types/mod.rs` | a passive refraction, then the attenuation by the transmission (`Ray::filter_energy`) |
| `ParaxialLens` | `nodes/paraxial_surface.rs` | a passive refraction, then the deflection of an ideal thin lens (`Ray::refract_paraxial`) |
| `Diffraction` | `nodes/reflective_grating.rs` | the grating equation for one order (`Ray::diffract_at`); the whole energy goes into the order, and no hit is recorded |

At every surface the light is split into a transmitted and a reflected part. Which of them is the intended beam is
bookkeeping of the node, not physics; the node states it as a `ReflectedPart`:

| `ReflectedPart` | Used by | The reflected part |
|---|---|---|
| `Ghost` | lens, filter, detectors | is a ghost reflection: it branches off the bundle and starts a position history of its own |
| `Main` | mirror, parabolic mirror, grating | is the intended beam: it continues the bundle, and the bounce does not count as a ghost |
| `Branch` | beam splitter, bored mirror | is a second intended beam: a bundle of its own, and both parts keep their position history |

`Geometry::propagate_rays` does the same for all bundles of an analysis. After each bundle it lets the analyzer act
at the surface (`PropagationStrategy::on_surface_interaction`, for instance the ghost-focus cache), and it appends the
ghost reflections cached at the surface for the current direction.

### Surfaces in the sequential trace

The analyzers trace the graph node by node. The analysis of a node receives the light per input port and addresses
surfaces by port name; the node's surface ports say which face lies under the port. A lens traces its rays in through
the front face under its input port and out through the rear face under its output port.

Inverting a node rebuilds nothing; ghost focus analysis does this on every second pass. The ports of the node swap
inputs and outputs, so the same lookup returns the rear face first. The geometry stays as it physically is; only the
direction light takes through it changes. A single surface is the same surface in both directions, so light passing
it forward and backward is recorded in its one hit map.

Whether a ray hits a component is decided in exactly one place, by the geometry (`GeometryKind::includes`). It is
asked when a ray is traced at a face (`Geometry::intersect`) and when the body of a volume node measures how far a
ray travels through it. The point has to meet two conditions:

- the face reaches there (see [Where a face ends](#where-a-face-ends)); for a lens, the point lies within the rim,
  measured along the z axis of the rim;
- on a closed surface such as a sphere, it lies on the part that contains the vertex. A ray that reaches a sphere
  only behind its equator misses.

What happens to a ray that misses is decided by the analyzer's missed-surface strategy, not by the surface:

- `Stop`: the ray is lost.
- `Ignore`: the ray passes unchanged.

The alignment run always uses `Stop`.

### When surfaces are rebuilt

Surfaces are never moved one by one. A rebuild installs a new runtime geometry: the geometry derived anew and moved
to the node's current placement as a whole, every face with the coating and the LIDT of its surface property, the
ports set from the node's surface ports, and an empty inversion. It replaces the runtime geometry installed before,
with everything an analysis left on it; caches and hit maps are not carried over. No rebuild falls between an analysis writing them and the report reading them: every
analysis clears them after the alignment run, and nothing is rebuilt while it traces. Since the medium is the body of
the geometry, it is rebuilt with it; the inversion stays empty until the medium is prepared for an analysis.

| Event | Rebuilds the surfaces |
|---|---|
| Creating a node (`Default`, `new`) | yes |
| Loading an `.opm` file | yes |
| Placing the node (alignment run, absolute position) | yes |
| Changing the alignment (decenter, tilt) | yes |
| Changing the coating or LIDT of a surface through `set_coating` or `set_lidt` | yes |
| Start of a ray-trace or ghost-focus analysis | yes, for every node |
| Preparing the medium of a volume node | yes |
| Changing any other property (curvature, thickness, optic shape, a surface property in the GUI, ...) | **no** |
| Inverting a node | no, nothing to rebuild |

A property change does not rebuild anything. For that reason, every analysis that traces rays first rebuilds all
surfaces. The 3D view does not depend on the rebuilt surfaces at all: it meshes the geometry in the node's own frame
and puts the mesh at the node's current placement.

## Part 2: In the code

All paths are relative to `opossum_core/src` unless stated otherwise.

### The types

| Type | File | Role |
|---|---|---|
| `Geometry` | `geometry/node_geometry.rs` | the geometry of a component with the optic surface of every face, one variant per kind: `Surface`, `Extruded`, `Parametric`, `Assembly`. `Geometry::kind` is the one place that tells the kinds apart; `Geometry::intersect` answers where a ray hits a face; `interact_with_rays` and `propagate_rays` let the surface of a face act on rays; `move_by` moves it |
| `SurfaceHit` | `geometry/node_geometry.rs` | where a ray hits a face: the point, in the frame of the geometry and in that of the face, the normal, and whether a detector records it |
| `GeometryKind` | `geometry/node_geometry.rs` | what every kind answers: the optic surfaces of its numbered faces (`surface`, `surface_mut`), the names of its faces (`face_name`), where they end (`includes`), the window of a detector, its body (`as_body`, `as_body_mut`), how it moves (`move_by`) and how it is drawn (`mesh`). It has no default methods, so a new kind has to answer every question |
| `FaceIndex` | `geometry/face.rs` | the number of a face in its geometry |
| `SurfaceGeometry`, `SurfaceExtent` | `geometry/node_geometry.rs` | a single surface: one face, and whether it is open, ends at an edge or records within a window |
| `Extruded` | `geometry/solid/extruded.rs` | two faces capping a profile; holds a `Rim` and its material |
| `ParametricSolid`, `Side` | `geometry/solid/parametric.rs`, `geometry/solid/mod.rs` | the intersection of the inner sides of several faces |
| `Assembly`, `Part`, `Interface`, `FaceId` | `geometry/solid/assembly.rs` | several solids sharing interfaces |
| `Face` | `geometry/face.rs` | one surface: its shape at the frame of its vertex. Intersects rays, tells which side of it a point lies on, moves |
| `SurfaceShape`, `GeoSurface`, `Plane`, `Sphere`, `Cylinder`, `Parabola` | `geometry/surface/` | the shape of a surface and its math in the vertex frame; `SurfaceShape::as_geo_surface` hands out the math |
| `Rim` | `geometry/rim.rs` | an outline at a frame: whether a point lies within it or in its bore, whether a face reaches a point, where the axis of the bore meets a face |
| `Body`, `BoundingBox` | `geometry/body/mod.rs` | what a volume answers: is a point inside, how far a ray travels in it, which box encloses it, which side of a face it lies on, the material it is filled with (`material`, none for an assembly or a reference not resolved yet) and the inversion a gain model built in it (`inversion`, `inversion_mut`). Each kind of solid implements it next to its definition. What the medium does to the rays passing it (`propagate_inside`: gain and absorption along the chord) is a method of `dyn Body`, the same for every kind |
| `OpticShape`, `Outline`, `Bore` | `apertures/optic_shape.rs` | value of the `optic shape` property; the outline says whether a point lies within it, and the bore is a prism along its own axis |
| `OutlineMesh` | `apertures/mesh.rs` | an outline triangulated in its plane (`Outline::triangulate`): where a face is sampled when it is drawn |
| `SurfaceMesh` | `geometry/mesh.rs` | one drawn patch of surface: points, normals and triangles. A face laid over an outline mesh gives one (`Face::mesh_over`), and the lateral face of a solid is drawn between two (`SurfaceMesh::between`) |
| `FacePatch` | `geometry/mesh.rs` | a `SurfaceMesh` with the face it lies on, as `GeometryKind::mesh` returns it; the lateral face of a solid lies on none |
| `OpticSurface` | `core_optics/optic_surface.rs` | the surface of one face: the face, coating, LIDT, caches and hit map |
| `SurfaceConfig` | `core_optics/surface_config.rs` | value of a surface property: the coating and the LIDT of one face |
| `NodeAttr` | `core_optics/node_attr.rs` | keeps the runtime geometry (`runtime_geometry`, `set_runtime_geometry`), `#[serde(skip)]` |
| `SurfaceEffect`, `Refraction` | `core_optics/surface_effect.rs` | what a surface does to a ray at a hit; the other effects live next to their nodes (see [Surface effects](#surface-effects)) |
| `ReflectedPart` | `light/rays.rs` | which part of a split is the intended beam: the position history, bounce counter and bundle identity of both parts |
| `OpticPorts` | `core_optics/optic_ports.rs` | the names of the input and output ports, graph logic only |
| `SurfacePorts` | `core_optics/optic_ports.rs` | the ports a node declares, each with the face it sits on, and its splitting faces (`OpticNode::surface_ports`); `face_of` looks one up |
| `Appearance`, `SurfaceFinish` | `opossum_scene/src/appearance.rs`, not in the core | how a node type is drawn, looked up in the look table (`Appearance::of_node_type`), with the presets `mirror`, `transmissive`, `detector`, `glass` and `grating`, and how a coating mirrors one of its faces (`Appearance::coated`) |

### Example 1: a plano-convex lens

The setup is a source, then a lens 100 mm further on:

- front curvature 50 mm,
- rear curvature infinite (flat),
- center thickness 5 mm,
- the default optic shape, a circle of 12.5 mm radius.

Light runs along +z.

**Step 1: describe.**

1. `Lens::create_geometry` (`nodes/lens/mod.rs`) reads the three properties.
2. It asks `OpticNodeExt::cross_section` (`core_optics/optic_node_ext.rs`) for the outline of the optic shape. An
   open optic shape has none, and `Lens::create_geometry` refuses it, since the medium would be unbounded.
3. It asks `OpticNodeExt::material` for the material property as it is stored: embedded, or, while a document is
   being loaded, still a reference that is resolved afterwards.
4. It returns `Geometry::singlet(50 mm, ∞, 5 mm, circle, material)`.

`Geometry::singlet` hands this on to `Geometry::two_faced` as:

- front shape `SurfaceShape::spherical(50 mm)`, which is a `Sphere`,
- rear shape `spherical(∞)`, which is a `Plane`,
- the rear vertex `Isometry::new_along_z(5 mm)`.

`two_faced` builds `Extruded::new(axis = identity, front = Face(Sphere, identity), rear = Face(Plane, z = 5 mm),
circle, material)` and returns it as `Geometry::Extruded`.

`Extruded::new` checks the reach of both faces: the circle reaches 12.5 mm from the axis, and the sphere reaches
50 mm. A front curvature of 10 mm would be refused with "the front face does not reach the edge of its optic shape,
12.500 mm from the axis: its radius of curvature is smaller". It then moves both faces by the axis, gives the rim the
axis as its frame and wraps each face in an optic surface with the default coating and LIDT; for a lens the axis is
the identity.

**Step 2: place.** `RayTracingAnalyzer::analyze` (`analyzers/raytrace.rs`) first calls `NodeGroup::rebuild_surfaces`.
It then calls `calc_node_positions` with `RayTraceConfig::for_positioning`. For the lens,
`OpticGraph::set_node_isometry` (`nodes/node_group/optic_graph/analysis.rs`) does the following:

1. It propagates the axis ray by the 100 mm of the connection.
2. It turns the ray into a frame, `world_frame`.
3. It computes `node_iso = world_frame ∘ entrance_frame⁻¹`. The entrance frame
   (`AnalysisRayTrace::axis_entrance_frame`) is the identity for every node except the second input of a beam
   splitter. Here `node_iso` is therefore a translation by (0, 0, 100 mm).
4. It stores the result through `OpticNode::set_positioning(NodePositioning::Automatic(..))`, which calls
   `update_surfaces`.

The origin of the node, its anchor point, is the vertex of the front face.

**Step 3: build.** `Lens::surface_ports` returns `SurfacePorts::THROUGH_SOLID`: `input_1` on `Extruded::FRONT` and
`output_1` on `Extruded::REAR`. `OpticNode::update_surfaces`, the same for every node, hands these ports to
`OpticNodeExt::install_geometry` (`core_optics/optic_node_ext.rs`), which does the following:

1. `placed_geometry` calls `create_geometry` again, so it works from the properties as they are now, and moves the
   result by `effective_node_iso()`, the position followed by the alignment, or the identity for a node that has not
   been placed. `Geometry::move_by(&node_iso)` moves every face and the rim: each frame becomes `node_iso ∘ frame`.
2. For every port it checks that the geometry has its face; a face it does not have is an error. The node's ports
   are set from these port names.
3. Every face takes the coating and the LIDT of its surface property: face 0 those of `front surface`, face 1 those
   of `rear surface`.
4. `NodeAttr::set_runtime_geometry` keeps the result, with an empty inversion. Since the geometry encloses a volume
   (`GeometryKind::as_body`), it is the medium of the lens too; a single surface would leave the node without one.

| Face | frame in the geometry | frame at the node |
|---|---|---|
| 0, front, sphere R = 50 mm | identity | (0, 0, 100 mm) |
| 1, rear, plane | z = 5 mm | (0, 0, 105 mm) |

The frame of a face sits at its vertex for every shape; the sphere computes around its center of curvature,
(0, 0, 50 mm) in its own frame.

The runtime geometry of the lens then holds:

```text
face 0, front  = OpticSurface { Sphere with its vertex at z = 100 mm, coating + LIDT of "front surface" }
face 1, rear   = OpticSurface { Plane at z = 105 mm, coating + LIDT of "rear surface" }
rim            = the circle, at z = 100 mm
inversion      = none
ports          = input_1 → face 0, output_1 → face 1
```

The rim was moved together with the faces, so asking where a face ends needs no recomputation.

**Step 4: the medium.** After the alignment run the analyzer calls `reset_data` and then `OpticNode::prepare_volume`.
For a node whose geometry encloses a volume (`OpticNodeExt::encloses_volume`), `prepare_volume` does the following:

1. It calls `update_surfaces` once more, which rebuilds the surfaces and the medium as in Step 3.
2. If the gain model reads an inversion, it builds one and stores it in the body of the medium
   (`Extraction::build_inversion`, `Body::inversion_mut`).

The body is the extruded solid of the placed geometry from Step 3, with the frame of its rim, `node_iso ∘ axis`. It
is bounded by the very faces light is traced at: its front face is face 0 under `input_1`, the sphere, its rear face
face 1 under `output_1`, the plane. It knows no port names.

**Step 5: trace.** `OpticGraph::analyze_raytrace` (`nodes/node_group/optic_graph/raytrace.rs`) walks the graph and
hands each node the light that arrives at its ports. `Lens::analyze` (`nodes/lens/analysis_raytrace.rs`) calls
`OpticNodeExt::unified_analyze_volume_node`, the volume counterpart of `unified_analyze_single_surface_node`, which
does two things:

1. `single_io_port_names(self)` returns `("input_1", "output_1")`. It takes them from `self.ports()`, which accounts
   for inversion.
2. `OpticNodeExt::pass_through_volume_generic("input_1", "output_1", ..)` runs three steps:
   1. `pass_through_surface_generic("input_1", &Refraction(Some(index of the material)), .., ReflectedPart::Ghost)`
      finds the face under `input_1` (`face_of_port`, face 0), lets the runtime geometry of the lens act on the
      bundles and follows this chain: `Geometry::propagate_rays` → `Geometry::interact_with_rays` →
      `Geometry::intersect(face 0, ray)` → `Refraction::act_at` → `Ray::refract_at(&hit, coating, n)`. The index
      is that of the material the body of the runtime geometry holds.
   2. `Body::propagate_inside` applies gain and absorption along the chord through the body of the medium: the gain
      model the operating point runs the node with, and the absorption of the body's material. It does not move the
      rays, and a positioning run skips it.
   3. `pass_through_surface_generic("output_1", &Refraction(Some(ambient index)), ..)` runs the same chain at face 1,
      the rear plane.

`Geometry::intersect` (`geometry/node_geometry.rs`) decides between hit and miss:

1. `Face::intersect` turns the ray into the vertex frame of the face, intersects it with the unbounded surface
   (`GeoSurface::calc_intersect_and_normal`) and turns the result back.
2. The point counts only if the geometry says its face reaches there: `GeometryKind::includes(face, point)`. For the
   lens this is `Rim::bounds`: `Rim::contains(point)` and `Face::is_on_vertex_sheet(point)` both hold.
3. The `SurfaceHit` carries the point, the normal, the point in the frame of the face (for the hit map and the phase
   of a grating) and whether it is recorded, which only the window of a detector denies.

`Ray::refract_at` then needs nothing geometric any more: it applies Snell's law and the coating, adds the path length
and leaves a hit point in the frame of the face. `Body::path_length_inside` of the extruded solid asks the same
`includes` when it measures the chord through the body.

A ray parallel to the axis at y = 13 mm meets the sphere, but outside the 12.5 mm circle, so it misses. With `Stop`,
`Geometry::interact_with_rays` invalidates it (`MissedSurfaceStrategy::apply_to`); with `Ignore`, it stays unchanged.
Nothing else acts on a ray that misses: whatever a surface does happens in its effect, at the hit.

**Variations.**

- *Inverted lens.* On every second ghost-focus pass the whole scenery is inverted. Nothing is rebuilt:
  - `ports()` swaps, so `single_io_port_names` returns `("output_1", "input_1")`.
  - `pass_through_volume_generic` passes `backward = self.inverted()`, and the rear plane is traced first.
  - The body stays the same.
- *Changed property.* After `set_property("center thickness", ..)` the description changes at once. The runtime
  surfaces, and the medium with them, stay stale until the next rebuild: at the start of an analysis, or through an
  explicit `update_surfaces`. This holds for a changed material too, since the medium holds its own. Whatever builds
  the shape from `create_geometry()` directly, such as the 3D view, is always current.

### Example 2: a flat mirror tilted by 45°

The setup is a source, then a thin mirror 100 mm further on:

- curvature infinite (flat),
- the default optic shape,
- an alignment tilt of 45° about x, which turns the beam by 90°.

**Describe.** `ThinMirror::create_geometry` (`nodes/thin_mirror.rs`) returns
`Geometry::surface(identity, SurfaceShape::spherical(∞), identity, Some(circle))`. The shape `spherical(∞)` is a
`Plane`, and the result is a `SurfaceGeometry` with the face `Face(Plane, identity)` and the extent
`Edge(Rim(circle, identity))`. Its reach check always passes, since a plane reaches any edge.

**Place and build.**

1. The mirror is placed as in the lens example, at a translation by (0, 0, 100 mm).
2. `effective_node_iso` is that position followed by the alignment: the translation, then 45° about x.
3. `ThinMirror::surface_ports` returns `SurfacePorts::SINGLE_SURFACE`: `input_1` and `output_1`, both on face 0
   (`SurfaceGeometry::FACE`), and `install_geometry` installs the geometry as in the lens example.
4. A surface has its one face, so both ports share its one optic surface. A surface encloses no volume, so the mirror
   gets no medium.
5. `ThinMirror::default` gives its `surface` a 100 % `ConstantR` coating through
   `set_coating(SurfaceGeometry::FACE, ..)`, which rebuilds as well.

**Trace.** `ThinMirror::analyze` (the `AnalysisRayTrace` implementation in `nodes/thin_mirror.rs`) uses neither the
volume helper nor the single-surface helper:

1. It takes the first input and output names from `self.ports()`.
2. It calls `interact_with_rays(face, &mut rays, &Refraction(None), ReflectedPart::Main, missed strategy)` on its
   runtime geometry directly, with the face under its input port (`face_of_port`): a passive surface whose
   reflected part is the intended beam.
3. It emits only the reflected bundle at the output port. Outside the alignment run it first drops rays below the
   energy threshold.

A ray that misses the mirror is lost in every analysis, because only reflected light leaves it. In the alignment run
the next node is placed along the reflected axis. If the axis misses the mirror, the run stops with an error that
names the mirror.

**Draw.** The 3D view (`for_each_drawable` in `opossum_scene/src/components.rs`) looks up each node's appearance by
its type (`Appearance::of_node_type`) and passes over a node whose type has none. A type missing from the look table
is listed as skipped, with that as the reason. It meshes the node's geometry in the
node's own frame (`OpticNodeExt::geometry_mesh`) and puts the mesh at the node's current placement
(`effective_node_iso`), the same placement its surfaces are traced at. Every kind of geometry meshes itself
(`GeometryKind::mesh`):

- The mirror lays its face over the outline of its rim, measured along the axis of the rim. A surface without a rim
  is drawn over the default optic shape around its vertex.
- A volume node lays both faces over the outline of its rim and closes the body with the lateral face between them,
  and with the wall of its bore if it has one (see [Bores](#bores)).

The patches of a solid form a closed shell and are drawn once. A single surface has no far side of its own, so each of
its patches is drawn a second time, turned over. Glass takes its refractive index from the material of the solid and
its thickness from the extent of the solid.

Every patch carries the face it lies on (`FacePatch`) and is drawn in the node's appearance under the coating of that
face (`Appearance::coated`, read from the face's `SurfaceConfig`). The share the coating reflects at normal incidence
makes a glass or a reflecting face that much metallic, and a reflecting one that much opaque; a mirror's bare surface
is a pale pane. Other finishes keep their look, because those nodes do not act through their coating. The lateral face
of a solid has no coating and is drawn as the appearance asks. Glass is drawn at an opacity of 0.7 (`GLASS_OPACITY` in
`opossum_scene`), rising to 1 the more its face is mirrored, because three.js shows only opaque objects through
transmissive glass (see the 3D view reference).

Neither reads the runtime surfaces, so a property that was changed without a rebuild is drawn as it is now. A single
component's file (`glb_of_node` in `opossum_scene`) is therefore meshed without rebuilding the node, and its endpoint
in `opossum_backend/src/document.rs` answers "not found" for a node that is not drawn.

### Other components

| Node | What differs |
|---|---|
| Parabolic mirror | `Geometry::surface` with an axis other than the identity. The node sits where the chief ray hits the mirror. `calc_off_axis_frames` returns two frames: the frame of the parent parabola's axis through the node, in which the optic shape is measured, and the vertex of the parent parabola, stated in that frame. |
| Beam splitter | One plane, face 0, under four port names: its surface ports put `input_1` and `input_2` and the outputs `out1_trans1_refl2` and `out2_trans2_refl1` all on `SurfaceGeometry::FACE`, so all four share the one optic surface, and declare that face splitting, so its `surface` takes only a `Separator`. Its analysis addresses the ports by name itself and splits with `Refraction(None)` and `ReflectedPart::Branch`, so the coating of its `surface` is the split (by default a `Separator` of 50 %). Its positioning splits the axis with `SplittingConfig::Ratio(0.5)` as the effect, whatever the coating. The second input enters mirrored on the plane (`axis_entrance_frame`). |
| Bored mirror | `Geometry::singlet` with a flat rear face: an `Extruded` plate whose outline has a bore. Its surface ports put `input_1` and `out2_trans2_refl1` on the front face and `input_2` and `out1_trans1_refl2` on the rear face, and declare no face splitting. Ray tracing, positioning and ghost focus are those of the beam splitter (`Splitter`), with the bore deciding what passes (see [Bores](#bores)). Its energy analysis is an error. |
| Detectors | `Geometry::detector` gives the extent window. A window is no edge: the face reaches everywhere, so all light passes. `SurfaceHit::recorded` decides which hit points are recorded, and `record_within_window` cuts the recorded light to the window of the geometry. With an open optic shape there is no window, and everything is recorded. |
| Ideal filter, paraxial surface | `Geometry::plane`. Their effects (`FilterType`, `ParaxialLens`) attenuate or deflect at the hit, so a ray or helper ray that misses is neither. The paraxial surface may be unbounded. |
| Grating | `Geometry::plane`. Its effect, `Diffraction`, sends the whole energy into one order, and the diffracted part is the intended beam (`ReflectedPart::Main`), like the reflection of a mirror. |
| Source, dummy, ray propagation visualizer | `Geometry::plane`. Source and visualizer are unbounded. A dummy is bounded only if it is given an optic shape, and then acts as an iris. None of them has an appearance, so none is drawn. A source declares only `output_1`. |
| Group, reference | `create_geometry()` returns `None`, the surface ports are `SurfacePorts::NONE`, and `update_surfaces` does nothing. The nodes inside a group have their own geometry. A reference is traced through the node it refers to. |

### One analysis run, in order

A ray-tracing analysis (`RayTracingAnalyzer::analyze`) runs these steps:

1. `NodeGroup::rebuild_surfaces` calls `update_surfaces` on every node, recursing into groups, so the trace sees the
   properties as they are now.
2. `calc_node_positions` runs with `RayTraceConfig::for_positioning`:
   - the missed-surface strategy is fixed to `Stop`;
   - `positioning_run` is true, so there is no energy threshold and no medium;
   - every node that gets placed rebuilds its surfaces through `set_positioning`;
   - if the axis ray leaves a node invalid and the node has successors, the run stops with an error naming the node.
3. `reset_data` clears light data, the caches and hit maps of the optic surfaces, and the inversion.
4. `prepare_volume` makes the volume nodes rebuild their surfaces and media, and builds the inversion where the gain
   model needs one (see Example 1, Step 4).
5. `AnalysisRayTrace::analyze` performs the trace itself.

A ghost-focus analysis (`GhostFocusAnalyzer::analyze`, `analyzers/ghostfocus/analyzer.rs`) runs the same first four
steps. Its alignment run uses `AnalyzerType::positioning_config`: a plain ray-trace configuration with the
analysis's sources. Then the bounce passes follow:

- Even passes run forward.
- Odd passes run with the scenery inverted. `OpticGraph::analyze_ghostfocus` then reverses the graph for the pass
  through an `InvertGraphGuard`. Surfaces are not rebuilt.
- A ghost reflected in one pass is cached on the optic surface it came from and re-enters at that surface in the
  next pass.

## Part 3: Where to change what

- **A new node type.**
  - Implement `OpticNode::create_geometry` with a preset. The method has no default, so the compiler forces an
    answer; `None` is the answer for a node without a shape of its own.
  - Implement `OpticNode::surface_ports`: the port names of the node, each with the number of the face it sits on,
    and the faces at which the node follows both parts of the light (`splitting`, usually none).
    `SurfacePorts::SINGLE_SURFACE` and `SurfacePorts::THROUGH_SOLID` cover the usual cases.
  - In `Default`, declare the `OPTIC_SHAPE` property and the properties the geometry is derived from, then call
    `create_default_surface_properties` before anything installs the surfaces (`update_surfaces`, `set_positioning`,
    `set_coating`), as `ThinMirror::default` does.
  - Add the registered name of the node type to the look table `LOOKS` in `opossum_scene/src/appearance.rs`: an
    `Appearance` preset, or `None` for a node that is not drawn. The test `every_registered_node_type_has_a_look`
    fails until it is there.
  - For a solid, pass the material (`OpticNodeExt::material`) to the preset. Whether the node has a medium follows
    from its geometry; a volume node analyzes through `unified_analyze_volume_node` and its ghost focus variant.
- **A new preset**, such as another kind of lens: add a constructor on `Geometry` next to `singlet` that builds an
  `Extruded` or a `SurfaceGeometry`. Nothing downstream changes.
- **A new effect at a surface.** Implement `SurfaceEffect` next to the type that defines it, as the filter and the
  grating do, and pass it to `interact_with_rays` or `pass_through_surface_generic`. The trait has no default
  methods, and the loop in `Geometry::interact_with_rays` stays as it is.
- **A new surface shape.**
  - Add a type for it in a file of its own in `geometry/surface/`, like `Sphere`, and implement `GeoSurface` for it
    in its vertex frame. Its `local_z_at` has to describe the sheet through the vertex: `is_on_vertex_sheet`, and with
    it every hit test, is derived from it. Its `local_normal_at` gives the normal there; the 3D view lays the face
    over an outline with both (`Face::mesh_over`).
  - Add a variant to `SurfaceShape` in `geometry/surface/mod.rs`. The compiler then points at `as_geo_surface`,
    `matches` and `depends_on_roll`.
- **A node made of a parametric solid or an assembly.** `Geometry::move_by`, `GeometryKind`, `Body` and
  `install_geometry` accept every kind. A node only has to return such a geometry from `create_geometry()` and name
  the face of each port in its surface ports; an assembly numbers the faces of its parts one after the other. The limits are listed under
  [Three kinds of solid](#three-kinds-of-solid): no bounding box for a parametric solid or an assembly yet, and no
  face that is a whole closed surface.
- **A new kind of solid.** Add a file in `geometry/solid/` with the type, holding an `OpticSurface` per face, and its
  implementations of `GeometryKind` and `Body`, a variant in `Geometry`, and its arms in `Geometry::kind` and
  `Geometry::kind_mut`. `GeometryKind::mesh` is part of it: a kind that cannot be drawn yet returns an error, as the
  parametric solid does. Nothing else changes.
- **Where a face ends.** To change how a kind decides it, change its implementation of `GeometryKind::includes`.
  Surfaces, the body and the analyzers ask nothing else.
- **Who reads what.**
  - Read the geometry directly: `install_geometry` and the 3D view.
  - Read the runtime geometry, its surfaces and its medium: all analyzers and `OpticNodeExt::effective_surface_iso`,
    the frame of the face under a port in the runtime geometry (used by the paraxial surface, the grating, the source,
    the spot diagram and the wavefront monitor).
  - `install_geometry` is the only place that installs a runtime geometry; the surfaces and the medium are part of it.
  - The coating and the LIDT of a face are read from its surface property only when the geometry is installed.
