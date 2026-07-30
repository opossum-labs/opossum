import { test, expect } from '@playwright/test';

test('Open About dialog', async ({ page }) => {
  await page.goto('http://localhost:8085/');
  await page.getByRole('button', { name: 'Help' }).click();
  await page.getByRole('button', { name: 'About' }).click();
  await expect(page.getByRole('button', { name: 'Close' })).toBeVisible();
});