# Building a lens

This chapter builds a real optical component — a biconvex lens — to show how patches,
analytic normals and instancing fit together. The crate ships this as
`fixtures::biconvex_lens`; here we look at how it is constructed and how to place it.

## The shape

A biconvex lens on the z-axis is three surfaces:

- a **front** spherical cap (bulging toward −z),
- a **back** spherical cap (bulging toward +z),
- a **cylindrical edge** joining their rims.

Each becomes its own `SurfacePatch`, so the rim stays a hard edge. Because the caller knows
the surface equations, each patch carries **exact analytic normals** — for a sphere of radius
`R` centered on the axis, the outward normal at a point `p` is simply `(p − center) / R`.

The *sag* (how far a cap bulges over its aperture) is
`R − sqrt(R² − aperture²)`, which is where each cap meets the cylindrical edge.

## Using the fixture

With the `fixtures` feature you get the whole lens in one call:

```rust
use optoscene::{fixtures, Material, Scene, SceneOptions};

let mut scene = Scene::new(SceneOptions::default());

let glass = scene.add_material(Material::Glass {
    color: [0.9, 0.95, 1.0],
    ior: 1.5,
    thickness: 0.01, // center thickness, meters
});

// biconvex_lens(diameter, center_thickness, r1, r2, segments, material)
let lens = scene.add_mesh(fixtures::biconvex_lens(0.05, 0.01, 0.1, 0.1, 48, glass))?;
```

`segments` is the azimuthal/radial subdivision count (clamped to ≥ 3); higher is smoother.
The returned `TriMesh` has exactly three patches, all sharing the `glass` material.

## Placing and instancing it

A registered mesh is placed by adding nodes. Add the **same** `MeshId` under two nodes and
you get instancing — one glTF mesh, two placements:

```rust
use nalgebra::Isometry3;
use optoscene::{Layer, SceneNode};

for (uid, z) in [("lens-1", 0.00), ("lens-2", 0.05)] {
    scene.add_node(SceneNode {
        uid: uid.to_string(),
        name: uid.to_string(),
        mesh: Some(lens),
        transform: Isometry3::translation(0.0, 0.0, z),
        layer: Layer::Optics,
        data: None,
    })?;
}
```

Because meshes are content-deduplicated, you would get the same single glTF mesh even if you
called `biconvex_lens(...)` twice with identical arguments and added each result — but reusing
the `MeshId` is clearer and avoids rebuilding the geometry.

## Building your own component

When you tessellate your own surface instead of using the fixture, the pattern is the same:

1. One `SurfacePatch` per smooth surface (so seams stay sharp).
2. Fill `positions` from your parametrization, in meters, in the component's local frame.
3. Fill `normals` from the analytic surface normal (`∂f/∂u × ∂f/∂v`, normalized). Do **not**
   leave them out expecting the crate to generate them — it never does; omitting them only
   gives flat shading.
4. Fill `indices` with the triangle winding that faces outward.
5. Pick a `Material` (`Glass` for refractive elements, `Mirror` for reflective, `Opaque` for
   mounts).

The complete, runnable version of this — a lens plus one object per material type — is the
`export_basic` example:

```bash
cargo run -p optoscene --example export_basic --features fixtures
```

Next: [Rays](./rays.md).
