import { test, expect } from '@playwright/test';
import { dragElementByOffset, connectElements, addNode } from './helpers/editor_actions';

test.describe('My tests', () => {

  test.beforeEach(async ({ page }) => {
    // Navigate to the scenery editor workspace
    await page.goto('/');
  });

  test('my special test', async ({ page }) => {
    await addNode(page, 'Dummy', 0);

    const generalTab = page.getByRole('button', { name: 'General' });
    await expect(generalTab).toBeVisible();
    await generalTab.click();

    const nodeTypeInput = page.getByRole('textbox', { name: 'Node Type' });
    await expect(nodeTypeInput).toHaveValue('dummy');
  });
});