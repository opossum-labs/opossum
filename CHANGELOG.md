# Changelog

All notable changes to this project will be documented in this file.

OPOSSUM is funded through THRILL (EU, grant agreement No 101095207) and LASE-FUSE (BMFTR, funding reference 13F1041); see [Funding & Acknowledgments](doc/book/src/concepts/background/funding.md). To cite OPOSSUM, use [`CITATION.cff`](CITATION.cff).

## [0.7.0] - 2026-02-18

### Bug Fixes

- :bug: Auto Setting of source position removed in GUI
- :bug: Fix subtle but evil bug while deleting nodes in the graph.
- :bug: Fix wrong default value in curvature editor.
- :bug: Fix error in ray tracing with unconnected beam splitters.
- :bug: Fix multiple alert dialogs on save and quit.
- :bug: Show a warning in the report if ray propagation detector is used together with energy analysis.
- :bug: Analyzer selection now shows config menu directly, again
- :bug: Fix evil bug while doing beam splitting.
- :bug: Fix loading spectra from file
- :bug: Fix error while plotting wavefront with one zero size dimension.
- :bug: Do not show input port for source nodes
- :bug: Prevent adding two laser lines with the same wavelength to a spectrum.
- :lock: Fix possible attack through malicious opm file.
- :bug: Do not allow duplicate laser lines for Energy LightData.
- :bug: Fix Hitmap not respecting aperture during analysis.
- :bug: Node config menu is reactive again
- :bug: Avoid that multiple analyses overwrite report data
- :bug: Unit parsing is now nore robust with whitespace
- :bug: Fix xtask bundle to support arbitrary target path locations
- :bug: Inputs now more permissive on input and revert only to old value on submission
- :bug: Fix focus issues while closing simulation window
- :bug: Oninput check for units and numbers removed. Only check on submission
- :bug: Wrongly displayed "m" unit in laserline removed
- :bug: Fix unnecessary updates in NodeEditor.
- :bug: Fixed file input for image field
- :bug: Do not delete report files before a simulation (security reasons)
- :bug: New downstream values now overwrite Unit-Input state signal

### Documentation

- :memo: Improve README.md

### Features

- :sparkles: Log-window now resizable. New logs on top
- :sparkles: Center graph after loading
- :sparkles: Sort list of available node types.
- :sparkles: Add additional check for duplicate UUIDs in OpticGraph::add_node
- :sparkles: Add shortcut (Alt+S) for starting a simulation.
- :sparkles: Add menu entry "Exit" and tooltip for Simulate button.
- :sparkles: Calculation node positions does no longer stop with error if a node has no successor nodes (pure sink)
- :sparkles: Automatically determine spectrum resolution for given ray bundles using the Freedman-Diaconis rule.
- :sparkles: Improve UI of simulation window.
- :sparkles: Do not show stale nodes in analysis report but show a warning.
- :sparkles: Add shortcut (ESC) for closing simulation window.
- :sparkles: Implemented Unit Inputs to display SI Units including prefixes
- :sparkles: Unit Inputs for all inputs with SI units
- :sparkles: Add refractive index model for air
- :sparkles: Order of nodes in report follows topology now.
- :sparkles: Double center-mouse click now also zooms to fit the graph

### GraphEditor

- Change node_editor_command to ReadSignal

### Miscellaneous Tasks

- :arrow_up: Upgrade to dioxus 0.7.0.2 and some linting.
- :construction_worker: Remove no longer necessary files.
- :wastebasket: Remove no longer necessary fscript files.

### Performance

- :zap: Improve memory allocation management during ray tracing.
- :zap: Dramatically improve performance of report generation for RayPropgationVisualizer

### Refactor

- :recycle: Use invetory crate for node type registration
- :construction_worker: Refactor MenuBar and App components with improved signal adn event handler usage
- :bookmark: Update dependencies.
- :recycle: refactor of AlertDialog component.
- :recycle: Refactor context menu structure
- :recycle: use inventory crate for dynamic  extensibility of analyzer types.
- :recycle: Refactor node energy analysis code to reduce code duplication.
- :recycle: Use report notes to show warnings in RayPropagationVisualizer
- :recycle: Refacotr RefractiveIndex module.
- :art: Added NodeConfigPlainF64Input to consistently format f64 inputs
- :art: Made more useful error message for Air-Model
- :recycle: Get_bounding_box is now calculated in each node. Union method used to remove code

### Testing

