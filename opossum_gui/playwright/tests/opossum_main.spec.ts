import { test, expect } from '@playwright/test';

test.describe('OPOSSUM GUI Main Features', () => {

  // This hook runs before each individual test
  test.beforeEach(async ({ page }) => {
    // Navigates to the baseURL configured in playwright.config.ts (http://127.0.0.1:8085)
    await page.goto('/');
  });

  test('has title', async ({ page }) => {
    // Expect a title "to contain" a substring.
    await expect(page).toHaveTitle(/OPOSSUM/);
  });
  test('Open About dialog', async ({ page }) => {
    await page.goto('http://localhost:8085/');
    await page.getByRole('button', { name: 'Help' }).click();
    await page.getByRole('button', { name: 'About' }).click();
    await expect(page.getByRole('button', { name: 'Close' })).toBeVisible();
  });
});