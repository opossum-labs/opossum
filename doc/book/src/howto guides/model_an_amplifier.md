# Model an amplifier

This guide turns an existing optical component into an amplifier and analyzes the model at two
operating points. It assumes a model that already contains a source, at least one lens, wedge or
cylindric lens, and an energy meter or another detector at the end.

The task splits in two: the component stays what it is, and a *pump scenario* says how hard it is
driven during a run. Both routes below produce the same thing — pick the one that fits how you work.
For what the settings mean, see [Pump scenarios](../reference/pump_scenarios.md).

## In the graphical user interface

**1. Mark the component as an amplifier.** Right-click the lens, wedge or cylindric lens on the
canvas and choose **As amplifier**. The entry only appears on components that enclose a volume of
material. **As passive optic** in the same menu takes the marking back.

If the document has no pump scenario yet, one is created automatically — a marked component with
nowhere to configure it would be a dead end.

**2. Open the scenario editor.** Click the amplifier icon in the narrow bar at the side of the
canvas to switch the sidebar to its **Pump scenarios** view. (Clicking the icon of the view that is
already open collapses the sidebar again.)

**3. Create the operating points you need.** Type a name into the **New scenario name** field and
press **Add**. Repeat for a second scenario, for instance `full power` and `half power`.

Clicking a scenario card makes it the *active* one. That choice only affects what the canvas shows;
it does not decide what is analyzed.

**4. Set the gain.** Expand a scenario card with the **▸ amplifiers** toggle. Every marked component
in the document gets a row there, whether or not it amplifies in this particular scenario. Per row:

- **Gain model** — choose `Const`.
- The **gain factor** field appears next to it. Enter the energy gain of that component in this
  scenario, e.g. `5` in `full power` and `2.5` in `half power`.

The factor is set per scenario, so editing one scenario never changes another. A component left at
`None` stays passive in that scenario.

**5. Tell the analysis which scenarios to run.** Select the analyzer node. In the node editor, tick
the scenarios under **Pump scenarios**. One report is produced per ticked scenario. Ticking nothing
runs the model once, passively.

**6. Simulate.** Press the green **Simulate** button as usual. The reports are named after the
scenario they belong to (`Energy Analysis - full power`), so they can be told apart directly.

On the canvas, a marked component shows `amp: <model>` in its footer — the gain model it runs with
*in the active scenario*. Switching the active scenario switches what is shown there.

## In code

A model built in Rust configures the same three things: the components, the scenarios, and which
scenarios the analyzer runs in. The complete, runnable program is
`opossum_core/examples/amplifier_chain.rs`; run it with

```bash
cargo run -p opossum_core --example amplifier_chain
```

and open the resulting `.opm` file in the GUI to see the very same set-up there. The parts that
matter here:

```rust
use opossum_core::{
    gain::{ConstGain, GainModel},
    prelude::*,
};

// The components are ordinary lenses - nothing about them says "amplifier".
let first_head = scenery.add_node(Lens::new(/* ... */)?)?;
let second_head = scenery.add_node(Lens::new(/* ... */)?)?;

let mut document = OpmDocument::new(scenery);

// Optional: makes the GUI show these nodes as amplifiers when the file is opened there.
// The analysis does not read it - the gain model below is what amplifies.
document.set_is_amplifier_node(first_head, true);
document.set_is_amplifier_node(second_head, true);

// One operating point per pump condition to be analyzed.
let scenario_id = document.add_pump_scenario("full power");
let scenario = document
    .pump_scenario_mut(scenario_id)
    .expect("the scenario just added must be there");
scenario.set_gain_model(first_head, GainModel::Const(ConstGain::new(5.0)?));
scenario.set_gain_model(second_head, GainModel::Const(ConstGain::new(4.0)?));

// Without this the model is analyzed once, passively.
document
    .analyzer_mut(analyzer_id)
    .expect("the analyzer just added must be there")
    .set_pump_scenarios(vec![scenario_id]);
```

`ConstGain::new` rejects a factor that is negative or not finite, so an invalid gain is refused where
it is set rather than halfway through an analysis.

## Checking the result

Read the energy at the detector at the end of the chain in each report and compare it with the
product of the factors you entered. The example above starts from 1 J and amplifies by 5 and then 4,
so the `full power` report has to read 20 J, and `half power` — 2.5 and 2 — has to read 5 J. If a
report reads the input energy unchanged, the analyzer was not given the scenario in step 5.
