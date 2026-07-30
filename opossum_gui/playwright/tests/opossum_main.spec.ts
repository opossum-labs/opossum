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

  // test('should display the main application title', async ({ page }) => {
  //   // Locate an heading element on the page
  //   const titleLocator = page.locator('h1');

  //   // Assert that the title is visible and contains expected content
  //   await expect(titleLocator).toBeVisible();
  // });

  // test('should interact with control buttons', async ({ page }) => {
  //   // Find a button by its user-visible text or accessible role
  //   const actionButton = page.getByRole('button', { name: /start/i });

  //   // Ensure the button is enabled before clicking
  //   await expect(actionButton).toBeEnabled();
    
  //   // Trigger a click event
  //   await actionButton.click();

  //   // Verify the result of the action (e.g. check for a visible status message)
  //   const resultElement = page.locator('#status-display');
  //   await expect(resultElement).toHaveText(/completed/i);
  // });
});