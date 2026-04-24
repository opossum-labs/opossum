# Usage

The following section describes how to use the OPOSSUM suite via the graphical user interface (GUI).

## Starting the GUI

We assume that the software has already been installed using the appropriate installer for your platform.

### Windows

On Windows, both the MSI and EXE installers create an entry in the Start Menu and a desktop icon named `Opossum_GUI`.

### Linux

On Linux, the launch method depends on the installation package type. Debian and Redhat packages (`.deb` & `.rpm`) install an icon in the system menu. Alternatively, the OPOSSUM GUI can be launched directly from the command line:

```bash
opossum_gui
```

For the AppImage package, you can execute the file directly by double-clicking it in your file manager or running it via the command line. You may need to mark the file as executable first:

```bash
cd <path to AppImage>
chmod u+x OpossumGui-0.7.0.AppImage
./OpossumGui-0.7.0.AppImage
```

Once started, the main window will appear:

![opossum_gui empty](../images/opossum_gui_empty.PNG)

## First Steps

The top menu bar contains standard file handling options, while the window controls are located on the right. We recommend maximizing the window to ensure sufficient space on the canvas for modeling your optical system.

In this tutorial, we will model a simple Kepler telescope consisting of a source and two convex lenses.

### Adding an Optical Node and Using the Canvas

Use the `Edit` menu to add either an optical node or an analyzer node. Let's start by selecting an optical node: a `Source` node, which represents the starting point of our optical system.

Once selected, the source node appears on the canvas. You can move the node by simply clicking and dragging it. Next, add two `Lens` nodes and place them on the canvas as well.

![first step](../images/opossum_gui_first_steps_1.PNG)

The canvas itself is virtually infinite. You can pan the viewport by clicking and dragging on the background, and zoom in or out using the mouse wheel.

![second step](../images/opossum_gui_first_steps_2.PNG)

### Connecting Nodes

To form an optical network, the nodes must be connected. Click on an output port (the light green square on the right side of a node) and drag a line to the input port of another node. Connect them in this order: `Source -> Lens -> Lens`. Your screen should look similar to this:

![third step](../images/opossum_gui_first_steps_3.PNG)
Each node port can only be connected to exactly one other port, i.e., a single source cannot be connected to multiple inputs simultaneously. 
For example, if a light source or signal needs to be split into multiple paths, a dedicated beamsplitter node must be used.

Each connection displays a numeric value representing the spatial separation between the connected nodes (the distance along the optical axis). For a detailed discussion on element placement, please refer to the [Concepts](../concepts/concepts.md) section. For this example, set the distance between the source and the first lens to **10 mm**, and the distance between the two lenses to **200 mm**.

Numeric values can be entered manually, with units, and unit prefixes are supported. For example, distances can be specified from millimeters (mm) to kilometers (km), including metric system prefixes from quecto (q) to quetta (Q). The system automatically reads these prefixes and converts the values into a standard internal unit.

You can easily delete a node by selecting it and pressing the 'Delete' key on the keyboard if it was created by mistake or is no longer needed.

You can also tidy up the node arrangement using the auto-layout function: Select `Auto Layout` from the `Layout` menu or press `Ctrl + Shift + A`. The result should look like this:

![fourth step](../images/opossum_gui_first_steps_4.PNG)


### Configuring Nodes

Now that the optical system structure is set, we need to define the parameters for each component. For this tutorial, we will configure only the essential parameters.

Start with the first lens. Click on the node to highlight it. Its properties are displayed in the `Node Editor` panel on the left. Focus on the `Properties` section, which shows the center thickness and the radii of curvature.
Set the **Center Thickness** to **3 mm**, the **Front Radius** to **60 mm**, and check the box for a **Plane Back Surface**. Leave the refractive index at **1.5**. This configures the component as a convex-plano lens.

![fifth step](../images/opossum_gui_first_steps_5.PNG)

Repeat this step for the second lens: Set the **Center Thickness** to **3 mm**, the **Front Surface** to **Flat** (Plane), and the **Back Surface** to **-40 mm**.

Finally, select the `Source` node to configure the spectral properties. Navigate to `Properties -> Light definition -> Spectral distribution`. Change the `Rays spectral distribution` from "Gaussian" to "Laser Lines". This generates a monochromatic source (defaulting to 1054 nm) instead of a broad spectrum.

### Adding an Analyzer

Although the optical setup is complete, we must define the analysis method. Select `Add Analyzer` from the `Edit` menu and choose **RayTracing**. As implied, this node performs the ray tracing calculations. We will keep the default settings for this tutorial.

![sixth step](../images/opossum_gui_first_steps_6.PNG)

### Adding Detector Nodes

Technically, we could start the simulation now, but we would see almost no output because we haven't specified any *Detector Nodes*.

We want to visualize the spot diagram at the end of the system and see the beam propagation path. Add the **Spot Diagram** and **Ray Propagation** detector nodes. Connect them to the end of the lens system.

Detector nodes are typically "transparent"—they have no thickness and do not alter the light passing through them. This allows you to chain multiple detectors with zero distance between them to monitor different aspects of the beam at the same location.
For now, connect the detectors and set a distance of **50 mm** between the last lens and the spot diagram, so we can see the light propagate slightly beyond the second lens.

![seventh step](../images/opossum_gui_first_steps_7.PNG)


### Performing the Simulation

We are now ready to run the simulation. First, save your model by selecting `File -> Save` (saved as an `.opm` file).

Start the simulation by clicking the green **Simulate** button at the top. On the first run, you will be asked to select a report directory where the output files will be stored.

**Note:** Use a dedicated directory that does not contain other important files (like your model file), as the contents may be overwritten or deleted between simulation runs.

After selecting the directory, the simulation will start in a separate window.

![eighth step](../images/opossum_gui_first_steps_8.PNG)

Once the simulation finishes, you can close the simulation window.

### Viewing the Report

The analysis data has been written to your selected report directory. OPOSSUM generates an HTML report (e.g., `<report dir>/report_0.html`) which you can open in any web browser. It should look similar to this:

![ninth step](../images/opossum_gui_first_steps_9.PNG)

## Further Reading

You have now built a simple (albeit not perfectly collimated) Kepler telescope.

* Check the [Reference](../reference/reference.md) documentation for details on all available optical components, detectors, and analyzers.
* Specific tasks are described in the [How-to Guides](../howto%20guides/howto%20guide.md).
* For general background information, consult the [Concepts](../concepts/concepts.md) section.
