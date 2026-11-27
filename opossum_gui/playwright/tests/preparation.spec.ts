import { test, expect } from '@playwright/test';

test.describe('Opossum checklist: 0. Preparation', () => {

  test.beforeEach(async ({ page }) => {
    // Navigate to the scenery editor workspace
    await page.goto('/');
  });

  test('editor starts with an empty project and no errors in the log', async ({ page }) => {
    // The editor is loaded when the Node menu is visible
    const nodeMenu = page.getByRole('button', { name: 'Node', exact: true });
    await expect(nodeMenu).toBeVisible();

    // No nodes and no connections in a fresh project
    await expect(page.locator('[data-testid^="node-"]')).toHaveCount(0);
    await expect(page.locator('[data-testid^="edge-"]')).toHaveCount(0);

    // The log should not contain any error message
    await expect(page.getByText('Failed to')).toHaveCount(0);
  });
});