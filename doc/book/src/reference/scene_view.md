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

Every component that **encloses a volume** — lenses, wedges, cylindric lenses. Components with no
shape of their own are left out: a detector or an energy meter measures light without occupying
space, a source port is where light begins rather than a thing, and a reference node is the same
component passed through a second time rather than a second component. The count in the toolbar tells
you how many are drawn.

A component only knows where it is once the optical axis has been traced, so the model needs an
analyzer that places its nodes — a ray trace. Without one there is nothing to place, and the view
stays empty. Components the trace does not reach are reported in the log and left out rather than
stopping the rest of the setup from being drawn.

Rays are not drawn yet.

## Navigating

| Action | Effect |
| :--- | :--- |
| Drag with the left mouse button | Orbit around the point the camera looks at |
| Mouse wheel | Zoom in and out |
| Drag with the right mouse button | Pan |
| Click a component | Select it; a box is drawn around it |
| Click empty space | Clear the selection |

Dragging never selects, so orbiting past a component does not pick it up by accident.

## The toolbar

| Button | Effect |
| :--- | :--- |
| **Refresh** | Ask the backend for the model again, at once |
| **Fit view** | Frame everything that is drawn |
| **Reset camera** | Return the camera to its starting pose |

These three are the only things that move the camera. Nothing that happens to the *model* does —
see below.

## When the view updates itself

The view follows the model on its own. Adding or deleting a component, changing a distance, changing
a lens radius or thickness, grouping, pasting, undo and redo all reach it within a moment; a burst of
edits is collected so that holding a spinner costs one update rather than dozens.

Two things are worth knowing about how it updates:

- **The camera never moves by itself.** Whatever changes in the model, the view you set up stays.
  Only the three toolbar buttons move the camera.
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

The reference grid is off: an optical setup is centimetres to metres across, and a grid sized for a
general-purpose 3D scene would swamp it rather than give it a floor.
