import { test, expect } from '@playwright/test';

test.describe('Basic node handling', () => {
  test('add dummy node and verify type in general settings', async ({ page }) => {
    // 1. Navigate to the Dioxus web application
    await page.goto('/');

    // 2. Open the main 'Edit' menu and verify it opened
    const editMenu = page.getByRole('button', { name: 'Edit' });
    await expect(editMenu).toBeVisible();
    await editMenu.click();

    // 3. Trigger the 'Add Node' submenu/item and ensure it is ready
    const addNodeOption = page.getByRole('button', { name: 'Add Node' });
    await expect(addNodeOption).toBeVisible();
    await addNodeOption.click();

    // 4. Locate the 'Dummy' item and ensure it is fully visible and interactive
    const dummyOption = page.getByRole('button', { name: 'Dummy' });
    await expect(dummyOption).toBeVisible();

    // Perform the click action to spawn the node
    await dummyOption.click();

    await expect(page.getByTestId('node-0')).toBeVisible();

    // 6. Switch to the General settings tab
    const generalTab = page.getByRole('button', { name: 'General' });
    await expect(generalTab).toBeVisible();
    await generalTab.click();

    // 7. Assert that the input field contains the expected value
    const nodeTypeInput = page.getByRole('textbox', { name: 'Node Type' });
    await expect(nodeTypeInput).toHaveValue('dummy');
  });
});