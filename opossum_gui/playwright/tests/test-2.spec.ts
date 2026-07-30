import { test, expect } from '@playwright/test';

test('should add a dummy node and verify its type in general settings', async ({ page }) => {
  // Navigate to the root URL (uses baseURL http://127.0.0.1:8085)
  await page.goto('/');

  // Locate and click the 'Edit' button
  const editButton = page.getByRole('button', { name: 'Edit' });
  await expect(editButton).toBeVisible();
  await editButton.click();

  // Ensure the 'Add Node' menu item appears in the DOM before clicking
  const addNodeButton = page.getByRole('button', { name: 'Add Node' });
  await expect(addNodeButton).toBeVisible();
  await addNodeButton.click();

  // Wait until the 'Dummy' node option is visible and ready
  const dummyButton = page.getByRole('button', { name: 'Dummy' });
  await expect(dummyButton).toBeVisible();
  await dummyButton.click();

  // Optional short pause if Dioxus needs a moment to update the state
  await page.waitForTimeout(100);

  // Ensure the 'General' tab is rendered before clicking
  const generalButton = page.getByRole('button', { name: 'General' });
  await expect(generalButton).toBeVisible();
  await generalButton.click();

  // Assert that the 'Node Type' input field receives the expected value
  const nodeTypeInput = page.getByRole('textbox', { name: 'Node Type' });
  await expect(nodeTypeInput).toHaveValue('dummy');
});