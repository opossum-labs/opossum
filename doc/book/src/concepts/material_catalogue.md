# Material Catalogue & Asset Management

OPOSSUM manages optical materials (and future registerable assets such as coatings) through a **dual-catalog registry system** that separates shared upstream definitions from user-created or customized materials. Every asset is uniquely identified by a UUID and tracked with an append-only version history (v1, v2, …), ensuring that optical models remain reproducible while still allowing controlled updates across projects.

## Dual-Catalog Architecture (`Official` vs. `User`)

On disk, the asset registry root directory (configured in the application settings) is divided into two independent catalog repositories, each initialized on its own Git `main` branch:

```
<catalog_root>/
├── official/          # Shared upstream OPOSSUM catalog (synchronized via Git, read-only)
│   ├── .git/
│   └── materials/
│       └── <uuid>/
│           └── v1.ron
└── user/              # Local user catalog (custom & forked materials, local Git commits)
    ├── .git/
    └── materials/
        └── <uuid>/
            ├── v1.ron
            └── v2.ron
```

### Official Catalog (`official/`)

* **Single Point of Truth:** The `official` catalog contains curated standard materials (e.g., Schott, Ohara, Corning glasses) synchronized from the shared remote repository (`origin/main`).  
* **Read-Only Protection:** Official assets cannot be modified in place or deleted—neither through the graphical user interface nor through the `opossum_registry` library API. Attempting to delete an official asset or publish a user asset carrying an official UUID is explicitly rejected.  
* **Git Synchronization:** Pressing **Pull Official Catalog** inside the **Asset Catalog Synchronization** dialog fetches `origin/main`, fast-forwards or aligns the local `main` branch to the remote state, and prunes files that were removed or migrated upstream.

### User Catalog (`user/`)

* **Local Workspace:** All newly created materials, forked official materials, and materials imported from `.opm` documents are stored in the `user` catalog.  
* **Automatic Local Versioning:** Whenever a material is published to the `user` catalog, OPOSSUM writes the new version file (`v1.ron`, `v2.ron`, …), updates the in-memory search index, and records a local Git commit on the `user` repository's `main` branch.

## Browsing, Searching, and Filtering

The **Material Catalog** dialog lists all indexed materials from both the `official` and `user` catalogs in a unified table:

* **Catalog Badges:** The **Catalog** column clearly distinguishes `Official` materials from `User` materials.  
* **Text Search:** **Search Name / Manufacturer** filters entries case-insensitively by material name, manufacturer, or description.  
* **Refractive Index Bounds:** **Min n\_d** and **Max n\_d** filter materials by their refractive index at the yellow sodium D-line (λ ≈ 587.56 nm).

## Asset Lifecycle: Creating, Forking, Versioning, and Deleting

### Creating a New Material

Pressing **New Material** opens the **Material Editor** with an unpublished draft (`v0`) and a newly generated UUID. After configuring the general metadata (name, manufacturer, description) and physical properties (dispersion/refractive index, absorption, thermal, and mechanical properties), pressing **Publish to User Catalog** writes the material as **`v1`** into the user catalog.

### Editing an Official Material (Automatic Fork)

Official materials cannot be overwritten. Clicking the **Edit** (pencil) button on an `Official` material calls AssetRegistry::prepare\_user\_draft, which:

1. Loads the selected official material.  
2. Assigns a **brand-new UUID** so it never collides with the upstream official entry.  
3. Resets the draft version to `v0`.

When saved in the **Material Editor**, the modified material is published as **`v1`** in the `user` catalog. Both entries remain distinguishable in the catalog table—the original entry stays marked as `Official`, while the forked entry appears with a `User` badge.

### Editing a User Material (Incremental Versioning)

Clicking the Edit (pencil) button on a User material does the following:

1. Loads the latest version of the user material.  
2. Preserves its existing UUID.  
3. Resets the working draft version to v0.

When saved, OPOSSUM automatically assigns the next sequential version number (**`v2`**, **`v3`**, …) under the same UUID without overwriting previous versions on disk.

### Deleting Material Versions

Only materials in the user catalog display a Delete (trash) button:

