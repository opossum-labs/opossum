# Reference node

![Reference node icon](../images/icons/node_unknown.svg)

## Key Usage Considerations

- **Placement restriction**: A reference to a group node must not be placed inside that same group, or inside any group nested within it, at any depth. Doing so would make the group depend on its own reference, which is rejected when creating, moving, cutting, or pasting a reference node. hence, this operation is prohibited.

## Analysis

## Ports

The ports of a reference node correspond to the ports of the node it refers to.

## Properties

This node type has no specific properties itself. All properties of this node correspond to the original node it refers to.
