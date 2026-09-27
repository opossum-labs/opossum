# 3D View

The **3D View** shows the optical components of the current model as solid geometry, in the place the
model puts them. It is a picture of the setup, not an analysis: nothing is computed from what it
shows, and it is meant for judging at a glance whether a setup looks the way it was intended —
whether a lens sits where it should, whether two components collide, whether a fold goes the way
round you had in mind.

## Opening and closing it

Open it from the top navigation bar: **Layout** → **3D View**. It appears as a further tab beside the
graph tabs and comes to the front straight away.

The same menu entry then reads **Close 3D View**, and the tab carries the usual close button. Closing
it frees the graphics resources it holds, so leave it closed when you are not looking at it.

The entry needs a connected backend: the view asks the backend for the model as soon as it opens.

## What is drawn

Every component that **encloses a volume** — lenses, wedges, cylindric lenses — and every component
that is **one optical surface** — mirrors (flat and parabolic), gratings, beam splitters, filters,
paraxial surfaces, and the detectors (energy meter, fluence detector, spot diagram, wavefront
monitor, spectrometer). Components with no shape of their own are still left out: a source port is
where light begins rather than a thing, and a reference node is the same component passed through a
second time rather than a second component. The count in the toolbar tells you how many are drawn.

A component only knows where it is once the optical axis has been traced, so the model needs an
analyzer that places its nodes — a ray trace. Without one there is nothing to place, and the view
stays empty. Components the trace does not reach are reported in the log and left out rather than
stopping the rest of the setup from being drawn.

If the optical axis ends *at* a component that still has others after it, those cannot be placed,
and the view cannot be drawn at all. The log says where the axis ended. The usual cause is a
**grating that is not tilted**: its diffraction order then does not propagate at the source's
wavelength, and the message names the Littrow angle to tilt the grating by. A grating with nothing
after it is drawn either way.

## Light: the optical axis and the rays

Two kinds of light can be drawn over the components, each switched on and off in the toolbar.

**The optical axis** (**Beam axis**, on by default) is the path the positioning run follows: one ray
per source, the one every component is placed on. It is drawn as a purple line from each source
through the components to wherever the axis leaves the setup, and shows how the setup is built up —
why a component sits where it does, which way a fold turns. A model without an analyzer is placed
from default rays, and its axis is drawn from those.

**The rays** (**Rays**, off by default) are the light of the sources an analyzer defines, traced
through the setup like a ray trace does. They show what the axis alone cannot: how a bundle widens
and focuses, where an aperture clips it, where a beam splitter divides it. Each ray is coloured by
its wavelength; infrared light, which has no colour of its own, is drawn red, and ultraviolet light
violet. A ray stopped by an aperture ends where it is stopped.

A few things to know about the rays:

- **They are a picture, not the analysis.** A source may hold far more rays than can be drawn, so
  each source is thinned first to at most **max. rays** (500 by default, at most 5000), spread evenly
  over the bundle's cross section with its outermost rays kept, so the drawn bundle has the true
  width. A source with fewer rays is drawn in full. The energies of the rays play no part in the
  picture.
- **Only the primary path is drawn.** A ghost focus analyzer contributes its sources, but not its
  reflections.
- **They need an analyzer with sources.** Without one there is no light of the setup's own to draw,
  and the Rays button shows nothing. A model analyzed in several ways cannot be drawn at all, since
  each analyzer may place it differently.
- **They cost a trace.** While Rays is on, every change to the model traces the sources again. Turn
  it off when you do not need it; while it is off nothing is traced.
- Light that ends in a component without an output is drawn up to the component before it, not into
  it.

**opacity** sets how opaque the axis and the rays are drawn, from barely visible to solid. It only
changes the picture; nothing is traced again. Dense bundles read better when they are partly
transparent.

Lines keep their width on screen however far you zoom in or out.

## Navigating

| Action | Effect |
| :--- | :--- |
| Drag with the left mouse button | Orbit around the point the camera looks at |
| Mouse wheel | Zoom in and out |
| Drag with the right mouse button | Pan |
| Click a component | Select it; a box is drawn around it |
| Click empty space | Clear the selection |
| Click a gizmo axis | Snap the camera to look straight along that axis |

