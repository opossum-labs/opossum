# Demo assets

Place a `.glb` file named `sample.glb` here to enable the Add/Replace buttons in the demo:

```
examples/assets/sample.glb
```

Recommended sources (permissive licence, no registration required):

| Model | URL |
|---|---|
| `Box.glb` (simple box, ideal for initial testing) | https://github.com/KhronosGroup/glTF-Sample-Assets/tree/main/Models/Box |
| `DamagedHelmet.glb` (more complex model with textures) | https://github.com/KhronosGroup/glTF-Sample-Assets/tree/main/Models/DamagedHelmet |

Direct download (GLB variant):

```bash
# Box
curl -LO https://github.com/KhronosGroup/glTF-Sample-Assets/raw/main/Models/Box/glTF-Binary/Box.glb
mv Box.glb sample.glb

# DamagedHelmet
curl -LO https://github.com/KhronosGroup/glTF-Sample-Assets/raw/main/Models/DamagedHelmet/glTF-Binary/DamagedHelmet.glb
mv DamagedHelmet.glb sample.glb
```

Without `sample.glb` the demo still builds (`option_asset!` returns `None`) and shows a
notice banner. The desktop bytes path (the input field in the side panel) always works,
regardless of this file.
