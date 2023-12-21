# Changelog

All notable changes to this project will be documented in this file.

## [unreleased]

### Feature

- : Calc RMS radius of rays.

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
