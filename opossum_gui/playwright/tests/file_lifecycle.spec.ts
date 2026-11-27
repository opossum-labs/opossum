import { test, expect } from '@playwright/test';
import { connectElements, dragElementByOffset, addNode } from './helpers/editor_actions';

test.describe('Opossum checklist: 10. File lifecycle', () => {

  test.beforeEach(async ({ page }) => {
    // Open the editor before every test
    await page.goto('/');
  });

  test('new project clears the editor and the undo history', async ({ page }) => {
    // Helpers to count all nodes and all connections on the screen
    const nodes = page.locator('[data-testid^="node-"]');
    const edges = page.locator('[data-testid^="edge-"]');

    // Add two nodes and connect them
    const node_0 = await addNode(page, 'Dummy', 0);
    const node_1 = await addNode(page, 'Dummy', 1);
    await dragElementByOffset(page, node_0, -300, 0);
    await connectElements(page, node_0.getByTestId('port-output-output_1'), node_1.getByTestId('port-input-input_1'));
    await expect(nodes).toHaveCount(2);
    await expect(edges).toHaveCount(1);

    // File -> New Project: the dialog about unsaved changes appears
    await page.getByRole('button', { name: 'File', exact: true }).click();
    await page.getByText('New Project').click();
    await expect(page.getByText('Unsaved Changes')).toBeVisible();

    // Click "Yes" to discard the changes
    await page.getByRole('button', { name: 'Yes' }).click();

    // The editor is empty again
    await expect(nodes).toHaveCount(0);
    await expect(edges).toHaveCount(0);

    // Press Ctrl+Z and Ctrl+Y: nothing comes back, the history is empty
    await page.keyboard.press('Control+z');
    await page.keyboard.press('Control+y');
    await expect(nodes).toHaveCount(0);
    await expect(edges).toHaveCount(0);
  });
});