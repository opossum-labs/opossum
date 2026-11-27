import { test, expect } from '@playwright/test';
import { dragElementByOffset, addNode } from './helpers/editor_actions';

test.describe('Opossum checklist: 7. Analyzer', () => {

  test.beforeEach(async ({ page }) => {
    // Open the editor before every test
    await page.goto('/');
  });

  test('add an analyzer, undo removes it and redo adds it again', async ({ page }) => {
    // The analyzer node shows its name "RayTracing" in the header
    const analyzer = page.locator('[data-testid^="node-"]').filter({ hasText: 'RayTracing' });

    // Add the analyzer via Node -> Add Analyzer -> RayTracing
    await page.getByRole('button', { name: 'Node', exact: true }).click();
    await page.getByRole('button', { name: 'Add Analyzer' }).click();
    await page.getByRole('button', { name: /ray\s?tracing/i }).click();

    // The analyzer appears and the analyzer panel is shown on the left
    await expect(analyzer).toBeVisible();
    await expect(page.getByText('Analyzer Configuration')).toBeVisible();

    // Click the analyzer so the editor listens to the keyboard
    await analyzer.click();

    // Press Ctrl+Z: the analyzer disappears
    await page.keyboard.press('Control+z');
    await expect(analyzer).not.toBeVisible();

    // Press Ctrl+Y: the analyzer comes back
    await page.keyboard.press('Control+y');
    await expect(analyzer).toBeVisible();
  });

  test('delete an analyzer, undo restores it and redo deletes it again', async ({ page }) => {
    const analyzer = page.locator('[data-testid^="node-"]').filter({ hasText: 'RayTracing' });

    // Add the analyzer via Node -> Add Analyzer -> RayTracing
    await page.getByRole('button', { name: 'Node', exact: true }).click();
    await page.getByRole('button', { name: 'Add Analyzer' }).click();
    await page.getByRole('button', { name: /ray\s?tracing/i }).click();
    await expect(analyzer).toBeVisible();

    // Select the analyzer and press DELETE: it should be gone
    await analyzer.click();
    await page.keyboard.press('Delete');
    await expect(analyzer).not.toBeVisible();

    // Press Ctrl+Z: the analyzer comes back
    await page.keyboard.press('Control+z');
    await expect(analyzer).toBeVisible();

    // Press Ctrl+Y: the analyzer is deleted again
    await page.keyboard.press('Control+y');
    await expect(analyzer).not.toBeVisible();
  });

  test('change an analyzer setting, undo shows the old value in the panel and redo shows the new value', async ({ page }) => {
    const analyzer = page.locator('[data-testid^="node-"]').filter({ hasText: 'RayTracing' });

    // Add the analyzer via Node -> Add Analyzer -> RayTracing
    await page.getByRole('button', { name: 'Node', exact: true }).click();
    await page.getByRole('button', { name: 'Add Analyzer' }).click();
    await page.getByRole('button', { name: /ray\s?tracing/i }).click();
    await expect(analyzer).toBeVisible();

    // The setting "Max refractions" starts with 1000
    const maxRefractions = page.getByRole('textbox', { name: 'Max refractions' });
    await expect(maxRefractions).toHaveValue('1000');

    // Change it to 500
    await maxRefractions.click();
    await maxRefractions.fill('500');
    await page.keyboard.press('Enter');
    await expect(maxRefractions).toHaveValue('500');

    // Click the analyzer so the editor listens to the keyboard
    await analyzer.click();

    // Press Ctrl+Z: the panel shows the old value right away
    await page.keyboard.press('Control+z');
    await expect(maxRefractions).toHaveValue('1000');

    // Press Ctrl+Y: the panel shows the new value again
    await page.keyboard.press('Control+y');
    await expect(maxRefractions).toHaveValue('500');
  });

  test('move an analyzer, undo restores the position and redo moves it again', async ({ page }) => {
    const analyzer = page.locator('[data-testid^="node-"]').filter({ hasText: 'RayTracing' });

    // Add the analyzer via Node -> Add Analyzer -> RayTracing
    await page.getByRole('button', { name: 'Node', exact: true }).click();
    await page.getByRole('button', { name: 'Add Analyzer' }).click();
    await page.getByRole('button', { name: /ray\s?tracing/i }).click();
    await expect(analyzer).toBeVisible();

    // Remember where the analyzer starts
    const startBox = await analyzer.boundingBox();
    expect(startBox).not.toBeNull();

    // Drag the analyzer to a new place and check that it really moved
    await dragElementByOffset(page, analyzer, 200, 100);
    const movedBox = await analyzer.boundingBox();
    expect(movedBox).not.toBeNull();
    expect(movedBox!.x).toBeGreaterThan(startBox!.x + 150);
    expect(movedBox!.y).toBeGreaterThan(startBox!.y + 50);

    // Press Ctrl+Z: the analyzer jumps back to where it started
    await page.keyboard.press('Control+z');
    await expect(async () => {
      const box = await analyzer.boundingBox();
      expect(Math.abs(box!.x - startBox!.x)).toBeLessThan(5);
      expect(Math.abs(box!.y - startBox!.y)).toBeLessThan(5);
    }).toPass();

    // Press Ctrl+Y: the analyzer goes to the new place again
    await page.keyboard.press('Control+y');
    await expect(async () => {
      const box = await analyzer.boundingBox();
      expect(Math.abs(box!.x - movedBox!.x)).toBeLessThan(5);
      expect(Math.abs(box!.y - movedBox!.y)).toBeLessThan(5);
    }).toPass();
  });

  test('add a source port node, undo removes it and redo adds it again', async ({ page }) => {
    const analyzer = page.locator('[data-testid^="node-"]').filter({ hasText: 'RayTracing' });
    const sourcePorts = page.locator('[data-testid^="node-"]').filter({ hasText: 'source port' });

    // Add the analyzer via Node -> Add Analyzer -> RayTracing
    await page.getByRole('button', { name: 'Node', exact: true }).click();
    await page.getByRole('button', { name: 'Add Analyzer' }).click();
    await page.getByRole('button', { name: /ray\s?tracing/i }).click();
    await expect(analyzer).toBeVisible();

    // No source port yet, so the analyzer panel says so
    await expect(page.getByText('No Source Ports found.')).toBeVisible();

    // Add a source port node via Node -> Add Node -> Source port
    await addNode(page, 'Source port');
    await expect(sourcePorts).toHaveCount(1);

    // Select the analyzer again: the source port is now in its source definitions
    await analyzer.click();
    await expect(page.getByText('No Source Ports found.')).not.toBeVisible();

    // Press Ctrl+Z: the source port disappears and the analyzer has no source again
    await page.keyboard.press('Control+z');
    await expect(sourcePorts).toHaveCount(0);
    await analyzer.click();
    await expect(page.getByText('No Source Ports found.')).toBeVisible();

    // Press Ctrl+Y: the source port comes back
    await page.keyboard.press('Control+y');
    await expect(sourcePorts).toHaveCount(1);
  });

  test('delete a source port node, undo restores exactly one source port', async ({ page }) => {
    const analyzer = page.locator('[data-testid^="node-"]').filter({ hasText: 'RayTracing' });
    const sourcePorts = page.locator('[data-testid^="node-"]').filter({ hasText: 'source port' });

    // Add the analyzer via Node -> Add Analyzer -> RayTracing
    await page.getByRole('button', { name: 'Node', exact: true }).click();
    await page.getByRole('button', { name: 'Add Analyzer' }).click();
    await page.getByRole('button', { name: /ray\s?tracing/i }).click();
    await expect(analyzer).toBeVisible();

    // Add a source port node via Node -> Add Node -> Source port
    await addNode(page, 'Source port');
    await expect(sourcePorts).toHaveCount(1);

    // Select the source port and press DELETE: it should be gone
    await sourcePorts.first().click();
    await page.keyboard.press('Delete');
    await expect(sourcePorts).toHaveCount(0);

    // Press Ctrl+Z: exactly one source port is back (no duplicate)
    await page.keyboard.press('Control+z');
    await expect(sourcePorts).toHaveCount(1);

    // The analyzer is still there and its panel opens without a problem
    await analyzer.click();
    await expect(page.getByText('Analyzer Configuration')).toBeVisible();
  });
});