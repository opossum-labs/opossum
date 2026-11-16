//! The light files a 3D view was last told about, kept so that fetching them does not position
//! the model again.

use opossum_scene::digest;

/// A file of drawn light, with the digest a manifest names it by.
#[derive(Debug, Clone)]
pub struct DigestedFile {
    digest: String,
    glb: Vec<u8>,
}

impl DigestedFile {
    /// Wraps the GLB bytes of drawn light with their digest (see [`digest`]).
    ///
    /// # Arguments
    ///
    /// - `glb`: the file
    ///
    /// # Returns
    ///
    /// The file with its digest.
    #[must_use]
    pub fn new(glb: Vec<u8>) -> Self {
        Self {
            digest: digest(&glb),
            glb,
        }
    }

    /// Returns the file if `digest` names it.
    ///
    /// # Arguments
    ///
    /// - `digest`: the digest a viewer asks for the file by, if any
    ///
    /// # Returns
    ///
    /// A copy of the file, or `None` if `digest` names another one or none.
    #[must_use]
    pub fn named(&self, digest: Option<&str>) -> Option<Vec<u8>> {
        (digest == Some(self.digest.as_str())).then(|| self.glb.clone())
    }
}

/// The light files the last `/scene/manifest` built and named by their digests.
///
/// A viewer fetches exactly the files the manifest named, so serving them from here spares a
/// second positioning run and a second trace for each of them. A file is served only by its own
/// digest, so a stale one is never handed out in place of another.
#[derive(Debug, Clone, Default)]
pub struct DrawnLight {
    /// The optical axis, or `None` if the manifest built none.
    pub axis: Option<DigestedFile>,
    /// The rays, or `None` if the manifest built none.
    pub rays: Option<DigestedFile>,
}
