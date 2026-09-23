# Vendored three.js

three.js is not fetched from a CDN at runtime and not pulled in by a build step. It is
checked into the repository under `assets/three/`, and the component loads it from there.

The version in the tree is **r174** (npm `three@0.174.0`), which `three.core.js` confirms
in its own `REVISION` constant.

## What is vendored

Six files, copied unmodified from the npm package:

| File in `assets/three/` | Source in the package |
|---|---|
| `LICENSE` | `package/LICENSE` |
| `three.module.js` | `package/build/three.module.js` |
| `three.core.js` | `package/build/three.core.js` |
| `examples/jsm/controls/OrbitControls.js` | `package/examples/jsm/controls/OrbitControls.js` |
| `examples/jsm/loaders/GLTFLoader.js` | `package/examples/jsm/loaders/GLTFLoader.js` |
| `examples/jsm/utils/BufferGeometryUtils.js` | `package/examples/jsm/utils/BufferGeometryUtils.js` |

## Why exactly these files

`viewer.js` imports three of them directly — the ESM build, `OrbitControls` and
`GLTFLoader`. The other two are there because the first three need them:

- `three.module.js` imports and re-exports from `./three.core.js`, in the same directory.
- `GLTFLoader.js` imports `../../../three.module.js` and
  `../utils/BufferGeometryUtils.js`, the latter for `toTrianglesDrawMode`.
- `OrbitControls.js` imports `../../../three.module.js`.

Nothing else from the package is reachable from those entry points, which is why the list
is this short. It is also why the list has to be rechecked on every upgrade: a new release
can add an internal import, and the missing file only shows up as a 404 at runtime.

## Why a folder asset

The directory is declared as a **folder asset**, not as individual `asset!()` calls, and
that is load-bearing.

`asset!()` hashes filenames — `three.core.js` would be served as something like
`three.core-a1b2cd.js`. But the import inside `three.module.js` is the literal string
`'./three.core.js'`, and the browser resolves it verbatim. After hashing it would 404.
The same goes for `'../../../three.module.js'` in both addons: those paths only work if the
directory structure survives intact.

A folder asset is **unhashed and structure-preserving**, so every relative import inside
the vendored tree resolves exactly as it did in the npm package.

## How the module finds it

There is **no importmap.** The boot script resolves the folder asset to a URL and passes it
to `createViewer` as `threeBase`, which then builds each import URL by concatenation.

This is worth stating plainly because an importmap is the conventional answer to bare
specifiers like `three` and `three/addons/...`, and the crate deliberately does not use
one: since the vendored files import each other *relatively*, knowing the base directory is
sufficient, and an explicit parameter cannot get out of sync with what the asset pipeline
produced. If you are reading the older note in `assets/VENDORING.md` that mentions an
importmap and a fallback snippet, that note predates the current loader and no longer
matches the code.

## Upgrading

```bash
# 1. Pick the version
npm info three version

# 2. Unpack it
npm pack three@0.175.0
tar -xzf three-0.175.0.tgz -C extracted/

# 3. RECHECK the internal imports — this is the step that matters
grep -rn "from '" extracted/package/build/three.module.js \
                  extracted/package/examples/jsm/loaders/GLTFLoader.js \
                  extracted/package/examples/jsm/controls/OrbitControls.js

# 4. Copy the files to the same targets as the table above
cp extracted/package/LICENSE                                    assets/three/
cp extracted/package/build/three.module.js                      assets/three/
cp extracted/package/build/three.core.js                        assets/three/
cp extracted/package/examples/jsm/controls/OrbitControls.js     assets/three/examples/jsm/controls/
cp extracted/package/examples/jsm/loaders/GLTFLoader.js         assets/three/examples/jsm/loaders/
cp extracted/package/examples/jsm/utils/BufferGeometryUtils.js  assets/three/examples/jsm/utils/

# 5. Update the version in this chapter and in assets/VENDORING.md
```

Step 3 is not optional. If the new release imports a file that is not vendored, everything
compiles and the viewer fails at runtime with a module-import error reported as
`ViewerEvent::Error`.

After upgrading, run the [manual acceptance script](./testing.md#the-manual-acceptance-run):
the renderer has no automated test coverage, so a three.js upgrade is verified by using it.

## Licence

three.js is MIT licensed; the licence text travels with the vendored files in
`assets/three/LICENSE`. The crate itself is GPL-3.0, and MIT is permissive, so the
combination is fine — but keep the `LICENSE` file in place, since it is the condition on
which redistribution rests.

## Compression extensions

Draco, KTX2 and Meshopt decoders are **not** vendored, which is why compressed meshes are
unsupported. Adding support means vendoring separate WASM blobs and wiring the
corresponding loader plugin — a larger decision than a file copy, and the reason it has not
been made by default.

Next: [Testing and checks](./testing.md).
