import { test, expect } from '@playwright/test';
import { dragElementByOffset, connectElements, addNode } from './helpers/editor_actions';

test.describe('Opossum checklist: 1. Nodes', () => {

  test.beforeEach(async ({ page }) => {
    // Open the editor before every test
    await page.goto('/');
  });

  test('add a node, undo removes it and redo restores it at the same position', async ({ page }) => {
    // Add one node
    const node_0 = await addNode(page, 'Dummy', 0);

    // Remember where the node is
    const startBox = await node_0.boundingBox();
    expect(startBox).not.toBeNull();

    // Click the node so the editor listens to the keyboard
    await node_0.click();

    // Press Ctrl+Z: the node should go away
    await page.keyboard.press('Control+z');
    await expect(node_0).not.toBeVisible();

    // Press Ctrl+Y: the node should come back in the same place
    await page.keyboard.press('Control+y');
    await expect(node_0).toBeVisible();
    await expect(async () => {
      const box = await node_0.boundingBox();
      expect(Math.abs(box!.x - startBox!.x)).toBeLessThan(5);
      expect(Math.abs(box!.y - startBox!.y)).toBeLessThan(5);
    }).toPass();
  });

  test('delete a connected node, undo restores node and connection, redo deletes again', async ({ page }) => {
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

    // Check that the line is there
    const createdEdge = page.getByTestId('edge-0');
    await expect(createdEdge).toBeVisible();

    // Click the second node and press DELETE
    await node_1.click();
    await page.keyboard.press('Delete');

    // The node and its line should both be gone
    await expect(node_1).not.toBeVisible();
    await expect(createdEdge).not.toBeVisible();

    // Press Ctrl+Z: the node and its line should both come back
    // (we count them, because a restored node may get a new name)
    await page.keyboard.press('Control+z');
    await expect(nodes).toHaveCount(2);
    await expect(edges).toHaveCount(1);

    // Press Ctrl+Y: the node and its line should be gone again
    await page.keyboard.press('Control+y');
    await expect(nodes).toHaveCount(1);
    await expect(edges).toHaveCount(0);
  });

  test('move a node, undo restores the position and redo moves it again', async ({ page }) => {
    // Add one node
    const node_0 = await addNode(page, 'Dummy', 0);

    // Remember where the node starts
    const startBox = await node_0.boundingBox();
    expect(startBox).not.toBeNull();

    // Drag the node to a new place and check that it really moved
    await dragElementByOffset(page, node_0, 200, 100);
    const movedBox = await node_0.boundingBox();
    expect(movedBox).not.toBeNull();
    expect(movedBox!.x).toBeGreaterThan(startBox!.x + 150);
    expect(movedBox!.y).toBeGreaterThan(startBox!.y + 50);

    // Press Ctrl+Z: the node should jump back to where it started
    await page.keyboard.press('Control+z');
    await expect(async () => {
      const box = await node_0.boundingBox();
      expect(Math.abs(box!.x - startBox!.x)).toBeLessThan(5);
      expect(Math.abs(box!.y - startBox!.y)).toBeLessThan(5);
    }).toPass();

    // Press Ctrl+Y: the node should go to the new place again
    await page.keyboard.press('Control+y');
    await expect(async () => {
      const box = await node_0.boundingBox();
      expect(Math.abs(box!.x - movedBox!.x)).toBeLessThan(5);
      expect(Math.abs(box!.y - movedBox!.y)).toBeLessThan(5);
    }).toPass();
  });
});

  test('delete two nodes together, one undo brings both back, one redo deletes both again', async ({ page }) => {
    // Helpers to count all nodes and all connections on the screen
    const nodes = page.locator('[data-testid^="node-"]');
    const edges = page.locator('[data-testid^="edge-"]');

    // Add four nodes. The chain from the checklist is:
    // A = node 0, B = node 1, C = node 2, D = node 3
    const node_0 = await addNode(page, 'Dummy', 0);
    const node_1 = await addNode(page, 'Dummy', 1);
    const node_2 = await addNode(page, 'Dummy', 2);
    const node_3 = await addNode(page, 'Dummy', 3);

    // New nodes lie on top of each other, so move them apart into a row,
    // starting with the last one (it lies on top)
    await dragElementByOffset(page, node_3, 300, 0);
    await dragElementByOffset(page, node_2, 100, 0);
    await dragElementByOffset(page, node_1, -100, 0);
    await dragElementByOffset(page, node_0, -300, 0);

    // Check that the nodes now stand in a row from left to right
    const row_0 = await node_0.boundingBox();
    const row_1 = await node_1.boundingBox();
    const row_2 = await node_2.boundingBox();
    const row_3 = await node_3.boundingBox();
    expect(row_0).not.toBeNull();
    expect(row_1).not.toBeNull();
    expect(row_2).not.toBeNull();
    expect(row_3).not.toBeNull();
    expect(row_1!.x).toBeGreaterThan(row_0!.x + 100);
    expect(row_2!.x).toBeGreaterThan(row_1!.x + 100);
    expect(row_3!.x).toBeGreaterThan(row_2!.x + 100);

    // Connect the nodes to a chain: node 0 -> node 1 -> node 2 -> node 3
    await connectElements(
      page,
      node_0.getByTestId('port-output-output_1'),
      node_1.getByTestId('port-input-input_1')
    );
    await connectElements(
      page,
      node_1.getByTestId('port-output-output_1'),
      node_2.getByTestId('port-input-input_1')
    );
    await connectElements(
      page,
      node_2.getByTestId('port-output-output_1'),
      node_3.getByTestId('port-input-input_1')
    );

    // Check that there are 4 nodes and 3 lines
    await expect(nodes).toHaveCount(4);
    await expect(edges).toHaveCount(3);

    // Work out a rectangle around the two middle nodes (node 1 and node 2):
    // it starts in the gap after node 0 and ends in the gap before node 3
    const left = (row_0!.x + row_0!.width + row_1!.x) / 2;
    const right = (row_2!.x + row_2!.width + row_3!.x) / 2;
    const top = Math.min(row_1!.y, row_2!.y) - 30;
    const bottom = Math.max(row_1!.y + row_1!.height, row_2!.y + row_2!.height) + 30;

    // Draw the rectangle on the empty canvas with the mouse
    await page.mouse.move(left, top);
    await page.mouse.down();
    await page.mouse.move(right, bottom, { steps: 10 });
    await page.mouse.up();

    // Press DELETE: the two middle nodes and all the lines should be gone
    await page.keyboard.press('Delete');
    await expect(nodes).toHaveCount(2);
    await expect(edges).toHaveCount(0);
    await expect(node_0).toBeVisible();
    await expect(node_3).toBeVisible();

    // Click the first node so the editor listens to the keyboard
    await node_0.click();

    // Press Ctrl+Z only once: both nodes and all three lines should come back
    // (we count them, because restored nodes may get new names)
    await page.keyboard.press('Control+z');
    await expect(nodes).toHaveCount(4);
    await expect(edges).toHaveCount(3);

    // Press Ctrl+Y only once: both nodes and all the lines should be gone again
    await page.keyboard.press('Control+y');
    await expect(nodes).toHaveCount(2);
    await expect(edges).toHaveCount(0);
  });
  