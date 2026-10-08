//! Upgrading `.opm` files written by older versions of OPOSSUM.
//!
//! Every file states the version of its format (`opm_file_version`). A file of an older version
//! is brought up to the current one when it is read and stamped with the current version, so it
//! is written back in the current format. A file of a newer version, or one whose version cannot
//! be read, is read as it is, with a warning.
//!
//! The nodes of an older file are upgraded one by one while they are read, before each of them
//! builds its surfaces: an older file may describe a node the current checks would refuse.

use super::OpmDocument;
use crate::{
    apertures::{Aperture, ApertureShape, ApertureType, CircleShape},
    core_optics::{NodeAttrExt, OpticNode, OpticNodeExt, PortType},
    error::{OpmResult, OpossumError},
    geometry::{
        RimRole,
        body::{CLEAR_APERTURE, default_clear_aperture},
    },
    nanometer,
    nodes::NodeGroup,
    properties::Proptype,
    utils::geom_transformation::Isometry,
};
use log::{info, warn};
use serde::Deserialize;
use std::cell::Cell;
use uom::si::{angle::radian, f64::Length, length::millimeter};

thread_local! {
    /// The format version of the file being read on this thread, while it is older than the
    /// current one (see [`Upgrading`]).
    static UPGRADING_FROM: Cell<Option<u32>> = const { Cell::new(None) };
}

/// While it lives, every node read on this thread is upgraded from the given file format version
/// (see [`upgrade_node`]).
struct Upgrading {
    previous: Option<u32>,
}

impl Upgrading {
    /// Start upgrading the nodes read on this thread from the given file format version.
    fn from(version: u32) -> Self {
        Self {
            previous: UPGRADING_FROM.replace(Some(version)),
        }
    }
}

impl Drop for Upgrading {
    fn drop(&mut self) {
        UPGRADING_FROM.set(self.previous);
    }
}

/// The part of a file that states its format version.
#[derive(Deserialize)]
struct FileHeader {
    opm_file_version: String,
}

/// Return the version of the file format this program writes.
///
/// # Errors
///
/// This function returns an error if the version this program was built with is not a number.
fn current_version() -> OpmResult<u32> {
    env!("OPM_FILE_VERSION").parse().map_err(|e| {
        OpossumError::OpmDocument(format!(
            "the file format version '{}' of this program is not a number: {e}",
            env!("OPM_FILE_VERSION")
        ))
    })
}

/// Read a document from the contents of an `.opm` file and bring it up to the file format this
/// program writes.
///
/// The version is read first, so that the nodes of an older file are upgraded while they are
/// read (see [`upgrade_node`]).
///
/// # Arguments
///
/// * `file_string` - the contents of the file.
///
/// # Errors
///
/// This function returns an error if the file cannot be parsed, a node cannot be upgraded or the
/// version this program writes is not a number.
pub(super) fn read(file_string: &str) -> OpmResult<OpmDocument> {
    let current = current_version()?;
    let older = ron::from_str::<FileHeader>(file_string)
        .ok()
        .and_then(|header| header.opm_file_version.parse::<u32>().ok())
        .filter(|version| *version < current);
    let mut document: OpmDocument = {
        let _upgrading = older.map(Upgrading::from);
        ron::from_str(file_string)
            .map_err(|e| OpossumError::OpmDocument(format!("parsing of model failed: {e}")))?
    };
    upgrade(&mut document, current);
    Ok(document)
}

/// Upgrade a node just read from a file of an older format version.
///
/// It is called while the node is read, before the node builds its surfaces. Outside the reading
/// of an older file it does nothing.
///
/// # Arguments
///
/// * `node` - the node as read from the file.
///
/// # Errors
///
/// This function returns an error if a property of the node cannot be set or its geometry cannot
/// be derived.
pub fn upgrade_node(node: &mut dyn OpticNode) -> OpmResult<()> {
    if UPGRADING_FROM.get().is_some_and(|version| version < 1) {
        upgrade_node_from_0(node)?;
    }
    Ok(())
}

