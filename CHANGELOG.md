# Changelog

All notable changes to this project will be documented in this file.

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