Dragging never selects, so orbiting past a component does not pick it up by accident. The optical
table is not selectable: clicking it clears the selection, the same as clicking empty space. Neither
are the axis and the rays: a click on a line selects the component behind it.

## The toolbar

| Button | Effect |
| :--- | :--- |
| **Refresh** | Ask the backend for the model again, at once |
| **Fit view** | Frame all components (the optical table is not included) |
| **Reset camera** | Return the camera to its starting pose |
| **Table** | Show or hide the optical table |
| **Axes** | Show or hide the corner orientation gizmo |
| **Beam axis** | Show or hide the optical axis |
| **Rays** | Show or hide the traced rays of the analyzer's sources |
| **max. rays** | The most rays drawn per source; takes effect on Enter or when leaving the field |
| **opacity** | How opaque the axis and the rays are drawn |

See [Light: the optical axis and the rays](#light-the-optical-axis-and-the-rays) for what the last
four show.

Refresh, Fit view, and Reset camera move the camera, and so does clicking a gizmo axis in the
viewport. Table, Axes, Beam axis and Rays are visibility toggles and do not move the camera. Nothing that happens to
the *model* moves it either — see below.

## When the view updates itself

The view follows the model on its own. Adding or deleting a component, changing a distance, changing
a lens radius or thickness, grouping, pasting, undo and redo all reach it within a moment; a burst of
edits is collected so that holding a spinner costs one update rather than dozens. The optical axis
and, while shown, the rays follow along with the components.

Two things are worth knowing about how it updates:

- **The camera never moves by itself.** Whatever changes in the model, the view you set up stays.
  Only the toolbar buttons and clicking a gizmo axis move the camera.
- **Moving nodes around the graph canvas changes nothing here**, and neither does Auto Layout. Where a
  node sits on the diagram has nothing to do with where its component sits in space.

Switching to a graph tab and back leaves the camera exactly where you left it. Closing the view and
opening it again does not: that builds it up afresh.

## Appearance

Glass is drawn as glass — refracting its surroundings rather than painted on — using the refractive
index the component's material reports.

That index is always taken at **1053 nm**, the design wavelength of the laser systems OPOSSUM is
built around. A drawing has to pick some colour of light to render glass at, and this one does not
follow the analyzer or the default wavelength in the settings. A material that cannot state an index
at 1053 nm is drawn as plain glass, and the substitution is noted in the log.

A mirror and a beam splitter are drawn reflective; a grating is drawn with an iridescent,
diffraction-coloured sheen that marks it at a glance as the component that spreads light by wavelength. A filter or a paraxial surface is
drawn as a faint, translucent plane. A detector is drawn as a plain, matte surface, deliberately
distinct from glass and from a mirror, so a setup reads at a glance which components measure the
light rather than shape or redirect it.

Unlike a lens or a wedge, these components have no second surface of their own to keep facing the
camera, so each is drawn from both sides at once and stays visible however far you orbit around it.

Every component is drawn slightly see-through, so a component sitting behind another optic stays
visible rather than being hidden by it. This is deliberate: glass shows only solid surroundings
through it — another lens in front would otherwise blot out whatever is behind — so drawing the
components a little transparent lets them show through one another and keeps the whole setup
readable from any angle.

## The optical table and orientation

Beneath the setup sits an **optical table**: a dark breadboard surface with a regular raster of holes
spaced 2.5 cm apart, like a real optical bench. It lies **7.5 cm below the beam** — the optical axis
runs through the components' centres — so it gives the scene a floor and an immediate sense of scale
without cutting through the optics. It replaces the old generic reference grid, which would swamp a
centimetre-to-metre setup rather than frame it.

The table has no edge: it reaches to the horizon wherever you move the camera and fades into the
background in the distance. It is scenery, not part of the model — **Fit view** frames only the
components, and clicking the table clears the selection. Seen through a lens, the table does not
show. Show or hide it with the **Table** button in the toolbar.

In the bottom-right corner of the view a small **orientation gizmo** shows which way x, y, and z
point. It keeps you from losing your bearings while orbiting. Click one of its axes to snap the camera
to look straight along that direction; show or hide it with the **Axes** button in the toolbar.