* Deleting removes only the **latest version** (`vN`) from the `user` catalog and reverts the index entry to `vN-1`.  
* If `v1` is the only remaining version, confirming the deletion removes the material completely from the `user` catalog.  
* `Official` materials never expose a delete action and are protected against deletion in the backend registry.

## Using Materials in Optical Nodes

Volume components (such as lenses, cylindric lenses, and wedges) store their material property directly in the optical document (`AssetRef::Inline`). This makes `.opm` files self-contained and portable across machines, even if a recipient has not synchronized the same catalog version yet.

In the Node Editor, a component's material can be managed in three ways:

* **Select from Catalog:** Opens the **Material Catalog** in selector mode (`Select` button) to assign the latest version of an `Official` or `User` material to the node.  
* **Edit Locally (Ad-Hoc):** Edits the node's inline material directly. Local, uncataloged edits carry version **`v0` (`Ad-Hoc`)** until explicitly published to the `user` catalog.  
* **Re-sync Later:** Because each inline material retains its catalog **UUID** and **version number**, OPOSSUM can compare the model against both catalogs at any time.

## Asset Catalog Synchronization (`Sync with Model`)

Clicking **Sync with Model** in the Material Catalog opens the **Asset Catalog Synchronization** dialog. It scans all optical nodes in the active document, extracts their inline materials, and compares each material's UUID and version against both the official and user catalog indices.

| Status | Origin | Condition | Available Action |
| :---- | :---- | :---- | :---- |
| **Up to date** | `Official / User` | Model version equals latest catalog version (v\_model \= v\_cat ≥ 1). | None (*In Sync*). |
| **Update available** | `Official / User` | Catalog has a newer version than the model (v\_cat \> v\_model). | **Update in Model** (or **Update All**) updates all referencing nodes to v\_cat. |
| **Pull Official Catalog** | `Official` | Model references a higher version of an official UUID than cached locally (v\_model \> v\_official). | **Sync Official** pulls the latest commits from `origin/main`. |
| **New for User Catalog** | `Unlisted` | Versioned material (v ≥ 1\) is not present in either local catalog. | **Add to User Catalog** (or **Import All to User**) imports it into `user/`. |
| **Ad-Hoc Draft** | `Unlisted` | Local unversioned material (v \= 0\) not yet in any catalog. | **Add to User Catalog** opens the Material Editor to review and publish as `v1`. |
| **User Conflict** | `User` | UUID exists in `user/`, but model version/state diverges from the user catalog. | **Resolve Conflict** opens the conflict resolution dialog (see below). |

### Resolving User Catalog Conflicts

When a model material shares a UUID with an existing entry in the `user` catalog but has a conflicting version or state, clicking **Resolve Conflict** offers three choices:

1. **Overwrite (New Version):** Publishes the model's material as the next sequential version (`vN+1`) under the existing UUID in the `user` catalog and updates all referencing nodes in the model.  
2. **Save as New (Rename):** Clones the material with a **new UUID**, assigns the entered name, publishes it as `v1` in the `user` catalog, and updates the referencing nodes in the model to point to the new UUID.  
3. **Cancel:** Leaves both the `user` catalog and the optical model unchanged.

## Developer Notes (`opossum_registry`)

For developers extending OPOSSUM or writing Rust tools against `opossum_registry`:

* **`AssetRegistry<T>`:** Central facade holding `official_loader` / `official_index` and `user_loader` / `user_index`. Lookup methods (`is_official`, `in_user_catalog`, `load`) treat `official` as the primary source of truth by UUID.  
* **Enforced Invariants:**  
  * `publish_to(CatalogSource::User, &mut asset)` rejects any asset whose UUID exists in `official_index`.  
  * `delete_latest_version_from(CatalogSource::Official, id)` always returns an `OpossumError::Registry`.  
  * `prepare_user_draft(id, version)` encapsulates the fork-vs-increment rule (`Official` → new UUID \+ `v0`; `User` → existing UUID \+ `v0`).  
* **Failsafe Git Integration (`RegistrySync`):** Repository operations use pure-Rust `gix` on branch `main`. `open_registry_repo` automatically falls back to `gix::open_opts` with `Options::isolated()` if a user's global or system `.gitconfig` contains syntax errors or platform-specific paths that fail strict parsing.
