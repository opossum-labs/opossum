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
    apertures::{Aperture, ApertureType, Bore, CircleShape, ClearAperture, Outline},
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
/// * `port_apertures` - the physical type, the name and the aperture of every port of the node
///   that held one in the file.
///
/// # Errors
///
/// This function returns an error if a property of the node cannot be set or its geometry cannot
/// be derived.
pub fn upgrade_node(
    node: &mut dyn OpticNode,
    port_apertures: Vec<(PortType, String, Aperture)>,
) -> OpmResult<()> {
    if UPGRADING_FROM.get().is_some_and(|version| version < 1) {
        upgrade_node_from_0(node, port_apertures)?;
    }
    Ok(())
}

/// Set the clear aperture a file stated as an
/// [`ApertureShape`](crate::apertures::ApertureShape), the way files did before the clear aperture
/// had a type of its own, on the node read from it.
///
/// Such a value does not match the declared property, so it was not merged into the properties of
/// the node; the type of the value tells, not the format version of the file. A shape no outline
/// with bores can state (see [`ClearAperture::from_aperture_shape`]), or one the node refuses,
/// leaves the default, with a warning naming the node.
///
/// # Arguments
///
/// * `node` - the node as read from the file.
/// * `read` - the value the file stated for the clear aperture, if any.
pub fn upgrade_clear_aperture(node: &mut dyn OpticNode, read: Option<&Proptype>) {
    let Some(Proptype::Aperture(shape)) = read else {
        return;
    };
    let upgraded = ClearAperture::from_aperture_shape(shape).and_then(|clear_aperture| {
        node.node_attr_mut()
            .set_property(CLEAR_APERTURE, clear_aperture.into())
    });
    if let Err(e) = upgraded {
        warn!(
            "the clear aperture '{shape}' of {} was dropped, it keeps its default: {e}",
            node.node_info()
        );
    }
}

/// Upgrade a node of format version 0.
///
/// That format stated the size of a component by a binary hole at its entrance port, let every
/// port mask the light passing it by an aperture, and did not check that a curved face reaches the
/// clear aperture. Such a hole becomes the clear aperture and takes the place of every port
/// aperture equal to it; a clear aperture beyond the curvature shrinks to it. Only the clear
/// aperture limits light now, so every other port aperture joins it or is dropped (see
/// [`join_port_apertures`]).
///
/// # Errors
///
/// See [`upgrade_node`].
fn upgrade_node_from_0(
    node: &mut dyn OpticNode,
    mut port_apertures: Vec<(PortType, String, Aperture)>,
) -> OpmResult<()> {
    if node.node_attr().get_property(CLEAR_APERTURE).is_ok() {
        if let Some(hole) = entrance_hole(&port_apertures)
            && port_frame_is_rim_frame(node, PortType::Input)?
        {
            node.node_attr_mut().set_property(
                CLEAR_APERTURE,
                ClearAperture::from_aperture_shape(hole.shape())?.into(),
            )?;
            port_apertures.retain(|(.., aperture)| *aperture != hole);
        }
        fit_clear_aperture_to_curvature(node)?;
    }
    join_port_apertures(node, port_apertures)
}

