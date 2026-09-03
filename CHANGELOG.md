# Changelog

All notable changes to this project will be documented in this file.

OPOSSUM is funded through THRILL (EU, grant agreement No 101095207) and LASE-FUSE (BMFTR, funding reference 13F1041); see [Funding & Acknowledgments](doc/book/src/concepts/background/funding.md). To cite OPOSSUM, use [`CITATION.cff`](CITATION.cff).

## [0.7.3] - 2026-08-14

### <!-- 0 -->Features

- :sparkles: OPM files can now be saved in WASM builds - ([4c37866](https://github.com/opossum-labs/opossum/commit/4c37866b159974f0889cc78d9c7b6c5ea460c7ea))


### <!-- 1 -->Bug Fixes

- :bug: Make PortMap survive serde Content-buffering during tolerant node deserialization - ([47cdbfd](https://github.com/opossum-labs/opossum/commit/47cdbfdcbfff9366770574ef83eba33725a5259c))


### <!-- 6 -->Miscellaneous Tasks

- :bookmark: bump version 0.7.3 - ([e45e5fa](https://github.com/opossum-labs/opossum/commit/e45e5fa574265df34a7e5b73e9235f09bcc10f6a))


## [0.7.2] - 2026-08-06

### <!-- 0 -->Features

- :sparkles: Show warning if a node cannot be placed without a valid optical axis. - ([9ac68d6](https://github.com/opossum-labs/opossum/commit/9ac68d6f87ff4d919f3fb082909fd312e5cc0483))

- :sparkles: Connections that target a reference are now hidden - ([a70c48c](https://github.com/opossum-labs/opossum/commit/a70c48c70cf5be2c57fd5f8d099952f51dfe2ea2))

- :sparkles: Graph is now dragged using the middle mouse button - ([93a06dc](https://github.com/opossum-labs/opossum/commit/93a06dc73393fce6db64ce6fa877219c93ed3a83))

- :sparkles: GUI now checks, if newer version is available - ([dac0872](https://github.com/opossum-labs/opossum/commit/dac0872f6a79886bfebfd0de547e57082edcefda))

- :sparkles: Doubleclick on group nodes opens new tab - ([9857b4a](https://github.com/opossum-labs/opossum/commit/9857b4a95401191561238e1b49cfd00c07872aff))

- :sparkles: Name of group tab changes when group is renamed - ([218d93e](https://github.com/opossum-labs/opossum/commit/218d93ec27d25e0c9490405c3803f5008d79b705))

- :sparkles: Breadcrumbs for group navigation introduced - ([addf75b](https://github.com/opossum-labs/opossum/commit/addf75bf3fd3bccf0d31075f8e135b31248739b3))

- :sparkles: Selection of multiple nodes implemented - ([f31a47a](https://github.com/opossum-labs/opossum/commit/f31a47abca1fb725ea088d8de344e63b4b71e7ae))

- :sparkles: Implemented functionality to convert nodes to a group - ([13fac6d](https://github.com/opossum-labs/opossum/commit/13fac6d6a6bf0fda24b104657216d7d8fd9f4bd2))

- :sparkles: analyzers only aloowed to paste/add in root - ([6f1bd43](https://github.com/opossum-labs/opossum/commit/6f1bd439697d66ce58c711adce272548f7fcdcac))

- :sparkles: Groups can now be copied - ([ec2c821](https://github.com/opossum-labs/opossum/commit/ec2c8214ab5895842ac176c3143e8467ea7098da))

- :sparkles: PortMaps are now visualized in- and outside of groups - ([2bcf0ad](https://github.com/opossum-labs/opossum/commit/2bcf0ad9149633a8da7e18a968e4249420b0cf62))

- :sparkles: Groups can now be used in the GUI - ([54a4fbf](https://github.com/opossum-labs/opossum/commit/54a4fbf6dd109ec9b46393ff93b8603584096c90))

- :sparkles: Cutting out nodes using ctrl+x - ([bb4b95f](https://github.com/opossum-labs/opossum/commit/bb4b95f42c8c73e6ffe47157fbf3eff8f0f3708f))

- :sparkles: Add button to open reports in the browser after the simulation - ([4a70115](https://github.com/opossum-labs/opossum/commit/4a70115eea5d022ad79386a1987826a55f84b68b))

- :sparkles: Add the source port mapping feature to the backend. - ([bf43a11](https://github.com/opossum-labs/opossum/commit/bf43a1164663dcec33505e0adf10948141a9abad))

- :sparkles: Add configuration for optic ports to GUI (so far LIDT & Coating) - ([0c35b5f](https://github.com/opossum-labs/opossum/commit/0c35b5ffa667189bd415fc8931a1b4309333bdd2))

- :sparkles: Add option to skip using SI prefixes in UnitInput component - ([909573e](https://github.com/opossum-labs/opossum/commit/909573e2140b4f7c2389dde0c126e5006002b0b5))

- :sparkles: Added ApertureConfig to PortConfig Editor - ([656cc09](https://github.com/opossum-labs/opossum/commit/656cc090b74d40ab773d88a225937c8f086a0e15))

- :sparkles: Aperture Editor implemented for PortConfig in GUI - ([d0eb8dc](https://github.com/opossum-labs/opossum/commit/d0eb8dcb8cbbc6b64fc70f0053f82ed854d618d7))

- :sparkles: Show error message during gui startup if backend could not be started before. - ([4cde9d9](https://github.com/opossum-labs/opossum/commit/4cde9d9c5329697b740b86202b21dbe98c3a0606))

- :sparkles: The wavefront monitor now has an option to automatically subtract a wavefront tilt. - ([246a1ff](https://github.com/opossum-labs/opossum/commit/246a1ff735e175382b9d5d3cf92ba8381fe81467))

- :sparkles: Do not show standard context menu of the browser in release builds - ([1b22c54](https://github.com/opossum-labs/opossum/commit/1b22c548ddeac7cdf5c5c87dad710db89fa4f873))

- :sparkles: Strongly improve auto layout algorithm - ([81be232](https://github.com/opossum-labs/opossum/commit/81be232a6c9e9d5e938eb7e3a3b41e49202c685b))

- :sparkles: Perform an auto layout if an opm file contains nodes with no GUI coordinates. - ([a9f7afc](https://github.com/opossum-labs/opossum/commit/a9f7afc8af8f74263c82118e0a9017d3e8710e1c))

- :sparkles: Installer bundles now contain some example files. - ([0599f7c](https://github.com/opossum-labs/opossum/commit/0599f7c69deb5d14c428f141cded3f3a780cb380))

- :sparkles: Imrprove handling and serialization of aperture isometries. - ([5a091ab](https://github.com/opossum-labs/opossum/commit/5a091ab4e5f330b48919b7a9ca3dd22e29338475))

- :sparkles: Add application settings dialog - ([3c0c9c0](https://github.com/opossum-labs/opossum/commit/3c0c9c0f5b4ef7fabff7fb61feb2f08f48775a62))

- :sparkles: Significantly improve handling of report directories in gui. - ([568feee](https://github.com/opossum-labs/opossum/commit/568feee8257b90e9674d762d0b17e944b780e979))

- :memo: Documentation: Fluence.md - ([5e77bba](https://github.com/opossum-labs/opossum/commit/5e77bba63e46d22930b5073452bb956a0b61d76c))

- :memo: Documentaion: workshop_11_ghostfocus.rs - ([281c76c](https://github.com/opossum-labs/opossum/commit/281c76cfa128e3d2caf07df93ac95c3a5bb11a87))

- :sparkles: - ([808a078](https://github.com/opossum-labs/opossum/commit/808a078c2ddca1c476a610c63828e6145a80b9ed))

- :sparkles: Add missing editor for Vec3 property type. - ([460b0b3](https://github.com/opossum-labs/opossum/commit/460b0b360cd91b5024e3b58b5424d184ae96695f))

- :sparkles: Reading spectrum from csv file is now much more tolerant. - ([f795e38](https://github.com/opossum-labs/opossum/commit/f795e382d0ed40de9d0ba54d5f97ecabe7b33909))

- :sparkles: Make reading of model files more fault tolerant. - ([5bf5ea6](https://github.com/opossum-labs/opossum/commit/5bf5ea6d8c42b69ecbbaf0445924bcc9c59cf997))

- :sparkles: Add a refresh button to re-synchronize frontend and backend. - ([24fb4c1](https://github.com/opossum-labs/opossum/commit/24fb4c118d09c3b0e9fb365fdb8a1c1d2150d558))


### <!-- 1 -->Bug Fixes

- :bug: Return an error when connecting an already mapped port - ([1ee507a](https://github.com/opossum-labs/opossum/commit/1ee507a903d42d1afb422d41edfebcb6799a39bb))

- :bug: Setting editor size when opening a new project - ([4fc2604](https://github.com/opossum-labs/opossum/commit/4fc26043240164a473a2eb818450cbce2bb6a059))

- :bug: Remove edge when not connected correctly - ([e676654](https://github.com/opossum-labs/opossum/commit/e676654554247e6a2739372400f628e05ba8c6e8))

- :bug: Deactive nodes when clicking on an edge input field - ([7b3dba3](https://github.com/opossum-labs/opossum/commit/7b3dba3b3a8cd31ff7781dfb9954a57413e6efd9))

- :bug: GUI fast again when adding lots of nodes - ([971b6ce](https://github.com/opossum-labs/opossum/commit/971b6ceacf4d141490ee55122725d2c700f5747d))

- :bug: Fixed missing edges when set of nodes is pasted into another group - ([a9128ea](https://github.com/opossum-labs/opossum/commit/a9128ea557af71c51e3450f580296979237dae48))

- :bug: Removed infinite recursion and deadlocks when copying grouped groups/references - ([c130594](https://github.com/opossum-labs/opossum/commit/c13059496ca6f1b30ec0baf1e8cb98db1ba8235d))

- :bug: Stabilized drop into group of nodes - ([2ed9406](https://github.com/opossum-labs/opossum/commit/2ed9406a97a23d4f5c08bdeb3f5b605d0ee113ac))

- :bug: OPM files with nested groups serialize correctly again - ([88746fb](https://github.com/opossum-labs/opossum/commit/88746fb3b61229173d2f04206cab774a8cc1e050))

- :bug: Using correct angle when setting grating alignment via littrow - ([b2ca217](https://github.com/opossum-labs/opossum/commit/b2ca217375f738cf7c84ed875315f80c484d89c4))

- :bug: Fix panic in OpticNode::update_surface - ([7c0b675](https://github.com/opossum-labs/opossum/commit/7c0b675296621e544b72c9dbf591fb27f3688d6e))

- :bug: properties usable again in GUI after backend refactor - ([1dfb552](https://github.com/opossum-labs/opossum/commit/1dfb552b5507e6da6afc4549c9aafff972c85c2d))

- :bug: References nods are now renamed when original node name changes - ([b0c53c4](https://github.com/opossum-labs/opossum/commit/b0c53c406f97022b23a2674f1b441ac6e5211db9))

- :bug: Do set file to status "needs saving" if centered or "zoom to fit". - ([88b9410](https://github.com/opossum-labs/opossum/commit/88b9410ec8daf35a9ff723972eb6af76018b2cb1))

- :bug: Fix wrong client API while deleting an analyzer. - ([198f5f3](https://github.com/opossum-labs/opossum/commit/198f5f3cb6016bcc4073adce0eced3c9198c4a2b))

- :bug: Fix wrong handling og apertures in GUI. - ([1292f1f](https://github.com/opossum-labs/opossum/commit/1292f1f955b7c00dbd799580f4bf9813553bf23c))

- :bug: Fix missing field name in edge distance input. - ([79e10a4](https://github.com/opossum-labs/opossum/commit/79e10a45b17159710d6394c7bf577a70ff3a26d4))

- :bug: Fix random order of analysis reports - ([90f8b0d](https://github.com/opossum-labs/opossum/commit/90f8b0df0abdee046940c216d64bb643f979dcb1))

- :bug: Fix converting a set of selected nodes to a group. - ([fa1f587](https://github.com/opossum-labs/opossum/commit/fa1f5874e328a26b80814523a1c1a4c7638987dd))

- :bug: Do not show `create reference` in context menu if multiple nodes are selected. - ([91b2fc5](https://github.com/opossum-labs/opossum/commit/91b2fc5fbff31a04844bfea07788bb56d2867672))

- :arrow_up: Update major version of vergen-git2 crate and update code accordingly. - ([ab94dfd](https://github.com/opossum-labs/opossum/commit/ab94dfd879793707dd0dc832e750d6d2af63adf3))

- :bug: Improve auto layout algorithm to further reduce connection crossings. - ([95dc161](https://github.com/opossum-labs/opossum/commit/95dc161b5c4a106826bcac869a518086913d9700))

- :bug: Fix missing highlighting and Cancel functionality in Settings dialog. - ([6892847](https://github.com/opossum-labs/opossum/commit/689284726a21c86c824ac294619b3460a1b6c0cb))

- :bug: Fix missing "needs saving" if port configuration of a node was changed. - ([858c2bc](https://github.com/opossum-labs/opossum/commit/858c2bc5a659051b33c2dbb93e92970a1c66723e))

- :bug: Fix that SourcePort did not consider apertures at output port. - ([d32151d](https://github.com/opossum-labs/opossum/commit/d32151d5849c3afb9fb4b54c653bf332a78d3166))

- :bug: Fix layout problem in node report (accordion collapse) - ([1b3d2f6](https://github.com/opossum-labs/opossum/commit/1b3d2f665b38823ca886de4380bbb1a5d384f82a))

- :bug: Fix error in sequential convert_to_group calls. Improve filtering out analyzer nodes from selection. - ([ba1a960](https://github.com/opossum-labs/opossum/commit/ba1a96053c585c5d0a387afa6e5676f370513ae9))

- :bug: Fix failing convert_to_group with more than one external connections. - ([342f5d3](https://github.com/opossum-labs/opossum/commit/342f5d363d79e87e1bc80d9d188cfcac1da73f96))

- :bug: Removing port map now only deletes correct connection - ([747a451](https://github.com/opossum-labs/opossum/commit/747a451195f0f8222fcf1f392a7c18e6f7865d06))

- :bug: Fix missing connection cleanup if nested nodes are deleted. - ([cd67a3e](https://github.com/opossum-labs/opossum/commit/cd67a3e9d1541cbecd5bc3566e8fc73bd3d5e8e6))

- :bug: Do not show menu entry Quit for web builds. - ([f4b7dc9](https://github.com/opossum-labs/opossum/commit/f4b7dc9beab7dfc409567d7a5c14d013b2381be2))

- :bug: Fix stale reference-node ports after a group's port mapping changes - ([a444389](https://github.com/opossum-labs/opossum/commit/a444389fa30f030cf0449f8f1461647c96c3c6ce))

- :bug: Forbid nesting a reference node inside the group it refers to - ([cfd403e](https://github.com/opossum-labs/opossum/commit/cfd403ecaa78792573bc34c79357233a9ecfec43))

- :bug: Fix compile / runtime errors in WASM builds - ([0233d02](https://github.com/opossum-labs/opossum/commit/0233d02eeff84189cfe9f16b5abf13d65d3398ea))

- :bug: Move aperture accordion auto-open out of use_memo, scope it per port - ([1004e7a](https://github.com/opossum-labs/opossum/commit/1004e7a0837013012741cd8714a66a5d6784c01d))

- :bug: Refresh analyzer source-port card list on undo/redo - ([f21b951](https://github.com/opossum-labs/opossum/commit/f21b951582d9dbec4fdbb4afe0e8fb9a14363e2b))


### <!-- 2 -->Refactor

- :recycle: Remove "synthetic" input port from Source node. - ([1c5a483](https://github.com/opossum-labs/opossum/commit/1c5a483a2f7d7115a6658f0ef08b24921ffe27b4))

- :recycle: Implement From<Ray> for Rays to simplify various code locations - ([d316b3f](https://github.com/opossum-labs/opossum/commit/d316b3fb5ac8d5869c40dcb275e0c5e684497109))

- :recycle: Remove center param from HexagonalTiling, moved to SourceIsometry - ([e95e9d9](https://github.com/opossum-labs/opossum/commit/e95e9d9c25976e9ea38a005c193d8cac4cc14479))

- :recycle: Move all distribution functions to a common module. - ([f0a9989](https://github.com/opossum-labs/opossum/commit/f0a9989e641f76b36fe8a46f551329f9a0b757cb))

- :recycle: Move OpticNode to core_optics - ([d77fa9c](https://github.com/opossum-labs/opossum/commit/d77fa9c91c088fdd9441d7b4439efb78c693c09e))

- :recycle: Move OpticPorts to core_optics module - ([37aa45b](https://github.com/opossum-labs/opossum/commit/37aa45bfd42ffe8cec1d4d4361e2ff8240505322))

- :recycle: Heavily change module hierachy. - ([cf50764](https://github.com/opossum-labs/opossum/commit/cf50764bc48a65781344b279d9edd09ebfe0c89b))

- :recycle: Heavily refactor optical port configuration. - ([93c443e](https://github.com/opossum-labs/opossum/commit/93c443ef00de20aa0878bc1fe8aa4230e9d1003f))

- :art: Improve structure of opm file format - ([43374f5](https://github.com/opossum-labs/opossum/commit/43374f5c87ec87342c3a0ef688c72f9a009c69f5))

- :recycle: Only passing on readSignals for GraphStore, GraphState, EditorState and Workspace - ([4af9a2a](https://github.com/opossum-labs/opossum/commit/4af9a2a50572015ef6dc8b09ceafa680c2ea5c5d))

- :recycle: Gui state structs are now implementes as dioxus stores - ([5034bba](https://github.com/opossum-labs/opossum/commit/5034bba11ac2c25b2afbb890b93ddfe399afd05b))

- :recycle: Imrpvoe various unit tests by removing unwrap() - ([632d4d5](https://github.com/opossum-labs/opossum/commit/632d4d5a558f465e591e84c148daa0e212afeafc))

- :recycle: Get rid of several unwrap() statements. - ([b09af85](https://github.com/opossum-labs/opossum/commit/b09af85e46a9a2469099714c4b0a373ac64b5b4e))

- :recycle: Heavily refacotr backend REST API - ([570abe9](https://github.com/opossum-labs/opossum/commit/570abe975bca31050fcbbd143001fa1688d3379e))

- :recycle: Imrpove handling of validated values for RefrIndexConst. - ([02a5740](https://github.com/opossum-labs/opossum/commit/02a574093f9a3979a1c9027f0e54036d564cac6e))

- :recycle: Use Ratio::percent for coating reflectivity - ([7854dfc](https://github.com/opossum-labs/opossum/commit/7854dfccb5ce86dfe7ec8c89b18c359cb30835f7))

- :recycle: Use Ratio for FilterType::Constant value. - ([f7b00c5](https://github.com/opossum-labs/opossum/commit/f7b00c50ae620f370c7a7aa9c0a19435ad4a3b8a))

- :recycle: OpticNode::node_report() now returns an OpmResult for better error handling. - ([0a8cfee](https://github.com/opossum-labs/opossum/commit/0a8cfee72e23efa52f3dc5eb5093f3761830268b))

- :recycle: Clean up responsibilitites and internal logic of WaveFrontMaps. - ([270ac8d](https://github.com/opossum-labs/opossum/commit/270ac8d0b94d48efbde7073ec151b37a942932b7))

- :recycle: Remove id from AnalyzerInfo - ([8c8f8da](https://github.com/opossum-labs/opossum/commit/8c8f8dace0c84b4f4341d80ae43b069e5bf7eea6))

- :rotating_light: Fix various linter warnings - ([2ea6222](https://github.com/opossum-labs/opossum/commit/2ea6222ff78cb49cf92ab102c498d7369c6339d9))

- :recycle: Extend OpmNode macro to include boiler plate impls for NodeAttr. - ([efbdffd](https://github.com/opossum-labs/opossum/commit/efbdffd7eeb12ee1120914dd8fdbf2639a4198e0))

- :recycle: Heavily refactored OpticNode trait by intorduction of extension traits and blanket implementations - ([22ea123](https://github.com/opossum-labs/opossum/commit/22ea1237aa860f16ce0af4d954cab7500681eb4e))


### <!-- 4 -->Documentation

- :memo: Document example workshop_02_kepler_real_lenses_chromatism.rs - ([5bfdab8](https://github.com/opossum-labs/opossum/commit/5bfdab8150cfba39c61046271bd2a3869e83c2aa))

- :memo: Edit docs- Delete option, selecting Nodes, Numeric Values - ([0ad4bea](https://github.com/opossum-labs/opossum/commit/0ad4beaf14ee083cec08b0b7963a0fd825aa18ca))

- :memo: Doc Update : Software Architecture - ([29083b1](https://github.com/opossum-labs/opossum/commit/29083b16ae82914df63a9bcdaf23d86e9aef0958))

- :memo: corrected spacing and typo errors - ([15dffb6](https://github.com/opossum-labs/opossum/commit/15dffb62ddccbb6ae2103d8754fcc2e0a0ce6efe))

- :memo: Document example workshop_00_kepler_paraxial.rs - ([daf3900](https://github.com/opossum-labs/opossum/commit/daf3900521d57303f53acea9ed1f44e15ca3de89))

- :memo: Document example workshop_01_kepler_real_lenses.rs - ([f3a679f](https://github.com/opossum-labs/opossum/commit/f3a679fa21f2d61ede275ee93b985e4aa287bf8c))

- :memo: #1005 Documentation: workshop_03_kepler_real_lenses_wavefront.rs - ([91722ed](https://github.com/opossum-labs/opossum/commit/91722ed453c4e35dc915c2ee4dc289c30615d456))

- :memo: #1007 Document example workshop_04_kepler_real_lenses_imaging_point.rs - ([b4fed01](https://github.com/opossum-labs/opossum/commit/b4fed01d09ebc053230b85008df27865ae20a724))

- :memo: #1009#1007 Document example workshop_04_kepler_real_lenses_im…#1008 - ([6b654e9](https://github.com/opossum-labs/opossum/commit/6b654e91909137079c0cf26f965156dd3526e6fe))

- :memo: #1009 Documentation: workshop_05_kepler_real_lenses_imaging_field.rs - ([ff2ce98](https://github.com/opossum-labs/opossum/commit/ff2ce98dd2dc61b06a2fbf12d46c2a08142f0d5e))

- :memo: #1011:DOC .workshop_06_geometry_mirrors.rs - ([9513173](https://github.com/opossum-labs/opossum/commit/9513173b2af63d6352109dcc3111273d9d534585))

- :memo: #1013workshop_07_geometry_shifted_lens.rs - ([399182c](https://github.com/opossum-labs/opossum/commit/399182c4596828fae5dd28d86b04f7b71891c054))

- :memo: #1013_workshop_07_geometry_shifted_lens.rs - ([73727f9](https://github.com/opossum-labs/opossum/commit/73727f958dfccb07e5e87710bf686aa54e7cc36b))

- :memo: #1015_workshop_08_reference_node.rs - ([cc1630a](https://github.com/opossum-labs/opossum/commit/cc1630a1df7c4ab266130306273ffe5d56db7d18))

- :memo: #1017_workshop_09_phelix.rs - ([6dbdea8](https://github.com/opossum-labs/opossum/commit/6dbdea8e455221f474633efa6f415144d4584471))

- :memo: Documentation : workshop_10_multi_path.rs - ([8c5fd86](https://github.com/opossum-labs/opossum/commit/8c5fd8674dc618e3ca9cbffced56089bbdb315ba))

- :memo: Update documentation of backend API data types. - ([d82a6f0](https://github.com/opossum-labs/opossum/commit/d82a6f05dc277c59e4dc5b0abcd3c28f2bfd62d6))

- :memo: Documentation and a brief description of the available position distributions. - ([d57b57b](https://github.com/opossum-labs/opossum/commit/d57b57bff70a626e5e1700261aa17cf7fc732dff))

- :memo: #1022 Documentation and a brief description of the available position distributions. - ([6e114fc](https://github.com/opossum-labs/opossum/commit/6e114fc020c15fda1a71f27d1bd350bc18dd80c0))

- :memo: Docuentation : Source port.md - ([84d9d3d](https://github.com/opossum-labs/opossum/commit/84d9d3d6bf7f16b3af1bd8b5577fa2042bcb1adf))

- Documentation : Wavefront Monitor.md - ([1857ee8](https://github.com/opossum-labs/opossum/commit/1857ee8840d7a05cce3ed0a7cd9b39ec21dae657))

- :memo: Update handbook. - ([d06fd64](https://github.com/opossum-labs/opossum/commit/d06fd64d1a8782ed557a521a5d3a64a864b9bb28))

- :memo: Documentation:source_port - ([67744bc](https://github.com/opossum-labs/opossum/commit/67744bc3fa1d616a4c1041a0b20c6eb04b7a3043))

- :memo: Documentation: Saving_options.md - ([b0ba981](https://github.com/opossum-labs/opossum/commit/b0ba981278cde3a0e1e7e2d20991db2e635b5f95))

- :memo: Documentation : Generating the Handbook - ([99e1f0a](https://github.com/opossum-labs/opossum/commit/99e1f0a5a0bc274d99505a405cbd9a5042ffc9d5))

- Documentation : Groups - ([279227d](https://github.com/opossum-labs/opossum/commit/279227d7e6297af53a61b11497e41f8e2967b2d6))

- documentation: saving_options - ([9d5a595](https://github.com/opossum-labs/opossum/commit/9d5a5952c7913c33727484a196a32d4253ceece9))

- :memo: Doc update - ([1dd31d7](https://github.com/opossum-labs/opossum/commit/1dd31d7c6013388caea0ba5499d52e9ce7d36fe5))

- :memo: Docs: Analyzers (updated ) - ([cf8eace](https://github.com/opossum-labs/opossum/commit/cf8eaced503a6e9943b123d851ce5184d4515d91))

- :memo: docs: energy_distribution - ([2ab266e](https://github.com/opossum-labs/opossum/commit/2ab266ea520f6f8b6026904a38069ae7484928ca))

- :memo: Docs: Analyzers (updated) - ([5cd14d9](https://github.com/opossum-labs/opossum/commit/5cd14d9eddb142b365c4326acc8934ee3f0e62ac))

- :memo: Docs: spectral_distribution - ([a49f793](https://github.com/opossum-labs/opossum/commit/a49f793c0590dfe99ddb9f3bd6864890cb4c83a9))


### <!-- 5 -->Testing

- :white_check_mark: Add some unit tests for apertures. - ([86af1fa](https://github.com/opossum-labs/opossum/commit/86af1faa161453f0ea3c635fc5fad7ea47c3140c))

- :test_tube: Add basic gui testing using playwright - ([8d69101](https://github.com/opossum-labs/opossum/commit/8d691017bc611d04b29545c40c24927e15106af2))


### <!-- 6 -->Miscellaneous Tasks

- :arrow_up: Update dependencies. - ([660f320](https://github.com/opossum-labs/opossum/commit/660f32047e88d029a2ae97fe3ada15a56b4fbf6f))

- :arrow_up: Update dependencies with less strict version requirements. - ([bbf25ec](https://github.com/opossum-labs/opossum/commit/bbf25ec2ad3677a006c4a8c9f00b4bb99b316bf4))

- :bookmark: Bump version number to 0.7.2 - ([c92698a](https://github.com/opossum-labs/opossum/commit/c92698ac1510059d02ff23b4996f24a242bddb12))


### ConvertToGroup

- Add guard if given node selection is empty. - ([3fcd1e7](https://github.com/opossum-labs/opossum/commit/3fcd1e72f5c052aa9ed1e777b90840cf4861a1de))


### Build

- :rocket: Fix application icon for debian pacakges - ([db48bba](https://github.com/opossum-labs/opossum/commit/db48bba04c92d8bc5ce8f3346e22dba0992e1c12))


## [0.7.1] - 2026-03-16

### <!-- 0 -->Features

- :sparkles: node and node_recursive functions now also return group itself if id matches - ([663de18](https://github.com/opossum-labs/opossum/commit/663de1819f6515c5d0a4f6155d74334b021e372b))

- :sparkles: Wavefront monitor now considers beam distortion - ([61d6c98](https://github.com/opossum-labs/opossum/commit/61d6c98bb7427e7a854f09f84011d906a44a384a))

- :sparkles: Add function to find all source port nodes in a scenery. - ([6b35f2f](https://github.com/opossum-labs/opossum/commit/6b35f2f39e7214934359e798b9f3c2ffff0823c5))

- :sparkles: SpotDiagram: show warning in report if used in EnergyAnalysis. - ([af6f2da](https://github.com/opossum-labs/opossum/commit/af6f2dac06cfe8bfee941d774137030cdec27553))


### <!-- 1 -->Bug Fixes

- :bug: delete_node now returns all node ids - ([7ef1a92](https://github.com/opossum-labs/opossum/commit/7ef1a9288f49f1b403e54e532c5cd6381d007d8e))

- :bug: rewrote usages of node recursive - ([b281845](https://github.com/opossum-labs/opossum/commit/b281845ddf427e90461fe299034e374ffdc1fa70))

- :bug: Roll/Pitch/Yaw notation corrected - ([2b7f34f](https://github.com/opossum-labs/opossum/commit/2b7f34fd0dab01c6de8db38e5dee1aca42bccb86))

- :bug: Fixed to consistently apply changes in CurvatureEditor - ([1c08c95](https://github.com/opossum-labs/opossum/commit/1c08c9513cb68cdbaa0705e2fb4a7cc6be0cdf9a))

- :bug: dropdown scrollbar is now always visible - ([51fc9a4](https://github.com/opossum-labs/opossum/commit/51fc9a4ef8ac53563cc513cc304df70eada56efe))

- :bug: Images displayed again in report - ([6d77c8d](https://github.com/opossum-labs/opossum/commit/6d77c8d7ddf221426c0410a0b58e32545cb30487))

- :bug: DIrectly nested portmaps are now displayed correctly in dot - ([da934c0](https://github.com/opossum-labs/opossum/commit/da934c092e123bb3eae2ff492cdaa833feae77af))

- :bug: Inverting groups now work as intended again - ([6df41c1](https://github.com/opossum-labs/opossum/commit/6df41c1ca1992092b86ce64eb795129f6bb88061))

- :bug: Fixed helper function point_ray_source - ([020e5b0](https://github.com/opossum-labs/opossum/commit/020e5b0f4ff80cbe4f0827db397d77088ec8926e))

- :bug: fixed point_Ray_source helper function - ([9011da9](https://github.com/opossum-labs/opossum/commit/9011da9c371a4c384b58a75d1c173211e90cbb32))

- :bug: Reference-node names change when original node name changes - ([3df01a1](https://github.com/opossum-labs/opossum/commit/3df01a181baa2d1c318d61852693f85b343a51ac))

- :bug: Beam splitter ports in dot are positioned correct again - ([e3f4b46](https://github.com/opossum-labs/opossum/commit/e3f4b4660f1756e9fcfd43f81744ed16f749db53))

- :bug: Beam combiner merges spectra correctly again - ([e7b9e74](https://github.com/opossum-labs/opossum/commit/e7b9e74cb3c93db4177968d65227a5ba6826aa31))

- :bug: Laser lines are now correctly displayed when combined - ([073db18](https://github.com/opossum-labs/opossum/commit/073db1841e700d268eb9499ce73230ab8a9be9f0))

- :bug: ray tracing plot after ghost focus shpwing again - ([502741d](https://github.com/opossum-labs/opossum/commit/502741d615601d4cdef47a23b4f54a0f88d3c84f))

- :bug: Fix smooth spectrum curve. - ([c0fb911](https://github.com/opossum-labs/opossum/commit/c0fb911e6b6838c52b6840248c3264af5a564291))


### <!-- 2 -->Refactor

- :recycle: Model_modified_signal is now completely handled via EventHandlers - ([854f80d](https://github.com/opossum-labs/opossum/commit/854f80d89be7c0dbd03ad3535e54e14001221b70))

- :art: A change of model_file_path is now propagated with EventHandlers - ([d267d3c](https://github.com/opossum-labs/opossum/commit/d267d3c98e32ad58722f98ce3005f15a2290a97b))

- :art: Context Menu signal now changed via EventHandlers - ([b83def3](https://github.com/opossum-labs/opossum/commit/b83def38c088bed5e027e77f717483327af85b95))

- :art: Connection changes from GUI now pass the group id - ([0225aa4](https://github.com/opossum-labs/opossum/commit/0225aa44212652bcd181f310f1fb4ced4a3475a2))

- :recycle: Heavily refactor Analysis trait code. - ([c946ef0](https://github.com/opossum-labs/opossum/commit/c946ef00fc9d660fb1a7268b6ea96fe500e9163c))

- :recycle: Remove no longer needed OpticNode::get_lightdata_mut trait function. - ([bdc9554](https://github.com/opossum-labs/opossum/commit/bdc95543cdf1134efdae99ba84dbad7537878474))

- :recycle: Replace all source helper functions by corrspeonding builder functions. - ([22b994e](https://github.com/opossum-labs/opossum/commit/22b994e5cefdb69925cc6c981bcfb84daa997ead))

- :recycle: Make analyzer configs source mappings consistent - ([999dc47](https://github.com/opossum-labs/opossum/commit/999dc475b76b0d5dcbda4653c5c704873dc30934))

- :recycle: Drastically reducing opm file size in fluence_test example - ([0c071f5](https://github.com/opossum-labs/opossum/commit/0c071f591f2ef4347c211d12d96dfc6195695b2f))

- :recycle: Improve structure of Ray <-> Rays::refract_onsurface - ([2be2bf7](https://github.com/opossum-labs/opossum/commit/2be2bf7e11553ca426e4735db80fa850a1a081aa))

- :recycle: Heavily refactor IdealFilter node and reorganize filter definitions. - ([1ea16e3](https://github.com/opossum-labs/opossum/commit/1ea16e304d8e8cfd1554fc30318af3033cf1eaf0))

- :recycle: Remove unnecessary functions in Lens and Cylindric Lens. - ([3e29a61](https://github.com/opossum-labs/opossum/commit/3e29a6197ac407b2ca50aeac4a14862722f7eaeb))

- :recycle: Heavily refactored Analysis::ghostfocus.rs - ([2bb07fb](https://github.com/opossum-labs/opossum/commit/2bb07fb192738aabb12cde868fb7fda03c32f085))


### <!-- 4 -->Documentation

- :memo: Add download badge to README.md - ([b354e79](https://github.com/opossum-labs/opossum/commit/b354e794c7a4ce7b1e7ef4d29de72c5f7736aa40))

- :memo: Fix broken screenshot link in README.md - ([d7c24ac](https://github.com/opossum-labs/opossum/commit/d7c24ac46daf7a338dd58e4d1114f8689b1fbcd2))

- :memo: Update all core library examples to use SourcePort now. - ([77c7d5f](https://github.com/opossum-labs/opossum/commit/77c7d5fb63f8b5ad18a15aba22960bca059dc6c6))


### <!-- 5 -->Testing

- :white_check_mark: Add unit test for checking correct handling of ray position history. - ([c85b4b4](https://github.com/opossum-labs/opossum/commit/c85b4b4cc212a09d2882fad8256e837cb89439a0))


### <!-- 6 -->Miscellaneous Tasks

- :bookmark: bump version 0.7.1 - ([7f8ff14](https://github.com/opossum-labs/opossum/commit/7f8ff1439e5ecbeae0413454b32e64ab75eb4d9a))


### NodeGroup

- :node() now rtruns the group itself if it matches the searched for uuid - ([7efd9a8](https://github.com/opossum-labs/opossum/commit/7efd9a81c78c791e62b9117678ff58efa7548d1f))


### Styling

- :lipstick: Improve layout of automatic changelog generation - ([2af01c0](https://github.com/opossum-labs/opossum/commit/2af01c0116b45e7e9d8d53cd43376bd3b4cfb8e5))


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