- Add missing test for reflective grating and improve error messages.
- :white_check_mark: Improve test for Rays::energy_weighted_centroid
- :white_check_mark: Extend unit tests for sobol and guassian distribution as well as the cynlinder surface.
- :white_check_mark: Improve and extend tests for energy distribution functions.
- :white_check_mark: Add missing unit tests for various aperture functions.

## [0.7.0.beta.3] - 2025-12-04

### Bug Fixes

- :bug: Fix several bugs in file handling.
- :bug: Fix handling of "Add Analyszer" and "Add Nodes" submenues
- :bug: OpticalNodeEditor now re-renders again by using use_reactive on the node_id input
- :bug: Add missing parameter validation in spectral_distribution::Gauss function
- :bug: No panic if negative value is set for Gaussian spectrum number
- :bug: Backend is terminated before dropping App to avoid crash on closing
- :bug: All report sections are collapsed by default (execpt for Ghost Focus Analysis)
- :bug: Disable unecessary build of opossum_core DLL
- :sparkles: Fixed file modification handling while selecting "New Project"
- :bug: added validation for curvature and refractive index values of lens
- :bug: Lidt now accepts only positive values
- :bug: Fix bug in nod drag event handling while migration to dioxus 0.7
- :bug: Fixed non-functioning asset loading for linux builds
- :bug: Fix file name display during parse error of a loaded model.
- :bug: Fix image link in README.md
- :bug: Source: Set LaserLines as default for spectral distribution instead of Gaussian
- :bug: Plots are omitted if triangulation is not possible, still finishing the simulation
- :bug: Wavefront plot-report is now skipped instead of creating an error
- :bug: If no plot can be produced a message is displayed

### Documentation

- :memo: Update toplevel README.md
- :memo: Added "First steps" and "Model geometry" to the book.
- :memo: Improve section on model geometry in the book.
- :memo: Improve documentation about modeling optical systems in the book.

### Features

- :sparkles: Copying of a nodes in the GUI is now possible even, if original node has been deleted
- :sparkles: Improve automatic placement of new nodes.
- :sparkles: added EnsureValidated Derive macro to recursively find all lack of validation in structs and enums
- :sparkles: Enable GUI to compile as WASM app (Saving and loading does not work yet).
- :arrow_up: Update rust-sugiyama crate.
- :arrow_up: Update various crate dependencies.

### Miscellaneous Tasks

- :construction: add the playground folder to gitignore.
- Bump version to 0.7.0.beta.3 and prepare for release.

### Refactor

- :recycle: Remove no longer necessary reexports in opossum_backend
- :recycle: Refactor signal handling in GraphStore.

### Styling

- :lipstick: Improve rsx code formatting with dioxus 0.7

### Testing

- :white_check_mark: Add paramter check and missing test for LightFlow::set_distance.
- :white_check_mark: Add missing test to OpmDocument.
- :white_check_mark: Add missing unit tests for OpmDocument & AnalyzerInfo.
- :white_check_mark: Add missing unit test for OpticPorts
- :white_check_mark: Add tests for EnergyDataBuilder
- :white_check_mark: Add unit tests for LightDataBuilder

### Build

