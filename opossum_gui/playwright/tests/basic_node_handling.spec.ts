import { test, expect } from '@playwright/test';
import { dragElementByOffset, connectElements } from './helpers/editor_actions';

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

  test('should add two nodes and connect them by mouse', async ({ page }) => {
    // Create first node
    const editMenu = page.getByRole('button', { name: 'Edit' });
    await expect(editMenu).toBeVisible();
    await editMenu.click();
    const addNodeOption = page.getByRole('button', { name: 'Add Node' });
    await expect(addNodeOption).toBeVisible();
    await addNodeOption.click();
    const dummyOption = page.getByRole('button', { name: 'Dummy' });
    await expect(dummyOption).toBeVisible();
    await dummyOption.click();
    const node_0 = page.getByTestId('node-0');
    await expect(node_0).toBeVisible();
    // Drag first node
    await dragElementByOffset(page, node_0, -300, 0);

    // Create second node
    await editMenu.click();
    await expect(addNodeOption).toBeVisible();
    await addNodeOption.click();
    await expect(dummyOption).toBeVisible();
    await dummyOption.click();
    const node_1 = page.getByTestId('node-1');
    await expect(node_1).toBeVisible();

    // 1. Target output port 'out' specifically inside 'node-0'
    const sourcePort = node_0.getByTestId('port-output-output_1');
    await expect(sourcePort).toBeVisible();

    // 2. Target input port 'in' specifically inside 'node-1'
    const targetPort = node_1.getByTestId('port-input-input_1');
    await expect(sourcePort).toBeVisible();

    // 3. Perform drag & drop sequence using the helper function
    await connectElements(page, sourcePort, targetPort);

    // 4. Assert that the persistent edge-0 is generated and visible on canvas
    const createdEdge = page.getByTestId('edge-0');
    await expect(createdEdge).toBeVisible();
  });
});