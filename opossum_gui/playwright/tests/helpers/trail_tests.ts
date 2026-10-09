import { test, expect } from '@playwright/test';
import { dragElementByOffset, addNode }  from '../helpers/editor_actions';
test.describe('Undo and redo', () => {

  test.beforeEach(async ({ page }) => {
    // Open the scenery editor
    await page.goto('/');
  });

  test('undo and redo adding a node', async ({ page }) => {
    const node_0 = await addNode(page, 'Dummy', 0);

    // Click the node first so the editor receives the keyboard shortcuts
    await node_0.click();

    // Ctrl+Z should remove the node again
    await page.keyboard.press('Control+z');
    await expect(node_0).not.toBeVisible();

    // Ctrl+Y should bring it back
    await page.keyboard.press('Control+y');
    await expect(node_0).toBeVisible();
  });

  test('undo and redo moving a node', async ({ page }) => {
    const node_0 = await addNode(page, 'Dummy', 0);

    // Remember where the node starts
    const startBox = await node_0.boundingBox();
    expect(startBox).not.toBeNull();

    // Drag the node 200px to the right and check that it really moved
    await dragElementByOffset(page, node_0, 200, 0);
    const movedBox = await node_0.boundingBox();
    expect(movedBox).not.toBeNull();
    expect(movedBox!.x).toBeGreaterThan(startBox!.x + 150);

    // Ctrl+Z should put the node back near its starting position
    await page.keyboard.press('Control+z');
    await expect(async () => {
      const box = await node_0.boundingBox();
      expect(box!.x).toBeLessThan(startBox!.x + 50);
    }).toPass();

    // Ctrl+Y should move it to the right again
    await page.keyboard.press('Control+y');
    await expect(async () => {
      const box = await node_0.boundingBox();
      expect(box!.x).toBeGreaterThan(startBox!.x + 150);
    }).toPass();
  });

  test('undo removes the last added node', async ({ page }) => {
    const node_0 = await addNode(page, 'Dummy', 0);
    const node_1 = await addNode(page, 'Dummy', 1);

    // Select the first node
    await node_0.click();

    // Undo removes the node that was added last, not the selected one
    await page.keyboard.press('Control+z');
    await expect(node_1).not.toBeVisible();
    await expect(node_0).toBeVisible();

    // Redo brings the removed node back
    await page.keyboard.press('Control+y');
    await expect(node_1).toBeVisible();
  });
});