- :building_construction: Improve bundling of opossum_gui
- :building_construction: Add missing edition (2024) to toplevel Cargo.toml
- :building_construction: Slightly change default_members in toplevel Cargo.toml. Dixous 7.1 still cannot handle workspaces :-(
- :building_construction: Implement xtask process to build a bundle.

## [0.7.0-beta.2] - 2025-10-09

### Bug Fixes

- :bug: Fix not disappearing context menu.
- :bug: Fiex subtle layout bug in Help menu
- :bug: Fix missing deselction after delete node.
- :bug: Synchronize backend and frontend at start.
- :bug: Delete node also deletes the entire cascade of (nested) references.
- :bug: Setter function of gaussian spectral editor was falsely using millimeter! macro instead of nanometer! Fixed.
- :bug: Fix wrong assignment of cols and rows in fluence data calculation (Voronoi).
- :bug: Fix wrong placement of reference nodes after creation.
- :bug: Correct handling of connection dragging when leaving the editor window.
- :bug: Clicking on main graph window now also triggers changes that have not been sent to the server
- :bug: Improve visibility of scrollbar in "Add Node" menu.
- :bug: Helperrays fluence calculation bug fixed

### Features

- :sparkles: Implement better file handling with "Save", "SaveAs" and warnings about unsaved models on quit.
- :sparkles: Add prelude to opossum_core for easier use statements.
- :sparkles: Set global position of a source at origin by default.
- :sparkles: Introduction of several GUI shortcuts
- :adhesive_bandage: Avoid unnecessary position updates sent to the backend while dragging nodes.

### Miscellaneous Tasks

- :pushpin: Define workspace dependencies thus synchronizing crate versions over the entire project.

### Performance

- :zap: Implement performance benchmarks and slightly improve plane intersection code.
- :zap: Speed up intersection calculation for cylindric surfaces.
- :zap: Imrpove performance of ray intersection with a sphere.
- :zap: Improve performance of ray interserction with parabola and correct an edge case.
- :zap: NodeConfigEditor now only re-renders when node_id or type changes

### Refactor

- :recycle: Heavily refactor and streamline code for OpticGraph.
- :art: Reduces Active-Node tracking to a single field in graphstore

## [0.7.0-beta.1] - 2025-09-22

### Bug Fixes

- :bug: Fix corrupted dot file generation.
- :bug: Fix wrong calculation of the center wavelength of a Spectrum
- :bug: Fix broken POST scenery/{uuid}/nodes
- :bug: Fix wrong display of spectra. It's now displayed like a histogram.
- :bug: Fix Send & Sync problems in async GraphStore functions.
- :bug: Fix bug while dragging nodes with icon
- :bug: Wrong context use in Raytrace and Ghostfocus cinfig editors. Using Coroutine handle instead (#527)
- :bug: Do not display "Create reference" context menu for analyzer nodes.
- :bug: Fix missing messages at the end of a simulation run.
- :bug: Dragging now stops when mouse leaves the graph editor window (#542)
- :bug: Graph store now cleared on GUI side when loading opm file (#547)
- :bug: Set report directory before simulation run if not already set before.
- :bug: GUI now requesting actual uuid of top-level scenery instead of assumin nil
- :bug: Z-index on selection nodes working again
- :bug: keydown- mouseleave-and mouseenter-handler moevd to outer div to correct copy behavior
- :bug: Copy-mechanism of nodes now works reliantly
- :bug: Add window resize functions.
- :bug: Fix useless serialization if property validators.
- :rotating_light: Fix linter errors and unit tests.

### Documentation

- :memo: Reorganize documentation folder
- :memo: Update documentation for OpticGraph::delete_node
- :memo: Work on `the book`.
- :memo: Improve documentation
- :memo: Move announcements, blog posts, presentations to separate repository
- :memo: Add README.md for core and backend. Fix rustdoc warnings.

### Features

- :sparkles: Add function to delete a node from the model
- :sparkles: Add function to disconnect nodes in a NodeGroup
- :zap: Serialize only NodeAttr (and possibly OpticGraph) in an OpticRef.
- :sparkles: Implement webAPI endpoint for patching / updating node attributes.
- :sparkles: Improve OpticGraph::delete_node to recursively delete subnodes as well.
- :sparkles: Use Rust Object Notation (RON) for OPM files
- :sparkles: Add support for RayDataBuilder::PointSrc
- :sparkles: Save GUI position for analyzer and optical nodes in properties. Update backend accordingly.
- :sparkles: Implement RayDataBuilder::Image. This allows for geometric image analysis.
- :sparkles: Improve error message when connecting nodes.
- :sparkles: Add property `light data iso` to Source for alignment of light field.
- :sparkles: Implement webAPI call for getting available AnalyzerTypes.
- :sparkles: RayPlotVisualizer: Add property to configure the ray transparency in plots.
- :sparkles: Center nodes on double-click in editor.
- :sparkles: Implement zoom of node editor around mouse position.
- :sparkles: Show port name as tooltip
- :sparkles: Backend: add endpoint to add reference node
- :sparkles: Implement addding reference nodes in GUI through context menu.
- :sparkles: New added nodes are placed at the current view port center.
- :bug: (Re-I)implement handling of the z-level display of nodes.
- :sparkles: Backend: Add endpoint for terminating the server.
- :sparkles: Implement validators for properties
- :sparkles: Filter Editor added to GUI node-config menu (#505)
- ✨ Implemented a configuration meun for the beam splitter node
- :sparkles: Added littrow configurator for reflective gratings in GUI node config menu (#511)
- :sparkles: Implement starting simulation run from GUI.
- :sparkles: Add command line flag to suppress logo and version information in CLI
- :sparkles: Improve styling of distance between nodes.
- :sparkles: Copy and paste of optical nodes now implemented
- :sparkles: Added copy-past functionality for analyzer nodes in GUI
- :sparkles: Copied nodes are now inserted at current mouse position
- :sparkles: Close, Minimize, maximize and drag of main window working without decoration.

### Miscellaneous Tasks

- :bookmark: Bump version number to 0.7.0-beta.1

### Performance

- :zap: Do not serialize Aperture:None, which is the default
- :zap: Improve efficiency of bundling rays in wavelength groups.

### Refactor

- :recycle: Make OPOSSUM thread safe using Arc<Mutex>
- :recycle: Rename start_server to start for consistency
- :recycle: Let NodeGorupp::add_node() really own a node instead of borrow.
- :recycle: Use more stable Uuid insted of NodeIndex while referring to nodes in a graph.
- :recycle: Simplify the data structure of the optical model.
- :recycle: Move toplevel analyze function from (CLI) main to OpmDocument.
- :recycle: Remove no longer needed Proptype::OpticGraph
- :recycle: Remove no longer necessary EnumProxy struct.
- :recycle: Use Uuid instead of array index for analyzers in OpmDocument
- :recycle: Remove no longer used (and functioning) bevy code.
- :recycle: Move serialization fn of AnalysisReport from main to th module.
- :recycle: Improve serialization / deserialization of Isometry.
- :recycle: Move AxLims to its own module and major linting.
- :recycle: Get rid of DataEnergy (replace directly by Spectrum)
- :rotating_light: Fix linter warnings.
- :recycle: Remove / disable unnecessary package dependencies.
- :recycle: Refactor Properties::Set function to avoid a clone operation.
- :recycle: Various cleanups in SceneryEditor component
- :art: NodeConfigEditor now part of grapheditor (#523)
- :recycle: Streamline serialization and deserialization of OpticGraph
- :recycle: Streamline Serialization and Deserialization of OpticRef
- :recycle: Refactor some math util functions.
- :recycle: Rename the core library package folder (opossum -> opossum_core)
- :recycle: Refactor report generation code
- :recycle: Separate command line interface (CLI) from core library.
- :recycle: Refactor the internal structure of the Aperture module.
- :rotating_light: Fix various linter warnings due to new rust version

### Simulation

- Make writing to temporary file synchronous. Search for CLI at various locations on disk.

### Testing

- :white_check_mark: Extend and improve tests for OpticNode & PortMap
- :white_check_mark: Add further tests for backend
- :white_check_mark: Add missing tests to PortMap.

### Build

- :construction_worker: Improve dioxus bundler configuration  (no windows exe icon)
- :construction_worker: Improve compile settings for release builds.

### Opossum_backend

- Change to 2024 edition.

### Xtask

- Switch to 2024 edition

## [0.6.0] - 2024-12-18

### Bug Fixes

- :bug: dot images now with centered text and non overlapping boundaries
- :bug: Fixed inverse analyze of plano convex lenses + setting alignment wavelength property correctly
- :bug: Added isometry to wavefront monitor to center the graph on central ray
- :bug: Fixed Fluence calculation report bugs and border artifacts
- :bug: Single surface nodes now assume same refractive index as incoming ray.
- :bug: Removed interpolation artifact by implementing spade crate for interpolation
- :bug: Added "up-direction" to fix isometry inconsistencies when placing optics
- :bug: replaced error of stale ray-visualizer with a warning
- :bug: Reset all (detector) nodes after position calculation.
- :bug: wrong description of AR coating in example. worked as intended
- :bug: Add missing surfaces for various nodes.
- :bug: Fix dropped `view_direction' while reporting RayPositionHistories.
- :bug: example 'inverse_beam_splitter_test' works again. Closes #324
- Fix missing 'set_inverse' during deserialization of NodeGroup.
- :bug: Fix error during apodization of rays: Aperture did not consider isometries.
- :bug: Fix overwriting report files if multiple analyzer runs defined.
- :bug: Auto Axisqual function now only runs if discrepancy between axis is not too high to avoid "zoom-out effect"
- Inverse group dot diagram now corrected
- :bug: Position history of rays is now deleted for unintended refraction/reflection. Allows for ghost focus analysis with mirrors
- :bug: Refraction counter now only increases for non-passive surfaces
- :bug: Initial fluence now set to 0 or first value instead of negative infinity
- :bug: Ray origin plots now correct again
- :bug: Fix ghost focus analysis for BeamSplitter.
- :bug: fixed ray visualizer plot
- :bug: Fix output mapping for ghost focus in NodeGroups

### Documentation

- :memo: Update documentation of Ray::refract_on_surface
- Add fresnel coating example
- :memo: Update documentation due to the change from Surface to GeoSurface
- :memo: Add further documentation to coatings module.
- :memo: Add some documentation to the different analyzers.
- :memo: Add further module documentation to hit_map
- :memo: Add v0.6 release announcement (draft)
- :memo: Extend 0.6 relase announcement and add sample analysis report.

### Features

- :sparkles: Coatings can be assigned to surfaces (through OpticPorts) and are considered during calculation.
- :sparkles: Added gratings as a node
- :sparkles: Default aperture implementation for lenses
- :sparkles: Spot diagram now autosizes to minimum of one wavelength
- :sparkles: Add a set of analyzer to OpmDocument
- :sparkles: Spot diagram now accumulates ray information if hit more than once.
- :sparkles: Create a special ghost focus report automatically including hit maps of all surfaces.
- :sparkles: Add global ray propagation plot to ghost focus analysis report.
- :sparkles: Show analysis type (i.e. Energy, Ghost Focus,...) in report.
- :sparkles: added different colors to bounces in hitmap plots
- :sparkles: Ghost focus analysis now report on individual bounces of rays that may be critical in terms of lidt
- :sparkles: Make all nodes (except NodeGroup & NodeReference) alignable.
- :sparkles: Issue warning if read OPM file version differs from programm version.
- :sparkles: Display distance between nodes in dot diagram
- :sparkles: Extend automatic scaling of fluence KDE plot by 3 kernel sigmas
- :sparkles: FluenceDetector: fluence estimator strategy can now be selected through a property.
- Helper rays can now be used to propagate fluence elements for ghost focus analysis

### Miscellaneous Tasks

- Added Stretcher and compressor examples
- :see_no_evil: Update .gitignore to keep the playground folder (but not its contents)
- :green_heart: Update cargo dist with new config file format.
- :rotating_light: Fix linter warnings.
- :bookmark: Bump version number to 0.6.0. Update relase notes.

### Performance

- :zap: Improve performance of binning fluence estimator

### Refactor

- :recycle: Add CaotingType to OpticPorts and correct tests accordingly.
- :recycle: Add OpticalSurface struct for combination of GeoSurface & Coating.
- :recycle: Split up analyzer module ins separate submodules (raytrace, ghotsfocus)
- :recycle: Replace `properties` field in OpticScenery by simply description: String
- :recycle: Use derived serializer for OpticScenery instead of explicit implementation.
- :recycle: Add Analyzer::analyze() function accepting an OpticScenery instead of vice versa.
- :recycle: Use OpmDocument to simplify main and OpticScenery.
- :recycle: Replace all OpticScenery by NodeGroup.
- :recycle: Move raytracing analyisi function for NodeGroup from OpticGraph to AnalysisRayTrace.
- :recycle: Separate HtmlReport and AnalysisReport
- Cleanup AnalysisReport and NodeReport.
- :recycle: Move Analyzable trait to analyzers module.
- :recycle: Implement stubs for analyzer-specific report functions.
- :recycle: handle export of analysis data through Properties of a NodeReport.
- :recycle: Code linting
- :recycle: Delete no longer needed Detector node and adapt examples.
- :recycle: New struct OpticSurface replaces OpticPort and is stored in OpticPorts of Nodeattributes, simplifying accessing surface attributes
- :recycle: Removed triangulate crate to return to most recent rust version
- :recycle: uuid now only stored in node attributes
- :recycle: Consistent conention for radius of curvatur of curved optics
- :recycle: Move GeoSurface to its own module within surfaces.
- :coffin: Remove no longer necessary code from OpticSurface
- :bug: More intuitive positioning of parabolic mirrors + bug fix for oap telescopes
- :recycle: energy distribution now accepts inputs with Length in stead of plane f64
- :recycle: Remove `average` parameter from FluenceData
- :coffin: Remove no longer ncecessary fn in Analyzable.
- :recycle: Properties: remove unused PropCondition.
- :recycle: Implement derive macro OpmNode for reduction of boilerplate code.

### Styling

- :art: Slightly improve text logo formatting.
- :lipstick: Code cleanup

### Testing

- :white_check_mark: Add further unit test for Parabola.
- :white_check_mark: Add further unit tests for OpticSurface
- :white_check_mark: Add further tests for HitPoint and RaysHitMap
- :white_check_mark: Add further tests to HitMap
- :white_check_mark: Add unit tests for EnergyAnalyzer.
- :white_check_mark: Add unit tests for unit_format helper functions.
- :white_check_mark: Add further tests and docs for various utils modules
- :white_check_mark: Add unit tests for EnergyAnalyzer.
- :white_check_mark: Add further tests for Kde
- :white_check_mark: Add further test for Kde functions
- :white_check_mark: Add further tests for Source
- :white_check_mark: Add further test to Nodes
- :white_check_mark: Add tests for spectral_distribution::Gaussian
- :white_check_mark: Add further unit tests for Proptype.

## [0.5.0] - 2024-07-26

### Bug Fixes

- :bug: calc_ray_fluence_in_voronoi_cells used invalid rays in calculation
- :bug: Use ray data between input and output apodization for further detector analysis.
- :bug: Add uuid to exported data files in order to avoid to be overwritten.
- :bug: Update global config also for nested group nodes.
- Consider output light (i.e. wavelength) from each node port while calculating position of following node 
- :bug: Fix wrong calculation of node positioning when using a BeamSplitter
- :rotating_light: Fix linter warnings.
- :art: removed excessive margin from ray plots
- :bug: fixed plotting bug for auto-sizing ray-propagation plots
- :bug: Fix left over code using old "name" property.

### Documentation

- :memo: Improve formatting for geom_transformation module.
- :memo: Extend documentation for various OpticScenery functions.
- :memo: Add example tilter_wavefront_sensor to demonstrate tilted detectors.
- :memo: Improve prism pair example.
- Improve prism_pair example using absolute positioning of 2nd prism.
- :memo: Extend documentation

### Features

- :sparkles: Added SDF primitives for plane, sphere, cuboid and cylinder to render these primitives and their combinations
- :sparkles: Issue warning, if rays have been apodized at a detector node
- :sparkles: Add handling of a refractive index of an ambient medium between nodes.
- :sparkles: Sources can now also be positioned and aligned in 3D space.

### Miscellaneous Tasks

- Update CHANGELOG
- :building_construction: Improve code coverage accuracy by adding compiler options to config.toml
- Improve example grouptest which still shows some bugs during node positioning.

### Refactor

- :recycle: Use UOM for the focal length parameter of paraxial surfaces
- :recycle: Simplify LightResult structure.
- :recycle: Remove serde dependency from NodeGroup
- Using uom deeper within the fluence calculation
- :recycle: Move reduction from light source beams to optical axis directly to Source.
- :recycle: Remove no longer necessary function Optical::is_source()
- :fire: Removed ncollide2d dependency
- :recycle: Remove Ray::propagate_along_z which is no longer necessary.

### Styling

- :lipstick: Fix formatting issues. Update dependencies.
- :rotating_light: Fix linter warnings.

### Testing

- :white_check_mark: (Hopefully) fix failing test Ray::wavefront_error_at_pos_in_wvl for linux
- :white_check_mark: Add additional unit tests for Isometry.
- :white_check_mark: Add further testing to distribution functions.

### Build

- :building_construction: Disable debug symbols and link time optimization for profile 'test'.

## [0.4.0] - 2024-04-04

### Bug Fixes

- Check for stale (fully unconnected) node during analysis.
- Pipeline failure on linux while working on windows
- Used "NamedTempFile" in tests
- Fixed plot_params_fdir test as it was not running on linux
- :bug: correctly calculate surface normal for a sphere with negative radius of curvature.
- Imrprove RMS calculations for wavefront
- :bug: When the plot creation for wavefronts fails, a warning is thrown instead of an error
- :bug: Single data points are now displayed with usful axis bounds in plots
- :bug: Enabled export_data function for detector nodes in a group
- :bug: SpotDiagram now produces a warning instead of an error when no light data is present
- :bug: Show error message while parsing a model with a NodeReference
- :bug: ports of dotted nodes are now symmetric again

### Documentation

- Add documentation for Propagation node.
- Fix typo in BeamSplitter docs
- :memo: Add missing documentation in ray module.
- :memo: Improve docs for position_distributions
- :memo: Improve documentation of various modules.

### Feature

- : Calc RMS radius of rays.

### Features

- Add Rays::add_rays fn.
- Add Rays::threshold_by_energy fn
- Implement dropping rays below a given energy during raytracing.
- Add spectrum helper create_short_pass_filter
- Add spectrum helper create_long_pass_filter
- :sparkles: add Ray::split_by_spectrum function
- Added ry position history to struct
- :sparkles: Add general logging capability instead of simple print statements.
- Added analysis type to the pdf report
- Added a raypropagation visualizer detector node
- Implement ray refraction on a surface.
- Added calculation of the transverse fluence of a beam
- :sparkles: Add new distribution stragey: regular grid.
- Added Fluence detector node
- :sparkles: Implement spherical lens.
- :sparkles: Issue  warning, if scenery with unconnected sub-trees found during analysis.
- Added Fluencedetector node
- :sparkles: Support for no longer valid rays in a bundle.
- :sparkles: Lens can now also have flat surfaces
- :sparkles: Add first support for refractive index dispersion functions.
- Added energydistribution functions and trait
- :art: Wavefront plots are now displayed as interpolaeted colormesh
- Added multicolor scatter plots
- Plots now scale according to plotparameter AxisEqual
- Added uom_macros for simpler unit unit generation
- Single wavelength spot diagrams or ray plots are now shown in red
- Added energy_weighted centroid calculation to rays methods
- Spot diagram now uses energy weighted rms radius and centroid
- :sparkles: Added cylndrical surface and signed-distance function trait

### Miscellaneous Tasks

- Fix some compiler warnings.
- Update build step
- Fix version of cargo-dist
- Move README.md and LICENSE to top-level dir.
- Hopefully fixed CI pipeline again.
- Bump version umber to 0.4.0

### Performance

- :zap: Improve memory allocation of point distribution generators

### Refactor

- Allow for engineering format of arbitrary quantities
- Move helper functions for generatin spectra to separate file.
- :recycle: Separate Ray & Rays into separate modules.
- Fix linter warnings.
- Changed the input argument of a new ray position from Point2 to Point3
- To_plot is now a pure default function of the Plottable trait
- Streamline unit Ray unit tests.
- Calculate internally in base units for Ray.
- :recycle: Extract DistributionStrategy from Rays and move to its own module.
- Use structs for DistributionStrategy enum parameters
- :recycle: Move distribution functions to its onw submodules.
- Changed rays in apodizing function instead of creating a newe set of rays
- :recycle: Move creation functions of light sources to own module: source_helper
- :recycle: All new() functions of uom have been replaced by the new uom macros, execpt for zero()
- :recycle: Split up the Properties module in several submodules

### Styling

- :rotating_light: Fix linter warnings in Rays
- :art: tiny reformatting

### Testing

- Add further tests
- Add unit test for ray splitting by spectrum.
- Added few units tests to plottable module
- Added few units tests to plottable module
- Added shit-ton of tests. still more to come
- FUrther testing. Still more to go
- Finished testing
- Add missing tests for IdealFilter
- :white_check_mark: Add further tests to Proptype
- :white_check_mark: cargo fmt and dot testing updated
- :white_check_mark: maybe fixed plottable test

### Build

- Add config to strip symbols if building with releas profile. This reduces binary file size.

### Refract_on_surface

- Return direction vector of reflected ray

## [0.3.0] - 2023-12-20

### Analyzer

- Add config for AnalyzerType::RayTrace
- Remove the analyzer struct.
- Add unit tests

### Aperture

- Derive Serialize
- Derive Deserialize

### BeamSplitter

- Impl analyze_raytrace
- Add unit test for raytrace
- Improve error message if wrong LightData datatype used.

### CI

- Simplify script
- Fix pipeline

### Cargo

- Fix version of ncollide2d

### Cargo.toml

- Add link to README.md

### Documentation

- Add 0.3.0 announcement.

### Dummy

- Add unit test for analyze_empty

### EnergyMeter

- Support LightData::Geometric

### Group

- Impl is_detector
- Impl report fn.

### IdealFilter

- Analysis of geo rays w/ fixed factor
- Add analysis unit test for geo rays.
- Return error if wrong analyzer type.

### Lib

- Add unit test

### LightData

- Add further unit tests

### Main

- Flush some output and write error to stderr

### Miscellaneous Tasks

- Add unit test for refract_paraxial.
- Use Kahan sum for total_energy().

### Nodes

- Modify ports handling.
- Add further unit tests
- Add further unit tests

### OpticGraph

- Add further unit tests

### OpticPorts

- Remove unnecessary fns.

### OpticRef

- Add further unit tests

### OpticScenery

- Impl PdfPlottable.
- Also apodize outgoing light.
- Add unit test for save_to_file
- Add further unit tests.

### Optical

- Further unit tests (through Dummy)

### ParaxialSurface

- Add basic unit tests.

### Plottable

- Implement different backends

### Propagation

- Add (yet empty) propagation node.

### Properties

- Add further unit tests.
- Maintain order of properties.
- Add unit test for format fn

### Property

- Avoid setting incompatible vlaue types.
- Add unit test.

### Proptype

- Add length property.
- Add new type: Energy.

### Ray

- Add unit test for propagation
- Add unit test for refract_paraxial
- Add fn filter_by factor
- Use FilterType for filter fn instead constant.
- Add unit test for filter with spectrum
- Add split fn.

### Rays

- Add ray distribution fns.
- Impl Plotaable trait.
- Add sobol distribution.
- Impl apodization of rays by given aperture.
- Fix chart error for empty Rays struct
- Impl propagation along the optical axis.
- Add unit tests
- Add test for propagation.
- Add further tests
- Add further unit tests.
- Impl generation of ray cone
- Extend unit test
- Slightly improve plot layout
- Use measurement units on interface.
- Add uom also to distributions fns
- Improve spot diagram plotting
- Implement paraxial refraction
- Add centroid and geometric radius fns.
- Add unit tests
- Impl wavelength_range() fn.
- Add unit test for wavelength_range()
- Impl to_spectrum instead  Spectrum::from_ray
- Add split fn and unit test
- Impl merge fn.
- Don't normalize after refraction.

### Report

- Add some basic pdf generation.
- Impl PdfReportable for Spectrum
- Improve layout of header.
- Align properties as table
- Implement new report fn for various nodes.
- Improve error handling. Scale diagram

### ReportGenerator

- Add unit tests.
- Add further unit test.
- Warning if graphviz not installed

### Reporter

- Embed fonts in binary.

### Reprt

- Update OpticScenery report function.

### SOurce

- Unit test for create_collimated_ray_source

### Source

- Apodize rays at output port aperture.
- Unit test for create_collimated_ray_source
- Extend unit tests
- Add test for create_point_ray_source
- Add unit test for set_light_data
- Add test for debug.

### Spectrometer

- Add unit test for debug.
- Add further unit tests.

### Spectrum

- Add further  unit test for scaling.
- Fix plot scale
- Further unit tests.
- Impl get_value() and unit test.
- Slight code cleanup
- Slightly improve plot layout.
- Add unit test for debug.
- Add Kahan sum for total_energy.

### SpotDiagram

- Add further unit tests

## [0.2.0] - 2023-10-18

### EnergyMeter

- Extend unit test.

### Group

- Add unit tests for analysis.
- Add analyze_inverted unit test.
- Cleanup test code.
- Treat non-existing input data as None.
- Simplify serialization.
- :add_node: return error is group is inverted.
- Connect_nodes: return error if inverted.

### Miscellaneous Tasks

- Synchronize graph from props after d13n.

### NodeReferecne

- Add serialization of reference uuid

### NodeReference

- D13n seems to work now.
- Add unit test for assign_reference
- Add several analysis unit tests.

### OpticGraph

- Implement d13n of edges with uuid.

### OpticScenery

- Analyze add consistency check.

### Properties

- Add create fn & better error handling.
- Make attribute private
- Prepare for integration of description
- Add description to each created prop.

### Property

- Make prob attribute private

## [0.1.0] - 2023-10-09

### BeamSplitter

- Add range check for split ratio

### Beamsplitter

- Set range as inclusive.
- Document errors.

### CSEpctrum

- Use vec of tuples instead  two vectors.

### Group

- Invert graph only during analysis and to_dot.
- Reenable all to_dot functionality

### IdealFIlter

- Add filter_type to properties.

### Miscellaneous Tasks

- Use only major version dependencies..

### Node

- Add is_detector fn.

### NodeReference

- Change node reference to waek reference.

### OpticGraph

- Implement d13n of edges.

### OpticSceneray

- Reenable analysis function.

### OpticScenery

- Add example.

### Scenery

- REmove add_element fn.

### Spectrum

- Replace energy with generic f64.
- Code optimization in plot fn.

### Connect_nodes

- Check if src_node & port already connected.

<!-- generated by git-cliff -->