/// Let the given port apertures of a node join its clear aperture.
///
/// A port aperture masked the light that passed its port. In the clear aperture a binary
/// obstruction becomes a bore through the component. A port aperture is dropped if the node has no
/// bounded clear aperture, if it is no binary outline (soft or stacked), if it was measured in
/// another frame than the clear aperture (an off-axis parabola, a tilted face), if it is a further
/// hole (only the hole at the entrance states the edge of a component), or if the bores do not lie
/// apart within that edge. A warning names each port aperture and what became of it.
///
/// # Errors
///
/// This function returns an error if the clear aperture cannot be set, or the frames of the node
/// cannot be derived.
fn join_port_apertures(
    node: &mut dyn OpticNode,
    apertures: Vec<(PortType, String, Aperture)>,
) -> OpmResult<()> {
    if apertures.is_empty() {
        return Ok(());
    }
    let node_info = node.node_info();
    let Ok(Proptype::ClearAperture(ClearAperture::Bounded(outline))) =
        node.node_attr().get_property(CLEAR_APERTURE).cloned()
    else {
        for (port_type, name, _) in apertures {
            warn!(
                "the aperture of {port_type} port '{name}' of {node_info} was dropped: the node \
                 has no bounded clear aperture; a dummy with one next to it limits the light \
                 instead"
            );
        }
        return Ok(());
    };
    let mut bores = outline.bores().to_vec();
    let mut joined = Vec::new();
    for (port_type, name, aperture) in apertures {
        let port = format!("{port_type} port '{name}'");
        let dropped = if !aperture.shape().is_binary() {
            Some("it is no binary outline")
        } else if !port_frame_is_rim_frame(node, port_type)? {
            Some("it was measured in another frame than the clear aperture")
        } else if *aperture.aperture_type() == ApertureType::Hole {
            Some("only the hole at the entrance of a component states its edge")
        } else {
            None
        };
        if let Some(reason) = dropped {
            warn!("the aperture of {port} of {node_info} was dropped: {reason}");
        } else {
            bores.push(Bore::from_aperture(&aperture)?);
            joined.push(port);
        }
    }
    if joined.is_empty() {
        return Ok(());
    }
    let outcome = match Outline::new(outline.shape().clone(), bores) {
        Ok(bored) => {
            node.node_attr_mut()
                .set_property(CLEAR_APERTURE, ClearAperture::from(bored).into())?;
            "joined the clear aperture as a bore: rays through it now miss the component"
                .to_string()
        }
        Err(e) => format!("was dropped: it does not lie as a bore within the clear aperture ({e})"),
    };
    for port in joined {
        warn!("the aperture of {port} of {node_info} {outcome}");
    }
    Ok(())
}

/// Return the aperture the physical entrance `input_1` of a node held if it outlines a region the
/// way a clear aperture does: a centred binary hole.
///
/// # Arguments
///
/// * `port_apertures` - the physical type, the name and the aperture of every port of the node
///   that held one.
fn entrance_hole(port_apertures: &[(PortType, String, Aperture)]) -> Option<Aperture> {
    let (.., aperture) = port_apertures
        .iter()
        .find(|(port_type, name, _)| *port_type == PortType::Input && name == "input_1")?;
    (*aperture.aperture_type() == ApertureType::Hole
        && aperture.shape().is_binary()
        && aperture.isometry().is_none())
    .then(|| aperture.clone())
}

