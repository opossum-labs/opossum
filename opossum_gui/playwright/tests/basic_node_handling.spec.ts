import { test, expect } from '@playwright/test';
import { dragElementByOffset } from './helpers/editor_actions';

test.describe('Basic node handling', () => {

  test.beforeEach(async ({ page }) => {
    // Navigate to the scenery editor workspace
    await page.goto('/');
  });

  test('add dummy node and verify type in general settings', async ({ page }) => {
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

  test('should drag a new node to a new position by mouse', async ({ page }) => {
   const editMenu = page.getByRole('button', { name: 'Edit' });
    await expect(editMenu).toBeVisible();
    await editMenu.click();
    const addNodeOption = page.getByRole('button', { name: 'Add Node' });
    await expect(addNodeOption).toBeVisible();
    await addNodeOption.click();
    const dummyOption = page.getByRole('button', { name: 'Dummy' });
    await expect(dummyOption).toBeVisible();
    await dummyOption.click();
    const node = page.getByTestId('node-0');
    await expect(node).toBeVisible();

    // Store initial bounding box for verification
    const initialBox = await node.boundingBox();
    expect(initialBox).not.toBeNull();

    // Drag the node by 200px horizontally and 100px vertically
    await dragElementByOffset(page, node, 200, 100);

    // Verify that the node moved to the expected region
    const updatedBox = await node.boundingBox();
    expect(updatedBox).not.toBeNull();

    if (initialBox && updatedBox) {
      expect(updatedBox.x).toBeGreaterThan(initialBox.x + 150);
      expect(updatedBox.y).toBeGreaterThan(initialBox.y + 50);
    }
  });
});