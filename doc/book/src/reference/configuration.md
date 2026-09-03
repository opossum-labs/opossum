# Application Settings

The **Settings** dialog configures application-wide preferences, directory locations, catalog synchronisation, and default physical parameters[cite: 16]. Settings persist across sessions in a local configuration file (`config.ron`)[cite: 2].

## Accessing the Settings Dialog

Open the settings dialog from the top navigation bar[cite: 16]:

* Navigate to **File** → **Settings** (or press Ctrl+Alt+,).

Changes are staged in a temporary buffer while the dialog is open[cite: 16]. Clicking **Cancel** discards all changes, while **Save & Close** validates and writes the configuration to disk[cite: 16].

---

## Settings Reference

The dialog is organised into two categories: **General** and **Physics / Model**[cite: 16].

### General Settings

Configures filesystem directories and external asset synchronisation[cite: 16].

| Setting | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| **Report Base Directory** | Filesystem path | `~/Documents/opossum_reports` | Directory where simulation runs and generated analysis reports (PDF, CSV, diagrams) are stored[cite: 2, 16]. |
| **Catalog Directory** | Filesystem path | `<local_data>/catalogs` | Directory for local material and coating catalog registries[cite: 2, 16]. |
| **Catalog Git Remote URL** | URL / String | `https://github.com/opossum-labs/opossum_catalog.git` | Remote Git repository used to pull and update optical material definitions[cite: 2, 16]. |
| **Synchronise Catalog on Startup** | Boolean | `false` | When enabled, OPOSSUM checks the remote catalog URL and pulls updates on application startup[cite: 2, 16]. |

* **Directory Selectors (`Browse...`)**: Spawns a native directory picker to select valid folders on the filesystem[cite: 16].

---

### Physics / Model Settings

Configures global optical defaults applied when creating components, sources, and analyzers[cite: 16].

| Setting | Unit / Type | Default | Constraints | Description |
| :--- | :--- | :--- | :--- | :--- |
| **Default Wavelength** | Length (`m`, SI prefixed: `nm`, `µm`, etc.) | `1053.0 nm` | Positive, finite, non-zero (`AllPositive && AllFinite && AllNotZero`)[cite: 2] | Nominal wavelength used to initialise optical sources and analyzers[cite: 16]. |

#### Behavior and Validation

* **Validation Rules**: Wavelengths must be strictly positive and finite[cite: 2]. Non-numeric values, negative numbers, zero, and infinite values trigger an inline validation error and prevent saving[cite: 2, 16].
* **Analyzer Instantiation**: Newly created analyzers adopt this default wavelength and initialise their spectral distributions and laser line definitions accordingly[cite: 1, 12].
* **Source Port Mapping**: When a new optical `SourcePort` node is added to the optical graph, analyzers map the source port using this default wavelength[cite: 13, 14].
* **Distribution Editors**: Switching spectral distribution types in the node editor (e.g. from Gaussian to Laser Lines) seeds new entries with this value[cite: 1].

---

## Configuration File & Storage

All settings are serialized using the [Rusty Object Notation (RON)](https://github.com/ron-rs/ron) format and saved to `config.ron`[cite: 2].

### Storage Paths by Operating System

| Operating System | File Path |
| :--- | :--- |
| **Linux** | `~/.config/opossum/config.ron` |
| **Windows** | `%LOCALAPPDATA%\Opossumlabs\Opossum\config\config.ron` |
| **macOS** | `~/Library/Application Support/org.Opossumlabs.Opossum/config.ron` |

### File Format Example

```ron
(
    report_dir: Some("/home/user/Documents/opossum_reports"),
    catalog_dir: Some("/home/user/.local/share/opossum/catalogs"),
    catalog_remote_url: "[https://github.com/opossum-labs/opossum_catalog.git](https://github.com/opossum-labs/opossum_catalog.git)",
    sync_catalog_on_startup: false,
    default_wavelength: 0.000001053,
)
