# Analyzers

An analyzer is a module that "orchestrates" an optics simulation. An analyzer defines how the model will be treated. There are basically two types of analyzers: Sequential and Non-Sequential. A sequential analyzer traverses the graph in a defined manner and triggers the corresponding calculation defined inside the nodes. In contrast, the non-sequential analyzer does not actually make use of the graph structure (i.e. the relations between the nodes) but only uses the nodes and their corresponding attributes such as 3D coordinates or mechanical model data.

**Note**: The use of different analyzers might lead to ["contradicting" models](https://git.gsi.de/phelix/rust/opossum/-/issues/9). For example, one can model a free-space propagation node between two other elements (e.g. two lenses). It can thus define a given geometric length as an attribute. On the other hand, the two lenses might have 3D coordinates with a different distance to each other than defined in the propagation node. In this case, the sequential analysis would simulate another model situation as the non-sequential analyzer...

## Sequential Analyzers

A sequential analyzer uses the node relations defined by the edges of the model graph. It would traverse the graph starting from one or more sources to one or more sink nodes. While traversing, it calls the respective analysis functions of each node. The input data is taken from previously calculated light information stored in the input edges of a node. The node's analysis result will be stored on the output edges.

The analyzer type also determines the type of data "flowing" between the nodes. For the energy analyzer, the data mainly consists of a light spectrum, while the ray tracing analyzer uses bundles of geometric rays. These rays contain information such as position, direction, wavelength, and other ray properties. Furthermore, each analyzer can have a specific set of configuration parameters that influence the respective analysis algorithm.

The analyzer is also used to configure the properties of source ports.

Depending on the selected analyzer type, different source port configuration options are available.

Besides its own configuration, an analyzer carries a list of the [pump scenarios](pump_scenarios.md)
it is run in — the operating points at which the model's amplifying components are driven. It
produces one report per listed scenario; an empty list means a single run on the passive model.

The following analyzers are implemented:

- Energy Analysis

    This is the simplest analyzer. It just calculates the energy spectrum while passing through the optical system. Filter
    nodes attenuate the spectrum while beam splitter nodes divide the energy spectrum in two arms according to the splitting
    config. On the other hand, many other nodes such as lenses or gratings do not influence the data during the pass. Hence,
    this analyzer gives you rather limited information about an optical system. The advantage on the other side is that this
    analyzer is really fast. The energy analyzer has no further configuration parameters.

    The analyzer is also used to configure the properties of source ports. For the energy analyzer, the source port
    configuration provides the `Energy type`, `Wavelength`, `Energy`, and `Resolution` options.

    The `Energy type` provides two options: `Laser lines` and `From file`. 
    The default energy value is 1 Joule, and the default wavelength is 1.054 µm.

- Ray Tracing Analysis

    The ray tracing analyzer uses bundles of geometric rays. These rays contain information such as position, direction,
    wavelength, and other ray properties.

    Compared to the energy analyzer, the ray tracing analyzer provides more detailed information about the geometrical
    behavior of light propagation through the optical system, since individual rays are tracked during the analysis.

    The ray tracing analyzer provides configuration parameters including the maximum number of refractions, maximum number
    of bounces, minimum ray energy, and missed surface strategy. The maximum number of refractions defines the maximum number
    of refraction events considered during the analysis. The maximum number of bounces defines the maximum number of
    reflections considered during the analysis. The missed surface strategy defines how rays are handled when a surface is
    not reached. The available options are `Stop` and `Ignore`.

    The analyzer is also used to configure the properties of source ports. For the ray tracing analyzer, the source port
    configuration provides options for the `Ray type`, `Position distribution`, `Energy distribution`, and
    `Spectral distribution`. The available ray types are `Collimated`, `Point`, and `Image`.


- Ghost Focus Analysis

    The ghost focus analyzer can be seen as an extended ray tracing analyzer. In fact, the ghost focus analyzer with the `Max bounces`
    parameter set to zero is the basic ray tracing analysis presented above.

    The ghost focus analyzer is particularly important for the analysis of high-energy laser systems. In practical optical systems,
    optical surfaces are not ideal and can generate unintended reflections. Although these reflections may represent only a small
    fraction of the incident energy, they can become significant in systems operating with pulse energies in the kilojoule (kJ)
    range. For example, a reflection of only 1% corresponds to several joules of optical energy, which can affect other optical
    components and the overall optical system.

    The ghost focus analyzer provides the `Max bounces` configuration parameter. The `Max bounces` parameter specifies the maximum
    number of reflections that are considered during the analysis. A value of 1 considers rays that undergo a single reflection.
    A value of 2 considers rays that undergo two reflections, for example, one reflection at one optical surface followed by a
    second reflection at another optical surface. Likewise, values of 3 or 4 consider rays that undergo three or four reflections,
    respectively. Higher values therefore include additional reflection paths that may occur within the optical system. This is
    particularly important for high-energy laser systems, where even a small reflected fraction of the laser energy can correspond
    to several joules and may affect optical components and the overall system.If the `Max bounces` parameter is set to zero, no reflections are considered. However, this mode can still be useful because it allows analyzing the fluence of the primary beam (the main laser beam) on all optical surfaces and, for example, ensures that they all remain within safe limits.


    Additionally, it provides a `Fluence Estimator` option, which allows selecting the fluence estimator used for the analysis.
   For further details on the available fluence estimators and their differences,see the [Fluence Estimators](../concepts/fluence.md) section.