import { test, expect } from '@playwright/test';
import { dragElementByOffset, connectElements, addNode } from './helpers/editor_actions';

test.describe('Opossum checklist: 2. Connections', () => {

  test.beforeEach(async ({ page }) => {
    // Open the editor before every test
    await page.goto('/');
  });

  test('drag from an output port to an input port, a connection is created', async ({ page }) => {
    // Helper to count all connections on the screen
    const edges = page.locator('[data-testid^="edge-"]');

    // Add two nodes
    const node_0 = await addNode(page, 'Dummy', 0);
    const node_1 = await addNode(page, 'Dummy', 1);

    // Move the first node away, so the two nodes do not sit on top of each other
    await dragElementByOffset(page, node_0, -300, 0);

    // No connection yet
    await expect(edges).toHaveCount(0);

    // Drag a line from the output port of node 0 to the input port of node 1
    const sourcePort = node_0.getByTestId('port-output-output_1');
    const targetPort = node_1.getByTestId('port-input-input_1');
    await connectElements(page, sourcePort, targetPort);

    // The connection should be there now
    await expect(page.getByTestId('edge-0')).toBeVisible();
    await expect(edges).toHaveCount(1);
  });

  test('connect two nodes, undo removes the connection and redo restores it', async ({ page }) => {
    // Helpers to count all nodes and all connections on the screen
    const nodes = page.locator('[data-testid^="node-"]');
    const edges = page.locator('[data-testid^="edge-"]');

    // Add two nodes
    const node_0 = await addNode(page, 'Dummy', 0);
    const node_1 = await addNode(page, 'Dummy', 1);

    // Move the first node away, so the two nodes do not sit on top of each other
    await dragElementByOffset(page, node_0, -300, 0);

    // Connect the two nodes with a line
    const sourcePort = node_0.getByTestId('port-output-output_1');
    const targetPort = node_1.getByTestId('port-input-input_1');
    await connectElements(page, sourcePort, targetPort);
    await expect(edges).toHaveCount(1);

    // Click a node so the editor listens to the keyboard
    await node_0.click();

    // Press Ctrl+Z: the line should go away, both nodes stay
    await page.keyboard.press('Control+z');
    await expect(edges).toHaveCount(0);
    await expect(nodes).toHaveCount(2);

    // Press Ctrl+Y: the line should come back
    // (we count it, because a restored line may get a new name)
    await page.keyboard.press('Control+y');
    await expect(edges).toHaveCount(1);
    await expect(nodes).toHaveCount(2);
  });

  test('delete a connection, undo restores it', async ({ page }) => {
    // Helpers to count all nodes and all connections on the screen
    const nodes = page.locator('[data-testid^="node-"]');
    const edges = page.locator('[data-testid^="edge-"]');

    // Add two nodes
    const node_0 = await addNode(page, 'Dummy', 0);
    const node_1 = await addNode(page, 'Dummy', 1);

    // Move the first node away, so the two nodes do not sit on top of each other
    await dragElementByOffset(page, node_0, -300, 0);

    // Connect the two nodes with a line
    const sourcePort = node_0.getByTestId('port-output-output_1');
    const targetPort = node_1.getByTestId('port-input-input_1');
    await connectElements(page, sourcePort, targetPort);

    const createdEdge = page.getByTestId('edge-0');
    await expect(createdEdge).toBeVisible();

    // Activate the line. We use focus here, since click() would activate the text field on the line
    await createdEdge.focus();

    // Press DELETE: the line should disappear
    await page.keyboard.press('Delete');
    await expect(createdEdge).not.toBeVisible();
    await expect(edges).toHaveCount(0);

    // Click a node so the editor listens to the keyboard again
    await node_0.click();

    // Press Ctrl+Z: the line should come back, both nodes stay
    await page.keyboard.press('Control+z');
    await expect(edges).toHaveCount(1);
    await expect(nodes).toHaveCount(2);
  });
});