/// Upgrade a node of format version 0.
///
/// That format stated the size of a component by a binary hole at its entrance port, and it did
/// not check that a curved face reaches the clear aperture. Such a hole becomes the clear
/// aperture and the ports holding it open; a clear aperture beyond the curvature shrinks to it.
///
/// # Errors
///
/// See [`upgrade_node`].
fn upgrade_node_from_0(node: &mut dyn OpticNode) -> OpmResult<()> {
    if node.node_attr().get_property(CLEAR_APERTURE).is_err() {
        return Ok(());
    }
    if let Some(hole) = entrance_hole(node)
        && port_frame_is_rim_frame(node)?
    {
        node.node_attr_mut()
            .set_property(CLEAR_APERTURE, hole.shape().clone().into())?;
        open_ports_with(node, &hole);
    }
    fit_clear_aperture_to_curvature(node)
}

/// Return the aperture at the physical entrance `input_1` of a node if it outlines a region the
/// way a clear aperture does: a centred binary hole.
fn entrance_hole(node: &dyn OpticNode) -> Option<Aperture> {
    let aperture = &node
        .node_attr()
        .raw_ports()
        .ports_raw(&PortType::Input)
        .get("input_1")?
        .aperture;
    (*aperture.aperture_type() == ApertureType::Hole
        && aperture.shape().is_binary()
        && aperture.isometry().is_none())
    .then(|| aperture.clone())
}

/// Return whether the port apertures of a node were measured in the frame its clear aperture is
/// measured in, but for a shift along the axis. Only then does a port hole outline the same
/// region as a clear aperture of its shape; it does not for an off-axis parabola.
///
/// The frames do not depend on the size of the clear aperture, so they are taken with a tiny one
/// that any curvature reaches, and the declared one is put back.
///
/// # Errors
///
/// This function returns an error if the clear aperture cannot be set or the geometry cannot be
/// derived.
fn port_frame_is_rim_frame(node: &mut dyn OpticNode) -> OpmResult<bool> {
    let declared = node.node_attr().get_property(CLEAR_APERTURE)?.clone();
    let probe = ApertureShape::from(CircleShape::new(nanometer!(1.0))?);
    node.node_attr_mut()
        .set_property(CLEAR_APERTURE, probe.into())?;
    let frames = port_and_rim_frames(node);
    node.node_attr_mut()
        .set_property(CLEAR_APERTURE, declared)?;
    let Some((port_frame, rim_frame)) = frames? else {
        return Ok(false);
    };
    let relative =
        Isometry::new_from_transform(rim_frame.get_inv_transform() * port_frame.get_transform());
    let (shift, rotation) = (relative.translation(), relative.rotation());
    Ok(shift.x.abs() < nanometer!(1.0)
        && shift.y.abs() < nanometer!(1.0)
        && [rotation.x, rotation.y, rotation.z]
            .iter()
            .all(|angle| angle.get::<radian>().abs() < 1e-9))
}

/// Return the frame the entrance port's aperture is measured in and the frame the rim is
/// measured in, both relative to the node, or `None` if the node has no rim.
///
/// # Errors
///
/// This function returns an error if the geometry cannot be derived.
fn port_and_rim_frames(node: &dyn OpticNode) -> OpmResult<Option<(Isometry, Isometry)>> {
    let Some(geometry) = node.geometry()? else {
        return Ok(None);
    };
    let ((_, port_frame), _) = geometry.entrance_and_exit(&Isometry::identity())?;
    Ok(geometry
        .rim()?
        .map(|rim| (port_frame, rim.frame(&Isometry::identity()))))
}

/// Open every physical port of a node whose aperture is the given hole.
fn open_ports_with(node: &mut dyn OpticNode, hole: &Aperture) {
    let ports = node.node_attr_mut().raw_ports_mut();
    for port_type in [PortType::Input, PortType::Output] {
        for config in ports.ports_mut(&port_type).values_mut() {
            if config.aperture == *hole {
                config.aperture = Aperture::default();
            }
        }
    }
}

/// Shrink a clear aperture that reaches beyond the tightest radius of curvature of a node to a
/// circle of that radius, with a warning naming the node.
///
/// The radii are those of the node's curvature properties.
///
/// # Errors
///
/// This function returns an error if the clear aperture cannot be read or set.
fn fit_clear_aperture_to_curvature(node: &mut dyn OpticNode) -> OpmResult<()> {
    let tightest = node
        .node_attr()
        .properties()
        .into_iter()
        .filter_map(|(_, property)| match property.prop() {
            Proptype::Curvature(radius) if radius.is_finite() => Some(radius.abs()),
            _ => None,
        })
        .reduce(Length::min);
    let (Some(tightest), Some(cross_section)) = (tightest, node.clear_aperture()?) else {
        return Ok(());
    };
    if cross_section.transversal_reach() <= tightest {
        return Ok(());
    }
    node.node_attr_mut().set_property(
        CLEAR_APERTURE,
        ApertureShape::from(CircleShape::new(tightest)?).into(),
    )?;
    warn!(
        "the clear aperture of {} reached beyond its tightest radius of curvature and was reduced \
         to a circle of {:.3} mm",
        node.node_info(),
        tightest.get::<millimeter>()
    );
    Ok(())
}