/// Return whether the apertures of a node's physical ports of the given type were measured in the
/// frame its clear aperture is measured in, but for a shift along the axis. Only then does a port
/// aperture outline the same region as a clear aperture of its shape; it does not for an off-axis
/// parabola or at the tilted exit of a wedge.
///
/// The frames do not depend on the size of the clear aperture, so they are taken with a tiny one
/// that any curvature reaches, and the declared one is put back.
///
/// # Arguments
///
/// * `node` - the node.
/// * `port_type` - the physical type of the ports: input ports sit at the entrance surface,
///   output ports at the exit surface.
///
/// # Errors
///
/// This function returns an error if the clear aperture cannot be set or the geometry cannot be
/// derived.
fn port_frame_is_rim_frame(node: &mut dyn OpticNode, port_type: PortType) -> OpmResult<bool> {
    let declared = node.node_attr().get_property(CLEAR_APERTURE)?.clone();
    let probe = ClearAperture::from(CircleShape::new(nanometer!(1.0))?);
    node.node_attr_mut()
        .set_property(CLEAR_APERTURE, probe.into())?;
    let frames = port_and_rim_frames(node, port_type);
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

/// Return the frame the apertures of a node's ports of the given type are measured in and the
/// frame the rim is measured in, both relative to the node, or `None` if the node has no rim.
///
/// # Errors
///
/// This function returns an error if the geometry cannot be derived.
fn port_and_rim_frames(
    node: &dyn OpticNode,
    port_type: PortType,
) -> OpmResult<Option<(Isometry, Isometry)>> {
    let Some(geometry) = node.geometry()? else {
        return Ok(None);
    };
    let ((_, entrance), (_, exit)) = geometry.entrance_and_exit(&Isometry::identity())?;
    let port_frame = match port_type {
        PortType::Input => entrance,
        PortType::Output => exit,
    };
    Ok(geometry
        .rim()?
        .map(|rim| (port_frame, rim.frame(&Isometry::identity()))))
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
        ClearAperture::from(CircleShape::new(tightest)?).into(),
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
                    Ok(Proptype::ClearAperture(clear_aperture))
                        if *clear_aperture == default_clear_aperture()
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
        apertures::{
            Aperture, ApertureShape, ApertureType, Bore, CircleShape, ClearAperture, GaussianShape,
            Outline, OutlineShape, StackShape,
        },
        core_optics::{NodeAttrExt, OpticNode, OpticRef, PortType, node_attr::HasNodeAttr},
        degree,
        error::{OpmResult, OpossumError},
        geometry::body::{CLEAR_APERTURE, default_clear_aperture},
        millimeter,
        nodes::{
            BeamSplitter, Dummy, Lens, NodeGroup, ParabolicMirror, RayPropagationVisualizer,
            SourcePort, SpotDiagram, ThinMirror, Wedge,
        },
        opm_document::OpmDocument,
        properties::Proptype,
        refractive_index::RefrIndexConst,
    };
    use approx::assert_relative_eq;
    use num_traits::Zero;
    use ron::extensions::Extensions;
    use serde::Serialize;
    use std::{collections::BTreeSet, path::Path};
    use uom::si::{energy::joule, f64::Angle};
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
    /// The text a file holds for the value.
    fn ron_of<T: Serialize>(value: &T) -> OpmResult<String> {
        ron::Options::default()
            .with_default_extension(Extensions::UNWRAP_VARIANT_NEWTYPES)
            .to_string(value)
            .map_err(|e| OpossumError::Other(e.to_string()))
    }
    /// The file a program writing format version `version` would have written for the document,
    /// compact, with every clear aperture equal to `declared` stated as the aperture shape `old`,
    /// the way files did before the clear aperture had a type of its own.
    fn file_with_old_clear_aperture(
        document: &OpmDocument,
        version: &str,
        declared: &ClearAperture,
        old: &ApertureShape,
    ) -> OpmResult<String> {
        let mut document = document.clone();
        document.prepare_materials_for_serialization()?;
        document.opm_file_version = version.into();
        let declared = ron_of(&Proptype::ClearAperture(declared.clone()))?;
        let file = ron_of(&document)?;
        assert!(
            file.contains(&declared),
            "the file states the clear aperture"
        );
        Ok(format!(
            "#![enable(unwrap_variant_newtypes)]\n{}",
            file.replace(&declared, &ron_of(&Proptype::Aperture(old.clone()))?)
        ))
    }
    /// The file a program writing format version `version` would have written for the document,
    /// with each of the given apertures in a port of a node the way format version 0 wrote it:
    /// beside the coating and the LIDT of the port.
    ///
    /// # Arguments
    ///
    /// * `port_apertures` - the id of the node, the physical type and the name of the port, and
    ///   the aperture.
    fn file_with(
        document: &OpmDocument,
        version: &str,
        port_apertures: &[(Uuid, PortType, &str, Aperture)],
    ) -> OpmResult<String> {
        let mut file = file_of(document, version)?;
        let ids: BTreeSet<Uuid> = port_apertures.iter().map(|(id, ..)| *id).collect();
        for id in ids {
            let node = document.scenery().node_recursive(id)?.0;
            let mut ports = node.node_attr().raw_ports().clone();
            let apertures = port_apertures.iter().filter(|(node, ..)| *node == id);
            for (_, port_type, name, _) in apertures.clone() {
                if ports.ports_raw(port_type).get(*name).is_none() {
                    ports.add(port_type, name)?;
                }
            }
            let mut text = ron_of(&ports)?;
            for (_, _, name, aperture) in apertures {
                let port = format!("\"{name}\":(");
                text = text.replacen(&port, &format!("{port}aperture:{},", ron_of(aperture)?), 1);
            }
            // The ports of a node stand between the line with its name and its id.
            let at = file
                .find(&format!("\"uuid\": \"{id}\""))
                .expect("the file holds the node");
            let name = file[..at].rfind("\"name\": ").expect("the node has a name");
            let ports_at = name + file[name..].find('\n').expect("the name ends its line") + 1;
            file.replace_range(ports_at..at, &format!("\"ports\": {text}, "));
        }
        Ok(file)
    }
    /// Write the document in the given format version with the given apertures in the ports of
    /// the node with the given id (see [`file_with`]), read it back and return the node.
    fn read_back(
        document: &OpmDocument,
        version: &str,
        id: Uuid,
        apertures: &[(PortType, &str, Aperture)],
    ) -> OpmResult<OpticRef> {
        let port_apertures: Vec<_> = apertures
            .iter()
            .map(|(port_type, name, aperture)| (id, *port_type, *name, aperture.clone()))
            .collect();
        let read = OpmDocument::from_string(&file_with(document, version, &port_apertures)?)?;
        Ok(read.scenery().node_recursive(id)?.0)
    }
    /// Read a single node back from a file of format version 0, with the given apertures in its
    /// ports.
    fn upgraded<T: Analyzable + Clone + 'static>(
        node: T,
        apertures: &[(PortType, &str, Aperture)],
    ) -> OpmResult<OpticRef> {
        let (document, id) = document_with(node)?;
        read_back(&document, "0", id, apertures)
    }
    /// A circular hole of the given radius in mm.
    fn hole(radius: f64) -> OpmResult<Aperture> {
        Aperture::new_circle(millimeter!(radius), ApertureType::Hole, None)
    }
    /// The clear aperture of a node.
    fn clear_aperture(node: &OpticRef) -> ClearAperture {
        let Ok(Proptype::ClearAperture(clear_aperture)) =
            node.node_attr().get_property(CLEAR_APERTURE)
        else {
            panic!("the node has no clear aperture");
        };
        clear_aperture.clone()
    }
    /// A circle of the given radius in mm.
    fn circle(radius: f64) -> OpmResult<ClearAperture> {
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
    /// Before the clear aperture had a type of its own, a file stated it as an aperture shape. It
    /// is read whatever the version of the file: a shape with an edge becomes the edge of the
    /// component, a stack of one centred hole and obstructions in it an edge with bores. A shape
    /// no edge with bores can state, or one the node refuses, leaves the default, with a warning
    /// naming the node.
    #[test]
    fn a_clear_aperture_stated_as_an_aperture_shape_is_read() -> OpmResult<()> {
        let center = millimeter!(3.0, 0.0);
        let obstruction =
            Aperture::new_circle(millimeter!(2.0), ApertureType::Obstruction, Some(center))?;
        let ring = ApertureShape::Stack(StackShape::new(vec![hole(8.0)?, obstruction])?);
        let bore = Bore::new(CircleShape::new(millimeter!(2.0))?, center, Angle::zero());
        let bored = Outline::new(CircleShape::new(millimeter!(8.0))?, vec![bore])?;
        let two_holes = ApertureShape::Stack(StackShape::new(vec![hole(8.0)?, hole(5.0)?])?);
        let soft = GaussianShape::new((millimeter!(5.0), millimeter!(5.0)))?;
        let cases = [
            (
                ApertureShape::from(CircleShape::new(millimeter!(5.0))?),
                Some(circle(5.0)?),
            ),
            (ring, Some(bored.into())),
            (two_holes, None),
            (ApertureShape::Gaussian(soft), None),
            (ApertureShape::Open, None),
        ];
        let (document, id) = document_with(ThinMirror::default())?;
        for version in ["0", env!("OPM_FILE_VERSION")] {
            for (old, upgraded) in &cases {
                let file = file_with_old_clear_aperture(
                    &document,
                    version,
                    &default_clear_aperture(),
                    old,
                )?;
                testing_logger::setup();
                let mirror = OpmDocument::from_string(&file)?
                    .scenery()
                    .node_recursive(id)?
                    .0;
                testing_logger::validate(|logs| {
                    let warned = logs.iter().any(|log| {
                        log.level == log::Level::Warn
                            && log.body.contains("'mirror'")
                            && log.body.contains("clear aperture")
                    });
                    assert_eq!(warned, upgraded.is_none(), "{old:?} in version {version}");
                });
                assert_eq!(
                    clear_aperture(&mirror),
                    upgraded.clone().unwrap_or_else(default_clear_aperture),
                    "{old:?} in version {version}"
                );
            }
        }
        // A dummy may be open, so it keeps an open clear aperture and takes a shape for one.
        let (document, id) = document_with(Dummy::default())?;
        for (old, upgraded) in [
            (ApertureShape::Open, ClearAperture::Open),
            (CircleShape::new(millimeter!(5.0))?.into(), circle(5.0)?),
        ] {
            let file = file_with_old_clear_aperture(
                &document,
                env!("OPM_FILE_VERSION"),
                &ClearAperture::Open,
                &old,
            )?;
            let dummy = OpmDocument::from_string(&file)?
                .scenery()
                .node_recursive(id)?
                .0;
            assert_eq!(clear_aperture(&dummy), upgraded, "{old:?}");
        }
        Ok(())
    }
    /// In format version 0 a binary hole at the entrance of a component stated how large the
    /// component is. It becomes the clear aperture, also for the other ports holding the same hole.
    /// A detector records within the hole from now on, and a dummy becomes an iris.
    #[test]
    fn a_port_hole_becomes_the_clear_aperture() -> OpmResult<()> {
        let lens = upgraded(
            Lens::default(),
            &[
                (PortType::Input, "input_1", hole(5.0)?),
                (PortType::Output, "output_1", hole(5.0)?),
            ],
        )?;
        assert_eq!(clear_aperture(&lens), circle(5.0)?);

        let splitter = upgraded(
            BeamSplitter::default(),
            &[
                (PortType::Input, "input_1", hole(5.0)?),
                (PortType::Input, "input_2", hole(5.0)?),
            ],
        )?;
        assert_eq!(clear_aperture(&splitter), circle(5.0)?);

        for node in [
            OpticRef::new(Box::new(ThinMirror::default())),
            OpticRef::new(Box::new(SpotDiagram::default())),
            OpticRef::new(Box::new(Dummy::default())),
        ] {
            let mut scenery = NodeGroup::default();
            let id = scenery.add_node_ref(node)?;
            let node = read_back(
                &OpmDocument::new(scenery),
                "0",
                id,
                &[(PortType::Input, "input_1", hole(5.0)?)],
            )?;
            assert_eq!(clear_aperture(&node), circle(5.0)?, "{}", node.node_type());
        }
        Ok(())
    }
    /// A binary port obstruction becomes a bore through the clear aperture, shifted the way the
    /// obstruction was. A warning names the node and the port.
    #[test]
    fn a_port_obstruction_becomes_a_bore() -> OpmResult<()> {
        for center in [millimeter!(0.0, 0.0), millimeter!(5.0, 0.0)] {
            let obstruction =
                Aperture::new_circle(millimeter!(2.0), ApertureType::Obstruction, Some(center))?;
            testing_logger::setup();
            let lens = upgraded(
                Lens::default(),
                &[(PortType::Input, "input_1", obstruction)],
            )?;
            testing_logger::validate(|logs| {
                assert!(logs.iter().any(|log| log.level == log::Level::Warn
                    && log.body.contains("'input_1' of 'lens'")
                    && log.body.contains("joined")));
            });
            let bore = Bore::new(CircleShape::new(millimeter!(2.0))?, center, Angle::zero());
            let bored = Outline::new(CircleShape::new(millimeter!(12.5))?, vec![bore])?;
            assert_eq!(clear_aperture(&lens), bored.into(), "{center:?}");
        }
        Ok(())
    }
    /// A port aperture joins only if it is a binary obstruction in the frame the clear aperture is
    /// measured in. A soft or a stacked aperture is no binary outline; a hole other than the
    /// centred one at the entrance would cut the edge of the component, which a bore cannot state;
    /// the aperture of an off-axis parabola is measured around the off-axis point, the one at the
    /// tilted exit of a wedge in that face, while their clear apertures are measured along the
    /// axis. Each is dropped with a warning naming the node and the port, and the clear aperture
    /// stays as it was.
    #[test]
    fn port_apertures_that_cannot_join_are_dropped() -> OpmResult<()> {
        let soft = Aperture::new_gaussian(
            (millimeter!(5.0), millimeter!(5.0)),
            ApertureType::Hole,
            None,
            None,
        )?;
        let obstruction = Aperture::new_circle(millimeter!(2.0), ApertureType::Obstruction, None)?;
        let ring = StackShape::new(vec![hole(8.0)?, obstruction])?;
        let ring = Aperture::new(ApertureShape::Stack(ring), ApertureType::Hole, None, None)?;
        let decentred = Aperture::new_circle(
            millimeter!(5.0),
            ApertureType::Hole,
            Some(millimeter!(1.0, 0.0)),
        )?;
        let mut ringed = Lens::default();
        ringed.node_attr_mut().set_name("ringed lens");
        let mut holed = Lens::default();
        holed.node_attr_mut().set_name("holed lens");
        let parabola =
            ParabolicMirror::new_with_off_axis_x("oap", millimeter!(100.0), true, degree!(90.0))?;
        let wedge = Wedge::new(
            "wedge",
            millimeter!(10.0),
            degree!(5.0),
            RefrIndexConst::new(1.5)?,
        )?;
        let mut scenery = NodeGroup::default();
        let lens = scenery.add_node(Lens::default())?;
        let ringed = scenery.add_node(ringed)?;
        let holed = scenery.add_node(holed)?;
        let parabola = scenery.add_node(parabola)?;
        let wedge = scenery.add_node(wedge)?;
        let file = file_with(
            &OpmDocument::new(scenery),
            "0",
            &[
                (lens, PortType::Input, "input_1", soft),
                (ringed, PortType::Input, "input_1", ring),
                (holed, PortType::Input, "input_1", decentred),
                (parabola, PortType::Input, "input_1", hole(5.0)?),
                (wedge, PortType::Output, "output_1", hole(5.0)?),
            ],
        )?;
        let dropped = [
            (lens, "'lens'", "input_1"),
            (ringed, "'ringed lens'", "input_1"),
            (holed, "'holed lens'", "input_1"),
            (parabola, "'oap'", "input_1"),
            (wedge, "'wedge'", "output_1"),
        ];
        testing_logger::setup();
        let read = OpmDocument::from_string(&file)?;
        testing_logger::validate(|logs| {
            for (_, name, port) in dropped {
                assert!(
                    logs.iter().any(|log| log.level == log::Level::Warn
                        && log.body.contains(name)
                        && log.body.contains(port)
                        && log.body.contains("dropped")),
                    "{name}"
                );
            }
        });
        for (id, name, _) in dropped {
            let node = read.scenery().node_recursive(id)?.0;
            assert_eq!(clear_aperture(&node), default_clear_aperture(), "{name}");
        }
        Ok(())
    }
    /// A bore has to lie within the clear aperture. An obstruction reaching 11 mm off the axis of a
    /// lens with a clear aperture of 9 mm does not: it is dropped with a warning, and the lens
    /// keeps its clear aperture and still loads.
    #[test]
    fn a_port_aperture_the_component_cannot_take_is_dropped() -> OpmResult<()> {
        let lens = Lens::new_with_clear_aperture(
            "strong lens",
            millimeter!(10.0),
            millimeter!(f64::INFINITY),
            millimeter!(5.0),
            RefrIndexConst::new(1.5)?,
            circle(9.0)?,
        )?;
        let beyond = Aperture::new_circle(
            millimeter!(6.0),
            ApertureType::Obstruction,
            Some(millimeter!(5.0, 0.0)),
        )?;
        testing_logger::setup();
        let lens = upgraded(lens, &[(PortType::Input, "input_1", beyond)])?;
        testing_logger::validate(|logs| {
            assert!(logs.iter().any(|log| log.level == log::Level::Warn
                && log.body.contains("'input_1' of 'strong lens'")
                && log.body.contains("dropped")));
        });
        assert_eq!(clear_aperture(&lens), circle(9.0)?);
        Ok(())
    }
    /// A node without a clear aperture has no outline a port aperture could join: it is dropped,
    /// with a warning naming the node and the port.
    #[test]
    fn a_node_without_a_clear_aperture_drops_its_port_apertures() -> OpmResult<()> {
        let mut group = NodeGroup::new("group");
        let inside = group.add_node(Dummy::default())?;
        group.map_input_port(inside, "input_1", "input_1")?;
        let mut scenery = NodeGroup::default();
        let visualizer = scenery.add_node(RayPropagationVisualizer::new("visualizer", None)?)?;
        let source = scenery.add_node(SourcePort::new("source"))?;
        let group = scenery.add_node(group)?;
        let file = file_with(
            &OpmDocument::new(scenery),
            "0",
            &[
                (visualizer, PortType::Input, "input_1", hole(5.0)?),
                (source, PortType::Output, "output_1", hole(5.0)?),
                (group, PortType::Input, "input_1", hole(5.0)?),
            ],
        )?;
        testing_logger::setup();
        OpmDocument::from_string(&file)?;
        testing_logger::validate(|logs| {
            for name in ["'visualizer'", "'source'", "'group'"] {
                assert!(
                    logs.iter().any(|log| log.level == log::Level::Warn
                        && log.body.contains(name)
                        && log.body.contains("dropped")),
                    "{name}"
                );
            }
        });
        Ok(())
    }
    /// Nodes in nested groups are upgraded too.
    #[test]
    fn a_node_in_a_nested_group_is_upgraded() -> OpmResult<()> {
        let mut inner = NodeGroup::new("inner");
        let id = inner.add_node(Lens::default())?;
        let mut outer = NodeGroup::new("outer");
        outer.add_node(inner)?;
        let (document, _) = document_with(outer)?;
        let lens = read_back(
            &document,
            "0",
            id,
            &[(PortType::Input, "input_1", hole(5.0)?)],
        )?;
        assert_eq!(clear_aperture(&lens), circle(5.0)?);
        Ok(())
    }
    /// What counts is the hole at the physical entrance `input_1`, also for an inverted node.
    #[test]
    fn an_inverted_node_moves_the_hole_of_its_physical_entrance() -> OpmResult<()> {
        let mut lens = Lens::default();
        lens.set_inverted(true)?;
        let lens = upgraded(lens, &[(PortType::Input, "input_1", hole(5.0)?)])?;
        assert_eq!(clear_aperture(&lens), circle(5.0)?);
        Ok(())
    }
    /// A file of the current version or a newer one is not upgraded: a port hole in it does not
    /// become the clear aperture, also when an older file was read just before.
    #[test]
    fn only_a_file_of_an_older_version_is_upgraded() -> OpmResult<()> {
        let (document, id) = document_with(Lens::default())?;
        let holes = [(PortType::Input, "input_1", hole(5.0)?)];
        assert_eq!(
            clear_aperture(&read_back(&document, "0", id, &holes)?),
            circle(5.0)?
        );
        for version in [env!("OPM_FILE_VERSION"), "1000"] {
            let lens = read_back(&document, version, id, &holes)?;
            assert_eq!(clear_aperture(&lens), default_clear_aperture(), "{version}");
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
        let mirror = scenery.add_node(ThinMirror::default())?;
        let camera = scenery.add_node(SpotDiagram::new("camera")?)?;
        let file = file_with(
            &OpmDocument::new(scenery),
            "0",
            &[
                (mirror, PortType::Input, "input_1", hole(5.0)?),
                (camera, PortType::Input, "input_1", hole(5.0)?),
            ],
        )?;
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
    /// A model the program of file format version 0 wrote (`main` at `e4bcc3a5`), with apertures
    /// in its ports, is upgraded as a whole. The decentred holes of a mirror and of an iris cannot
    /// become a clear aperture, which is centred, and are dropped with the port apertures of its
    /// unconnected nodes; the other port apertures measure what that program measured.
    ///
    /// Of the 127 rays of the hexapolar source (rings of 6 k rays at k · 1.667 mm, the first ray
    /// of each on +y), the lens sized by a port hole of 8 mm passes rings 0 to 4, 61 rays, and the
    /// mirror and the iris pass all they get. The filter, whose hole of 6 mm sits where the
    /// converging beam has shrunk by about 4 %, passes rings 0 to 3, 37 rays, less the 2 on +y at
    /// about 1.6 mm and 3.2 mm that its obstruction of 1.5 mm at (0, 3) mm stops: 35. The camera's
    /// hole of 2 mm takes rings 0 and 1 but for the stopped ray, 6 rays. Every ray lies at least
    /// 0.1 mm off an edge. The filter halves the energy.
    #[test]
    fn a_model_written_as_version_0_is_upgraded_as_a_whole() -> OpmResult<()> {
        testing_logger::setup();
        let mut document = OpmDocument::from_file(Path::new(
            "./files_for_testing/opm/version_0_port_apertures.opm",
        ))?;
        assert_eq!(document.opm_file_version, env!("OPM_FILE_VERSION"));
        testing_logger::validate(|logs| {
            for (name, outcome) in [
                ("mirror with a decentred hole", "dropped"),
                ("filter with a hole and an obstruction", "joined"),
                ("iris", "dropped"),
                ("source with a wide aperture", "dropped"),
                ("lens with a soft aperture", "dropped"),
                ("lens with a stacked aperture", "dropped"),
                ("off-axis parabola with a hole", "dropped"),
                ("wedge with a hole at its exit", "dropped"),
                ("visualizer with a hole", "dropped"),
                ("group with a port aperture", "dropped"),
            ] {
                assert!(
                    logs.iter().any(|log| log.level == log::Level::Warn
                        && log.body.contains(&format!("'{name}'"))
                        && log.body.contains(outcome)),
                    "the aperture of '{name}' is not reported as {outcome}"
                );
            }
        });
        let node = |name: &str| {
            let nodes = document.scenery().nodes();
            let found = nodes.into_iter().find(|node| node.name() == name);
            found
                .cloned()
                .unwrap_or_else(|| panic!("'{name}' is missing"))
        };
        assert_eq!(
            clear_aperture(&node("lens sized by a port hole")),
            circle(8.0)?
        );
        assert_eq!(clear_aperture(&node("camera with a hole")), circle(2.0)?);
        let bore = Bore::new(
            CircleShape::new(millimeter!(1.5))?,
            millimeter!(0.0, 3.0),
            Angle::zero(),
        );
        assert_eq!(
            clear_aperture(&node("filter with a hole and an obstruction")),
            Outline::new(CircleShape::new(millimeter!(6.0))?, vec![bore])?.into()
        );
        assert_eq!(
            clear_aperture(&node("mirror with a decentred hole")),
            default_clear_aperture()
        );
        assert_eq!(clear_aperture(&node("iris")), ClearAperture::Open);
        assert!(matches!(
            clear_aperture(&node("spot diagram with a polygon hole")),
            ClearAperture::Bounded(outline) if matches!(outline.shape(), OutlineShape::Polygon(_))
        ));

        let reports = document.analyze()?;
        for (meter, rays, transmission) in [
            ("meter behind the lens", 61.0, 1.0),
            ("meter behind the mirror", 61.0, 1.0),
            ("meter behind the filter", 35.0, 0.5),
            ("meter behind the iris", 35.0, 0.5),
            ("camera with a hole", 6.0, 0.5),
        ] {
            let measured = reports
                .iter()
                .flat_map(|report| report.node_reports())
                .find(|report| report.name() == meter)
                .and_then(|report| match report.properties().get("Energy") {
                    Ok(Proptype::Energy(energy)) => Some(energy.get::<joule>()),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("'{meter}' measured no energy"));
            assert_relative_eq!(measured, rays * transmission / 127.0, max_relative = 1e-9);
        }
        Ok(())
    }
}
