# Beam propagation visualizer

![Beam propagation visualizer icon](../images/icons/node_propagation.svg)

The Beam Propagation Visualizer is a virtual detector that plots the path of all incoming light rays in a 2D projection. It is an essential tool for debugging and understanding ray behavior in an optical system, especially for ray tracing and ghost focus analysis.

Rays of different wavelengths are automatically plotted in different colors for easy identification.

## Key Usage Considerations

- **Placement**: This node only visualizes the ray history up to its own position. For a complete view of a system, you should typically place it at the very end of your optical network graph.

- **Multi-Path Systems**: When used in a system with components like beamsplitters, the visualizer will only plot the rays for the specific path it is on.

## Analysis

As a detector node, incoming light data is simply passed unmodified through the node. However, possible apodization due to input or output port apertures might occur.

- Energy Analysis

    In energy analysis, this node does not report anything.

- Ray tracing Analysis

    In ray tracing analysis, this node generates a 2D plot as described above.

- Ghost focus Analysis

    In ghost focus analysis, this node generates a 2D plot as described above.

## Ports

- `input_1`

    Input port

- `output_1`

    Light ouput. This port delivers a copy of the light data from the port `input_1`.

## Properties

- `view direction`

    The generated plot shows the 2D projection of the ray propagation. This parameter determines the projection plane defined by a normal vector. Default
    is the yz plane.

- `ray transparency`

    Sets the alpha (transparency) of the plotted rays. Useful for visualizing dense or overlapping beams. Must be in the interval [0.0,1.0]. The default value is 0.4.
