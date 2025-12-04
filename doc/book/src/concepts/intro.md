# Introduction

![OPOSSUM icon](../images/Logo_text.svg)

## What is OPOSSUM?

**OPOSSUM** is a powerful software framework designed to simulate **large-scale optical systems**.

Imagine your entire optical setup consisting of mirrors, lenses, filters, and prisms as a **network** or a **graph**. OPOSSUM uses this graph-based approach:

* **Nodes:** Each interconnected node in the graph represents a single **optical component**.
* **Analyzers:** On top of this model, we have specialized modules called **Analyzers**. These modules run specific types of simulations (e.g., raytracing, thermal analysis) on the exact same model.

This approach lets you **build your optical model once and analyze it often** in many different ways without having to rebuild the system every time you switch simulation types.

## Why OPOSSUM?

There are already many excellent, mostly commercial, tools available for optics simulation and calculation. So, why create another one?

Existing solutions are great, but they often struggle with the complexity of **modern, large-scale systems**:

### The Challenge of Complexity

When designing state-of-the-art laser facilities, you often end up with **hundreds of optical components**. The laser beam path is no longer a simple, single line; it involves:

* **Parallel Paths:** Beam splitters create multiple paths.
* **Main and Side Paths:** There's usually a "main" path for the high-energy beam and several "side" paths for diagnostics and monitoring.
* **Subsystems:** Facilities are broken down into distinct stages like a **frontend**, a **pre-amplifier**, a **main amplifier**, and a **target chamber**.

### The Problem of Fragmentation

Existing tools typically only address **specific aspects** of an optical system:

* Some are fantastic for pure **geometric optics (raytracing)**, but were originally built for smaller systems (like a single camera lens). Managing hundreds of optics or parallel beam lines in these tools can become a nightmare.
* Others focus entirely on **non-linear material effects**, **wavefront propagation**, **parasitic lasing**, or **straylight simulation**.

### The Need for Unification

In high-energy laser design, you must consider all these effects **at the same time**:

* **Beam path** (geometric ray or Fourier optics)
* **Temperature stability**
* **Ghost focus formation**
* **Thermal lensing effects**
* ...and much more.

Relying on fragmented tools means you constantly have to switch software and/or repeatedly remodel your system to simulate the next effect.

**OPOSSUM is an approach to unifying this entire design workflow.** It offers a consistent, comprehensive environment to model and analyze the full complexity of your optical system.
