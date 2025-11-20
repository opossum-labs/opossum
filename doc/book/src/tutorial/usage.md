# Usage

The following section describes the usage of the OPOSSUM suite through the graphical user interface (GUI).

## Starting the GUI

In the following we will assume that the software has been installed through one ot the onstallers for the specific platform.

### Windows

On the windows platform, both the MSI and the EXE installers provide an entry in the start menu and a desktop icon `Opossum_GUI`.

### Linux

For Linux it depends on the type of the installation package. Debian and Redhat packages (`.deb` & `.rpm`) install an icon in the menu system. Furthermore, the OPOSSUM GUI
can be started directly on the command line:

```bash
opossum_gui
```

For the AppImage package you can directly execute the package itself by double clicking on the file in the file manager or through the command line e.g.:

```bash
OpossumGui-0.7.0.AppImage
```

Starting the GUI leads to this window to appear:

![opossum_gui empty](../images/opossum_gui_empty.PNG)

## First steps

The top menu bar shows the typical menu entries for file handling and the window buttons on the right side. We recommended to use the GUI with a maximized window in order
to have enough space on the canvas to model your optical system.

### Adding an optical node and using the canvas

Using the `Edit` menu we can add either an optic node or an analyzer node. Let us first select an optical node. We start with a `Source` node representing the start of our
optical system.

After selecting, the source node appears on the canvas. The node can be moved on the canvas by simply clicking and dragging it. Let's add two additional `Lens` nodes and place them on the canvas.

The canvas istself is (almost) infinitely large. The view port can be moved by clicking and dragging on the canvs itself. Furthmore, the view port can be zoomed in an out using the mouse wheel.