/// Bring a document read from a file up to the file format this program writes, and warn about
/// what the upgrade changes for the analyses.
///
/// # Arguments
///
/// * `document` - the document as read from the file.
/// * `current` - the version of the file format this program writes.
fn upgrade(document: &mut OpmDocument, current: u32) {
    match document.opm_file_version.parse::<u32>() {
        Ok(read) if read == current => {}
        Ok(read) if read < current => {
            if read < 1 {
                warn_about_the_upgrade_from_0(&mut document.scenery);
            }
            info!("upgrading the model from file format version {read} to {current}");
            document.opm_file_version = current.to_string();
        }
        _ => {
            warn!("OPM file version does not match the used OPOSSUM version.");
            warn!(
                "read version '{}' <-> program file version '{current}'",
                document.opm_file_version,
            );
            warn!(
                "This file might have been written by an older or newer version of OPOSSUM. The model import might not be correct."
            );
        }
    }
}

/// Warn about what reading a file of format version 0 changes for the analyses.
///
/// Rays beyond the clear aperture of a component now miss it, so the components left with the
/// default clear aperture are counted. A detector whose port hole became the window it records
/// within now lets rays outside it pass unrecorded, so it is named.
///
/// # Arguments
///
/// * `scenery` - the upgraded model.
fn warn_about_the_upgrade_from_0(scenery: &mut NodeGroup) {
    let mut defaults = 0;
    let mut windows = Vec::new();
    scenery.for_each_node_mut(&mut |node| {
        let Ok(Some(rim)) = node
            .geometry()
            .and_then(|geometry| geometry.map_or(Ok(None), |geometry| geometry.rim()))
        else {
            return;
        };
        match rim.role() {
            RimRole::Window => windows.push(node.node_info()),
            RimRole::Edge => {
                if matches!(
                    node.node_attr().get_property(CLEAR_APERTURE),
                    Ok(Proptype::Aperture(shape)) if *shape == default_clear_aperture()
                ) {
                    defaults += 1;
                }
            }
        }
    });
    if defaults > 0 {
        warn!(
            "{defaults} component(s) keep the default clear aperture of 12.5 mm radius: rays \
             beyond it now miss them. Set the clear aperture of a component that is larger."
        );
    }
    for detector in windows {
        warn!(
            "the port hole of detector {detector} became the window it records within: rays \
             outside it now pass it unrecorded"
        );
    }
}

#[cfg(test)]
mod test {
    use crate::{
        analyzers::Analyzable,
        apertures::{Aperture, ApertureShape, ApertureType, CircleShape, StackShape},
        core_optics::{NodeAttrExt, OpticNode, OpticNodeExt, OpticRef, PortType},
        degree,
        error::OpmResult,
        geometry::body::{CLEAR_APERTURE, default_clear_aperture},
        millimeter,
        nodes::{BeamSplitter, Dummy, Lens, NodeGroup, ParabolicMirror, SpotDiagram, ThinMirror},
        opm_document::OpmDocument,
        properties::Proptype,
        refractive_index::RefrIndexConst,
    };
    use uuid::Uuid;

