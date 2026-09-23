# Vendored three.js

## Version

**r174** — npm package `three@0.174.0`

## Source

```
npm pack three@0.174.0
```

File: `three-0.174.0.tgz` (sha1: see `npm pack --dry-run three@0.174.0`)

## File list (relative to `assets/three/`)

| Target file | Source in the npm package |
|---|---|
| `LICENSE` | `package/LICENSE` |
| `three.module.js` | `package/build/three.module.js` |
| `three.core.js` | `package/build/three.core.js` |
| `examples/jsm/controls/OrbitControls.js` | `package/examples/jsm/controls/OrbitControls.js` |
| `examples/jsm/loaders/GLTFLoader.js` | `package/examples/jsm/loaders/GLTFLoader.js` |
| `examples/jsm/utils/BufferGeometryUtils.js` | `package/examples/jsm/utils/BufferGeometryUtils.js` |

## Why exactly these files?

- `three.module.js` is the ESM build, loaded in `viewer.js` via `import * as THREE from "three"`.
- `three.core.js` is imported by `three.module.js` via `import ... from './three.core.js'`.
- `OrbitControls.js`, `GLTFLoader.js`, and `BufferGeometryUtils.js` are add-ons required
  internally by `GLTFLoader.js`
  (`import { toTrianglesDrawMode } from '../utils/BufferGeometryUtils.js'`).
  All are addressed via `three/addons/...` (through the importmap).

## Why a folder asset (not individual `asset!()` calls)?

`asset!()` hashes file names (e.g. `three.core-a1b2cd.js`). `three.module.js` contains the
literal import `import ... from './three.core.js'` — after hashing that import would 404.
`AssetOptions::folder()` (manganis) is **unhashed and structure-preserving**
(`manganis-core/src/folder.rs:51-59`), so all relative imports stay intact. As a side
effect the path `/assets/three/...` is constant and can be hardcoded in the importmap —
important for the fallback snippet when runtime injection fails.

## Upgrade recipe

```bash
# 1. Find the new version
npm info three version   # e.g. 0.175.0

# 2. Unpack
npm pack three@0.175.0
tar -xzf three-0.175.0.tgz -C extracted/

# 3. Check internal imports (transitive dependencies may change)
grep -rn "from '" extracted/package/build/three.module.js extracted/package/examples/jsm/loaders/GLTFLoader.js

# 4. Copy files (same targets as above)
cp extracted/package/LICENSE assets/three/LICENSE
cp extracted/package/build/three.module.js assets/three/three.module.js
cp extracted/package/build/three.core.js   assets/three/three.core.js
cp extracted/package/examples/jsm/controls/OrbitControls.js    assets/three/examples/jsm/controls/
cp extracted/package/examples/jsm/loaders/GLTFLoader.js         assets/three/examples/jsm/loaders/
cp extracted/package/examples/jsm/utils/BufferGeometryUtils.js  assets/three/examples/jsm/utils/

# 5. Update the version line in VENDORING.md and commit
```

## Licence

three.js is released under the MIT licence (see `assets/three/LICENSE`).
The `dioxus_glb_viewer` crate is released under GPL-3.0 (compatible: MIT is permissive).
