import { test, expect } from '@playwright/test';
import { dragElementByOffset, connectElements, addNode } from './helpers/editor_actions';

test.describe('Opossum checklist: 5. Cut / Copy / Paste', () => {

  test.beforeEach(async ({ page }) => {
    // Open the editor before every test
    await page.goto('/');
  });

  test.describe('Copy -> Paste', () => {

    test('copy a node and paste it, undo removes the copy and redo brings it back', async ({ page }) => {
      // Helper to count all nodes on the screen
      const nodes = page.locator('[data-testid^="node-"]');

      // Add one node
      const node_0 = await addNode(page, 'Dummy', 0);
      await expect(nodes).toHaveCount(1);

      // Select the node and press Ctrl+C
      await node_0.click();
      await page.keyboard.press('Control+c');

      // Press Ctrl+V: a duplicate appears
      await page.keyboard.press('Control+v');
      await expect(nodes).toHaveCount(2);

      // Press Ctrl+Z: the pasted node disappears, the original stays
      await page.keyboard.press('Control+z');
      await expect(nodes).toHaveCount(1);

      // Press Ctrl+Y: the pasted node comes back
      await page.keyboard.press('Control+y');
      await expect(nodes).toHaveCount(2);
    });
  });

  test.describe('Cut -> Paste', () => {

    test('cut a node and paste it elsewhere, one undo reverses both and redo moves it again', async ({ page }) => {
      // Helper to count all nodes on the screen
      const nodes = page.locator('[data-testid^="node-"]');

      // Add one node and remember where it is
      const node_0 = await addNode(page, 'Dummy', 0);
      const startBox = await node_0.boundingBox();
      expect(startBox).not.toBeNull();

      // Select the node and press Ctrl+X: the node is gone
      await node_0.click();
      await page.keyboard.press('Control+x');
      await expect(nodes).toHaveCount(0);

      // Move the mouse to another place and press Ctrl+V: the node appears again
      await page.mouse.move(startBox!.x + 250, startBox!.y + 100);
      await page.keyboard.press('Control+v');
      await expect(nodes).toHaveCount(1);

      // The node should be at the new place
      const pastedBox = await nodes.first().boundingBox();
      expect(pastedBox).not.toBeNull();
      expect(pastedBox!.x).toBeGreaterThan(startBox!.x + 150);

      // Press Ctrl+Z only once: paste and delete are reversed together, the node is back at the start
      await page.keyboard.press('Control+z');
      await expect(nodes).toHaveCount(1);
      await expect(async () => {
        const box = await nodes.first().boundingBox();
        expect(Math.abs(box!.x - startBox!.x)).toBeLessThan(5);
        expect(Math.abs(box!.y - startBox!.y)).toBeLessThan(5);
      }).toPass();

      // Press Ctrl+Y: the node is moved again
      await page.keyboard.press('Control+y');
      await expect(nodes).toHaveCount(1);
      await expect(async () => {
        const box = await nodes.first().boundingBox();
        expect(box!.x).toBeGreaterThan(startBox!.x + 150);
      }).toPass();
    });
  });

  test.describe('Pasting a group', () => {

    test('copy and paste a group, internal connection stays, undo removes the group and redo brings it back', async ({ page }) => {
      // Helpers to count all nodes and all connections on the screen
      const nodes = page.locator('[data-testid^="node-"]');
      const edges = page.locator('[data-testid^="edge-"]');

      // Add two nodes
      const node_0 = await addNode(page, 'Dummy', 0);
      const node_1 = await addNode(page, 'Dummy', 1);

      // Move the first node away, so the two nodes do not sit on top of each other
      await dragElementByOffset(page, node_0, -150, 0);

      // Connect the two nodes with a line
      await connectElements(page, node_0.getByTestId('port-output-output_1'), node_1.getByTestId('port-input-input_1'));

      // Select both nodes by drawing a rectangle around them
      const box_0 = await node_0.boundingBox();
      const box_1 = await node_1.boundingBox();
      expect(box_0).not.toBeNull();
      expect(box_1).not.toBeNull();
      await page.mouse.move(box_0!.x - 30, box_0!.y - 30);
      await page.mouse.down();
      await page.mouse.move(box_1!.x + box_1!.width + 30, box_1!.y + box_1!.height + 30, { steps: 10 });
      await page.mouse.up();

      // Click the green "Convert to Group" button
      await page.getByRole('button', { name: /convert to group/i }).click();
      const group = nodes.filter({ hasText: 'new group' });
      await expect(group).toBeVisible();
      await expect(nodes).toHaveCount(1);

      // Select the group, press Ctrl+C and Ctrl+V: a second group appears
      await group.click();
      await page.keyboard.press('Control+c');
      await page.keyboard.press('Control+v');
      await expect(nodes).toHaveCount(2);

      // Open the pasted group: the line between the two inner nodes is still there
      await nodes.last().dblclick(); 
      await expect(nodes).toHaveCount(2); 
      await expect(edges).toHaveCount(1); 
      await page.getByRole('tab', { name: 'unsaved' }).click(); 

      // Select the pasted group so the editor listens to the keyboard
      await nodes.last().click();

      // Press Ctrl+Z: the pasted group disappears
      await page.keyboard.press('Control+z');
      await expect(nodes).toHaveCount(1);

      // Press Ctrl+Y: the pasted group comes back
      await page.keyboard.press('Control+y');
      await expect(nodes).toHaveCount(2);

      // The log must not show any Error 400
      await expect(page.getByText(/Error 400/)).toHaveCount(0);
    });
  });

  test.describe('Pasting a double-nested group', () => {

    test('copy and paste a group that contains another group, undo and redo work and the log shows no error', async ({ page }) => {
      // Helper to count all nodes on the screen
      const nodes = page.locator('[data-testid^="node-"]');

      // Add two nodes
      const node_0 = await addNode(page, 'Dummy', 0);
      const node_1 = await addNode(page, 'Dummy', 1);

      // Move the first node away, so the two nodes do not sit on top of each other
      await dragElementByOffset(page, node_0, -150, 0);

      // Connect the two nodes with a line
      await connectElements(page, node_0.getByTestId('port-output-output_1'), node_1.getByTestId('port-input-input_1'));

      // Select both nodes by drawing a rectangle around them
      const box_0 = await node_0.boundingBox();
      const box_1 = await node_1.boundingBox();
      expect(box_0).not.toBeNull();
      expect(box_1).not.toBeNull();
      await page.mouse.move(box_0!.x - 30, box_0!.y - 30);
      await page.mouse.down();
      await page.mouse.move(box_1!.x + box_1!.width + 30, box_1!.y + box_1!.height + 30, { steps: 10 });
      await page.mouse.up();

      // Click the green "Convert to Group" button: this is the inner group
      await page.getByRole('button', { name: /convert to group/i }).click();
      const innerGroup = nodes.filter({ hasText: 'new group' });
      await expect(innerGroup).toBeVisible();

      // Add two more nodes and move them away from each other and from the group
      const node_2 = await addNode(page, 'Dummy'); 
      const node_3 = await addNode(page, 'Dummy'); 
      await dragElementByOffset(page, node_3, 300, 0);
      await dragElementByOffset(page, node_2, 300, 200);
      await expect(nodes).toHaveCount(3);

      // Select the inner group and the two new nodes with one big rectangle
      const groupBox = await innerGroup.boundingBox();
      const box_3 = await node_3.boundingBox();
      const box_2 = await node_2.boundingBox();
      expect(groupBox).not.toBeNull();
      expect(box_3).not.toBeNull();
      expect(box_2).not.toBeNull();
      await page.mouse.move(groupBox!.x - 30, groupBox!.y - 30); 
      await page.mouse.down();
      await page.mouse.move(
        Math.max(box_3!.x + box_3!.width, box_2!.x + box_2!.width) + 30,
        Math.max(box_3!.y + box_3!.height, box_2!.y + box_2!.height) + 30,
        { steps: 10 }
      );
      await page.mouse.up();

      // Click the green "Convert to Group" button: this is the outer group
      await page.getByRole('button', { name: /convert to group/i }).click();
      await expect(nodes).toHaveCount(1);

      // Select the outer group, press Ctrl+C and Ctrl+V: a second outer group appears
      await nodes.first().click();
      await page.keyboard.press('Control+c');
      await page.keyboard.press('Control+v');
      await expect(nodes).toHaveCount(2);

      // Select the pasted group and press Ctrl+Z: the pasted group disappears
      await nodes.last().click(); 
      await page.keyboard.press('Control+z');
      await expect(nodes).toHaveCount(1);

      // Press Ctrl+Y: the pasted group comes back
      await page.keyboard.press('Control+y');
      await expect(nodes).toHaveCount(2);

      // The log must not show any Error 400
      await expect(page.getByText(/Error 400/)).toHaveCount(0);
    });
  });

  test.describe('Cut special cases', () => {

    test('cut one of two connected nodes, the connection is lost and undo restores node and connection', async ({ page }) => {
      // Helpers to count all nodes and all connections on the screen
      const nodes = page.locator('[data-testid^="node-"]');
      const edges = page.locator('[data-testid^="edge-"]');

      // Add two nodes
      const node_0 = await addNode(page, 'Dummy', 0);
      const node_1 = await addNode(page, 'Dummy', 1);

      // Move the first node away, so the two nodes do not sit on top of each other
      await dragElementByOffset(page, node_0, -300, 0);

      // Connect the two nodes with a line
      await connectElements(page, node_0.getByTestId('port-output-output_1'), node_1.getByTestId('port-input-input_1'));
      await expect(nodes).toHaveCount(2);
      await expect(edges).toHaveCount(1);

      // Select the second node and press Ctrl+X: the node and its line are gone
      await node_1.click();
      await page.keyboard.press('Control+x');
      await expect(nodes).toHaveCount(1);
      await expect(edges).toHaveCount(0);

      // Move the mouse to another place and press Ctrl+V: the node appears, the line stays lost
      await page.mouse.move(600, 500); 
      await page.keyboard.press('Control+v');
      await expect(nodes).toHaveCount(2);
      await expect(edges).toHaveCount(0);

      // Press Ctrl+Z once: the node and its line are back as before
      await page.keyboard.press('Control+z');
      await expect(nodes).toHaveCount(2);
      await expect(edges).toHaveCount(1);
    });

    test('cut a node that a reference node points to, paste it back, the connection is there and undo and redo work', async ({ page }) => {
      // Helpers to count all nodes and all connections on the screen
      const nodes = page.locator('[data-testid^="node-"]');
      const edges = page.locator('[data-testid^="edge-"]');

      // Add one node
      const node_0 = await addNode(page, 'Dummy', 0);

      // Right click the node and choose the reference node option
      await node_0.click({ button: 'right' });
      await page.getByRole('menuitem', { name: /reference/i }).click(); 
      await expect(nodes).toHaveCount(2);

      // The new reference node is the last node on the screen, move it away from node 0
      const referenceNode = nodes.last();
      await dragElementByOffset(page, referenceNode, 300, 0);

      // Connect node 0 and the reference node with a line
      const sourcePort = node_0.getByTestId('port-output-output_1'); 
      const targetPort = referenceNode.getByTestId('port-input-input_1'); 
      await connectElements(page, sourcePort, targetPort);
      await expect(edges).toHaveCount(1);

      // Select node 0 and press Ctrl+X: the connection is lost
      await node_0.click();
      await page.keyboard.press('Control+x');
      await expect(edges).toHaveCount(0);

      // Move the mouse to an empty place and press Ctrl+V: the node is back and the connection is there again
      await page.mouse.move(600, 500); 
      await page.keyboard.press('Control+v');
      await expect(edges).toHaveCount(1);

      // Press Ctrl+Z once: node and reference are as before the cut
      await page.keyboard.press('Control+z');
      await expect(nodes).toHaveCount(2);
      await expect(edges).toHaveCount(1);

      // Press Ctrl+Y once: the node is moved again and the connection is still there
      await page.keyboard.press('Control+y');
      await expect(nodes).toHaveCount(2);
      await expect(edges).toHaveCount(1);
    });

    test('cut and paste a node twice in a row, undo and redo work for both', async ({ page }) => {
      // Helper to count all nodes on the screen
      const nodes = page.locator('[data-testid^="node-"]');

      // Add one node
      const node_0 = await addNode(page, 'Dummy', 0);

      // First cut and paste
      await node_0.click();
      await page.keyboard.press('Control+x');
      await expect(nodes).toHaveCount(0);
      await page.mouse.move(500, 400); 
      await page.keyboard.press('Control+v');
      await expect(nodes).toHaveCount(1);
      const firstBox = await nodes.first().boundingBox();
      expect(firstBox).not.toBeNull();

      // Second cut and paste of the pasted node
      await nodes.first().click();
      await page.keyboard.press('Control+x');
      await expect(nodes).toHaveCount(0);
      await page.mouse.move(800, 500);
      await page.keyboard.press('Control+v');
      await expect(nodes).toHaveCount(1);
      const secondBox = await nodes.first().boundingBox();
      expect(secondBox).not.toBeNull();
      expect(secondBox!.x).toBeGreaterThan(firstBox!.x + 100); 

      // Press Ctrl+Z once: the second move is undone, the node is at the first place
      await page.keyboard.press('Control+z');
      await expect(nodes).toHaveCount(1);
      await expect(async () => {
        const box = await nodes.first().boundingBox();
        expect(Math.abs(box!.x - firstBox!.x)).toBeLessThan(5);
      }).toPass();

      // Press Ctrl+Y: the second move is done again
      await page.keyboard.press('Control+y');
      await expect(nodes).toHaveCount(1);
      await expect(async () => {
        const box = await nodes.first().boundingBox();
        expect(Math.abs(box!.x - secondBox!.x)).toBeLessThan(5);
      }).toPass();
    });
  });
});