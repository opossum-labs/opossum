import { test, expect } from '@playwright/test';
import { addNode } from './helpers/editor_actions';

test.describe('Opossum checklist: 8. Reference nodes', () => {

  test.beforeEach(async ({ page }) => {
    // Open the editor before every test
    await page.goto('/');
  });

  test('add a reference node, undo removes it and redo adds it again', async ({ page }) => {
    // Helper to count all nodes on the screen
    const nodes = page.locator('[data-testid^="node-"]');

    // The reference node shows the name "ref (dummy)" in its header
    const referenceNode = nodes.filter({ hasText: 'ref (dummy)' });

    // Add one dummy node
    const node_0 = await addNode(page, 'Dummy', 0);
    await expect(nodes).toHaveCount(1);

    // Right click the node and choose "Create reference"
    await node_0.click({ button: 'right' });
    await page.getByText('Create reference').click();

    // The reference node appears next to the dummy node
    await expect(referenceNode).toBeVisible();
    await expect(nodes).toHaveCount(2);

    // Click the original node so the editor listens to the keyboard
    await node_0.click();

    // Press Ctrl+Z: the reference node disappears, the dummy node stays
    await page.keyboard.press('Control+z');
    await expect(referenceNode).toHaveCount(0);
    await expect(nodes).toHaveCount(1);
    await expect(node_0).toBeVisible();

    // Press Ctrl+Y: the reference node comes back
    await page.keyboard.press('Control+y');
    await expect(referenceNode).toBeVisible();
    await expect(nodes).toHaveCount(2);
  });

  test('delete the target node of a reference, the reference node is deleted too and undo restores both', async ({ page }) => {
    // Helper to count all nodes on the screen
    const nodes = page.locator('[data-testid^="node-"]');

    // The reference node shows the name "ref (dummy)" in its header
    const referenceNode = nodes.filter({ hasText: 'ref (dummy)' });

    // Add one dummy node and create a reference node for it
    const node_0 = await addNode(page, 'Dummy', 0);
    await node_0.click({ button: 'right' });
    await page.getByText('Create reference').click();
    await expect(nodes).toHaveCount(2);

    // Select the dummy node and press DELETE: the reference node goes away with it
    await node_0.click();
    await page.keyboard.press('Delete');
    await expect(nodes).toHaveCount(0);

    // Press Ctrl+Z: both nodes are back, exactly one reference node and no duplicates
    // (we count the nodes, because restored nodes may get new names)
    await page.keyboard.press('Control+z');
    await expect(nodes).toHaveCount(2);
    await expect(referenceNode).toHaveCount(1);
  });

  test('delete the reference node itself, undo restores it and redo deletes it again', async ({ page }) => {
    // Helper to count all nodes on the screen
    const nodes = page.locator('[data-testid^="node-"]');

    // The reference node shows the name "ref (dummy)" in its header
    const referenceNode = nodes.filter({ hasText: 'ref (dummy)' });

    // Add one dummy node and create a reference node for it
    const node_0 = await addNode(page, 'Dummy', 0);
    await node_0.click({ button: 'right' });
    await page.getByText('Create reference').click();
    await expect(referenceNode).toBeVisible();
    await expect(nodes).toHaveCount(2);

    // Select the reference node and press DELETE: only the reference node is gone
    await referenceNode.click();
    await page.keyboard.press('Delete');
    await expect(referenceNode).toHaveCount(0);
    await expect(nodes).toHaveCount(1);
    await expect(node_0).toBeVisible();

    // Press Ctrl+Z: the reference node is back
    await page.keyboard.press('Control+z');
    await expect(referenceNode).toBeVisible();
    await expect(nodes).toHaveCount(2);

    // Press Ctrl+Y: the reference node is deleted again
    await page.keyboard.press('Control+y');
    await expect(referenceNode).toHaveCount(0);
    await expect(nodes).toHaveCount(1);
    await expect(node_0).toBeVisible();
  });
});