    /// The file a program writing format version `version` would have written for the document.
    fn file_of(document: &OpmDocument, version: &str) -> OpmResult<String> {
        let current = format!("opm_file_version: \"{}\"", env!("OPM_FILE_VERSION"));
        let file = document.to_opm_file_string()?;
        assert!(file.contains(&current), "the file states its version");
        Ok(file.replace(&current, &format!("opm_file_version: \"{version}\"")))
    }
    /// An empty document, written as a program writing format version `version` would have.
    fn file_of_version(version: &str) -> OpmResult<String> {
        file_of(&OpmDocument::default(), version)
    }
    /// A document holding the given node, and the id of that node.
    fn document_with<T: Analyzable + Clone + 'static>(node: T) -> OpmResult<(OpmDocument, Uuid)> {
        let mut scenery = NodeGroup::default();
        let id = scenery.add_node(node)?;
        Ok((OpmDocument::new(scenery), id))
    }
    /// Write the document in the given format version, read it back and return the node.
    fn read_back(document: &OpmDocument, version: &str, id: Uuid) -> OpmResult<OpticRef> {
        let read = OpmDocument::from_string(&file_of(document, version)?)?;
        Ok(read.scenery().node_recursive(id)?.0)
    }
    /// Read a single node back from a file of format version 0.
    fn upgraded<T: Analyzable + Clone + 'static>(node: T) -> OpmResult<OpticRef> {
        let (document, id) = document_with(node)?;
        read_back(&document, "0", id)
    }
    /// A circular hole of the given radius in mm.
    fn hole(radius: f64) -> OpmResult<Aperture> {
        Aperture::new_circle(millimeter!(radius), ApertureType::Hole, None)
    }
    /// The clear aperture of a node.
    fn clear_aperture(node: &OpticRef) -> ApertureShape {
        let Ok(Proptype::Aperture(shape)) = node.node_attr().get_property(CLEAR_APERTURE) else {
            panic!("the node has no clear aperture");
        };
        shape.clone()
    }
    /// The aperture of a physical port of a node.
    fn port_aperture(node: &OpticRef, port_type: &PortType, name: &str) -> Aperture {
        node.node_attr()
            .raw_ports()
            .ports_raw(port_type)
            .get(name)
            .map(|config| config.aperture.clone())
            .expect("the port exists")
    }
    /// A circle of the given radius in mm.
    fn circle(radius: f64) -> OpmResult<ApertureShape> {
        Ok(CircleShape::new(millimeter!(radius))?.into())
    }
    /// Read a file and return the version of the document and whether a warning was logged.
    fn read(file: &str) -> OpmResult<(String, bool)> {
        testing_logger::setup();
        let document = OpmDocument::from_string(file)?;
        let warned = std::cell::Cell::new(false);
        testing_logger::validate(|logs| {
            warned.set(logs.iter().any(|log| log.level == log::Level::Warn));
        });
        Ok((document.opm_file_version, warned.get()))
    }
    #[test]
    fn a_file_of_an_older_version_is_upgraded_to_the_current_one() -> OpmResult<()> {
        let (version, warned) = read(&file_of_version("0")?)?;
        assert_eq!(version, env!("OPM_FILE_VERSION"));
        assert!(!warned, "an upgrade is expected, not a reason to warn");
        Ok(())
    }
    #[test]
    fn a_file_of_the_current_version_is_read_as_it_is() -> OpmResult<()> {
        let (version, warned) = read(&file_of_version(env!("OPM_FILE_VERSION"))?)?;
        assert_eq!(version, env!("OPM_FILE_VERSION"));
        assert!(!warned);
        Ok(())
    }
    /// A newer file may hold what this program does not know; it is read with a warning and keeps
    /// its version. So does a file whose version is not a number.
    #[test]
    fn a_file_of_a_newer_or_unreadable_version_is_read_with_a_warning() -> OpmResult<()> {
        for version in ["1000", "abc"] {
            let (read_version, warned) = read(&file_of_version(version)?)?;
            assert_eq!(read_version, version);
            assert!(warned, "version {version}");
        }
        Ok(())
    }
    /// In format version 0 a binary hole at the entrance of a component stated how large the
    /// component is. It becomes the clear aperture, and every port with the same hole opens; a
    /// port with another aperture keeps it. A detector records within the hole from now on.
    #[test]
    fn a_port_hole_becomes_the_clear_aperture() -> OpmResult<()> {
        let mut lens = Lens::default();
        lens.set_aperture(&PortType::Input, "input_1", &hole(5.0)?)?;
        lens.set_aperture(&PortType::Output, "output_1", &hole(5.0)?)?;
        let lens = upgraded(lens)?;
        assert_eq!(clear_aperture(&lens), circle(5.0)?);
        assert_eq!(
            port_aperture(&lens, &PortType::Input, "input_1"),
            Aperture::default()
        );
        assert_eq!(
            port_aperture(&lens, &PortType::Output, "output_1"),
            Aperture::default()
        );

        let mut splitter = BeamSplitter::default();
        splitter.set_aperture(&PortType::Input, "input_1", &hole(5.0)?)?;
        splitter.set_aperture(&PortType::Input, "input_2", &hole(4.0)?)?;
        let splitter = upgraded(splitter)?;
        assert_eq!(clear_aperture(&splitter), circle(5.0)?);
        assert_eq!(
            port_aperture(&splitter, &PortType::Input, "input_1"),
            Aperture::default()
        );
        assert_eq!(
            port_aperture(&splitter, &PortType::Input, "input_2"),
            hole(4.0)?
        );

        for mut node in [
            OpticRef::new(Box::new(ThinMirror::default())),
            OpticRef::new(Box::new(SpotDiagram::default())),
        ] {
            node.set_aperture(&PortType::Input, "input_1", &hole(5.0)?)?;
            let mut scenery = NodeGroup::default();
            let id = scenery.add_node_ref(node)?;
            let node = read_back(&OpmDocument::new(scenery), "0", id)?;
            assert_eq!(clear_aperture(&node), circle(5.0)?, "{}", node.node_type());
            assert_eq!(
                port_aperture(&node, &PortType::Input, "input_1"),
                Aperture::default()
            );
        }
        Ok(())
    }
    /// Only a plain binary hole centred on the port outlines a component. A soft, an obstructing,
    /// a stacked or a decentred aperture stays a mask on the port.
    #[test]
    fn a_port_aperture_that_is_no_plain_outline_stays_a_mask() -> OpmResult<()> {
        let masks = [
            Aperture::new_gaussian(
                (millimeter!(5.0), millimeter!(5.0)),
                ApertureType::Hole,
                None,
                None,
            )?,
            Aperture::new_circle(millimeter!(5.0), ApertureType::Obstruction, None)?,
            Aperture::new(
                ApertureShape::Stack(StackShape::new(vec![
                    hole(8.0)?,
                    Aperture::new_circle(millimeter!(2.0), ApertureType::Obstruction, None)?,
                ])?),
                ApertureType::Hole,
                None,
                None,
            )?,
            Aperture::new_circle(
                millimeter!(5.0),
                ApertureType::Hole,
                Some(millimeter!(1.0, 0.0)),
            )?,
        ];
        for mask in masks {
            let mut lens = Lens::default();
            lens.set_aperture(&PortType::Input, "input_1", &mask)?;
            let lens = upgraded(lens)?;
            assert_eq!(clear_aperture(&lens), default_clear_aperture(), "{mask:?}");
            assert_eq!(port_aperture(&lens, &PortType::Input, "input_1"), mask);
        }
        Ok(())
    }
    /// A node without a clear aperture keeps its port hole: there is no outline to move it to.
    #[test]
    fn a_node_without_a_clear_aperture_keeps_its_port_hole() -> OpmResult<()> {
        let mut dummy = Dummy::default();
        dummy.set_aperture(&PortType::Input, "input_1", &hole(5.0)?)?;
        let dummy = upgraded(dummy)?;
        assert_eq!(
            port_aperture(&dummy, &PortType::Input, "input_1"),
            hole(5.0)?
        );
        Ok(())
    }
    /// The port hole of an off-axis parabola was measured around the off-axis point, its clear
    /// aperture is measured along the parent axis: the two are not the same outline, so the hole
    /// stays a mask.
    #[test]
    fn the_port_hole_of_an_off_axis_parabola_stays_a_mask() -> OpmResult<()> {
        let mut parabola =
            ParabolicMirror::new_with_off_axis_x("oap", millimeter!(100.0), true, degree!(90.0))?;
        parabola.set_aperture(&PortType::Input, "input_1", &hole(5.0)?)?;
        let parabola = upgraded(parabola)?;
        assert_eq!(clear_aperture(&parabola), default_clear_aperture());
        assert_eq!(
            port_aperture(&parabola, &PortType::Input, "input_1"),
            hole(5.0)?
        );
        Ok(())
    }
    /// Nodes in nested groups are upgraded too.
    #[test]
    fn a_node_in_a_nested_group_is_upgraded() -> OpmResult<()> {
        let mut lens = Lens::default();
        lens.set_aperture(&PortType::Input, "input_1", &hole(5.0)?)?;
        let mut inner = NodeGroup::new("inner");
        let id = inner.add_node(lens)?;
        let mut outer = NodeGroup::new("outer");
        outer.add_node(inner)?;
        let (document, _) = document_with(outer)?;
        let lens = read_back(&document, "0", id)?;
        assert_eq!(clear_aperture(&lens), circle(5.0)?);
        Ok(())
    }
    /// What counts is the hole at the physical entrance `input_1`, also for an inverted node.
    #[test]
    fn an_inverted_node_moves_the_hole_of_its_physical_entrance() -> OpmResult<()> {
        let mut lens = Lens::default();
        lens.set_aperture(&PortType::Input, "input_1", &hole(5.0)?)?;
        lens.set_inverted(true)?;
        let lens = upgraded(lens)?;
        assert_eq!(clear_aperture(&lens), circle(5.0)?);
        assert_eq!(
            port_aperture(&lens, &PortType::Input, "input_1"),
            Aperture::default()
        );
        Ok(())
    }
    /// A file of the current version or a newer one is not upgraded: its port holes stay masks,
    /// also when an older file was read just before.
    #[test]
    fn only_a_file_of_an_older_version_is_upgraded() -> OpmResult<()> {
        let mut lens = Lens::default();
        lens.set_aperture(&PortType::Input, "input_1", &hole(5.0)?)?;
        let (document, id) = document_with(lens)?;
        assert_eq!(
            clear_aperture(&read_back(&document, "0", id)?),
            circle(5.0)?
        );
        for version in [env!("OPM_FILE_VERSION"), "1000"] {
            let lens = read_back(&document, version, id)?;
            assert_eq!(clear_aperture(&lens), default_clear_aperture(), "{version}");
            assert_eq!(
                port_aperture(&lens, &PortType::Input, "input_1"),
                hole(5.0)?
            );
        }
        Ok(())
    }
    /// Format version 0 did not check that a curved face reaches the clear aperture. A lens of
    /// R = 10 mm with the default clear aperture of 12.5 mm still loads: its clear aperture shrinks
    /// to its tightest radius of curvature, with a warning naming the lens.
    #[test]
    fn a_clear_aperture_beyond_the_curvature_shrinks_to_it() -> OpmResult<()> {
        let lens = Lens::new_with_clear_aperture(
            "strong lens",
            millimeter!(10.0),
            millimeter!(f64::INFINITY),
            millimeter!(5.0),
            RefrIndexConst::new(1.5)?,
            circle(9.0)?,
        )?;
        let (document, id) = document_with(lens)?;
        let file = file_of(&document, "0")?;
        assert!(
            file.contains("radius: 0.009"),
            "the clear aperture is written as expected"
        );
        let file = file.replace("radius: 0.009", "radius: 0.0125");
        testing_logger::setup();
        let read = OpmDocument::from_string(&file)?;
        let lens = read.scenery().node_recursive(id)?.0;
        assert_eq!(clear_aperture(&lens), circle(10.0)?);
        testing_logger::validate(|logs| {
            assert!(
                logs.iter()
                    .any(|log| log.level == log::Level::Warn && log.body.contains("'strong lens'")),
                "a warning names the lens"
            );
        });
        Ok(())
    }
    /// After an upgrade a warning counts the components left with the default clear aperture,
    /// since rays beyond it now miss them, and another one names the detectors whose port hole
    /// became their window, since rays outside it now pass them unrecorded.
    #[test]
    fn an_upgrade_warns_about_default_clear_apertures_and_new_windows() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        scenery.add_node(Lens::default())?;
        let mut mirror = ThinMirror::default();
        mirror.set_aperture(&PortType::Input, "input_1", &hole(5.0)?)?;
        scenery.add_node(mirror)?;
        let mut camera = SpotDiagram::new("camera")?;
        camera.set_aperture(&PortType::Input, "input_1", &hole(5.0)?)?;
        scenery.add_node(camera)?;
        let file = file_of(&OpmDocument::new(scenery), "0")?;
        testing_logger::setup();
        OpmDocument::from_string(&file)?;
        testing_logger::validate(|logs| {
            let warnings: Vec<&str> = logs
                .iter()
                .filter(|log| log.level == log::Level::Warn)
                .map(|log| log.body.as_str())
                .collect();
            assert!(
                warnings
                    .iter()
                    .any(|w| w.starts_with("1 component") && w.contains("default clear aperture")),
                "{warnings:?}"
            );
            assert!(
                warnings.iter().any(|w| w.contains("'camera'")),
                "{warnings:?}"
            );
        });
        Ok(())
    }
}
