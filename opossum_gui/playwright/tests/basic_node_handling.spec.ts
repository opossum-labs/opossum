import { test, expect } from '@playwright/test';
import { dragElementByOffset, connectElements, addNode } from './helpers/editor_actions';

test.describe('Basic node handling', () => {

  test.beforeEach(async ({ page }) => {
    // Navigate to the scenery editor workspace
    await page.goto('/');
  });

  test('add dummy node and verify type in general settings', async ({ page }) => {
    await addNode(page, 'Dummy', 0);

    const generalTab = page.getByRole('button', { name: 'General' });
    await expect(generalTab).toBeVisible();
    await generalTab.click();

    const nodeTypeInput = page.getByRole('textbox', { name: 'Node Type' });
    await expect(nodeTypeInput).toHaveValue('dummy');
  });

  test('add two dummy nodes and change the name', async ({ page }) => {
    const node_0 = await addNode(page, 'Dummy', 0);
    const node_1 = await addNode(page, 'Dummy', 1);
    // Drag first node
    await dragElementByOffset(page, node_0, -300, 0);
    // Select node 0
    await node_0.click();
    // open general tab of node editor
    await page.getByRole('button', { name: 'General' }).click();
    await page.getByRole('textbox', { name: 'Node Name' }).click();
    await page.getByRole('textbox', { name: 'Node Name' }).fill('blah');
    // Select node 0
    await node_1.click();
    await expect(node_0).toContainText('blah');
    await expect(node_1).toContainText('dummy');
  });

  test('add two nodes and delete first one afterwards', async ({ page }) => {
    const node_0 = await addNode(page, 'Dummy', 0);
    const node_1 = await addNode(page, 'Dummy', 1);

    // now active node-0
    await node_0.click();
    // Now press DELETE
    await page.keyboard.press('Delete');
    await expect(node_0).not.toBeVisible();
    // node1 should still be there
    await expect(node_1).toBeVisible();
  });

  test('should drag a new node to a new position by mouse', async ({ page }) => {
    const node_0 = await addNode(page, 'Dummy', 0);

    // Store initial bounding box for verification
    const initialBox = await node_0.boundingBox();
    expect(initialBox).not.toBeNull();

    // Drag the node by 200px horizontally and 100px vertically
    await dragElementByOffset(page, node_0, 200, 100);

    // Verify that the node moved to the expected region
    const updatedBox = await node_0.boundingBox();
    expect(updatedBox).not.toBeNull();

    if (initialBox && updatedBox) {
      expect(updatedBox.x).toBeGreaterThan(initialBox.x + 150);
      expect(updatedBox.y).toBeGreaterThan(initialBox.y + 50);
    }
  });

  test('should add two nodes and connect them by mouse', async ({ page }) => {
    const node_0 = await addNode(page, 'Dummy', 0);
    const node_1 = await addNode(page, 'Dummy', 1);

    // Drag first node
    await dragElementByOffset(page, node_0, -300, 0);

    const sourcePort = node_0.getByTestId('port-output-output_1');
    const targetPort = node_1.getByTestId('port-input-input_1');
    await connectElements(page, sourcePort, targetPort);

    const createdEdge = page.getByTestId('edge-0');
    await expect(createdEdge).toBeVisible();
  });

  test('add three connected nodes and delete first connection', async ({ page }) => {
    const node_0 = await addNode(page, 'Dummy', 0);
    const node_1 = await addNode(page, 'Dummy', 1);
    const node_2 = await addNode(page, 'Dummy', 2);

    // Drag first node
    await dragElementByOffset(page, node_0, -300, 0);

    // Create third node
    await dragElementByOffset(page, node_2, 300, 0);

    const sourcePort0 = node_0.getByTestId('port-output-output_1');
    const targetPort1 = node_1.getByTestId('port-input-input_1');
    await connectElements(page, sourcePort0, targetPort1);
    const createdEdge_0 = page.getByTestId('edge-0');
    await expect(createdEdge_0).toBeVisible();

    const sourcePort1 = node_1.getByTestId('port-output-output_1');
    const targetPort2 = node_2.getByTestId('port-input-input_1');
    await connectElements(page, sourcePort1, targetPort2);
    const createdEdge_1 = page.getByTestId('edge-1');
    await expect(createdEdge_1).toBeVisible();

    // Activate first edge
    await createdEdge_0.focus(); // We have to use focus here, since click() would unfortinately activate thee text field on the edge.
    // Now press DELETE
    await page.keyboard.press('Delete');
    // edge_0 should disappear
    await expect(createdEdge_0).not.toBeVisible();
    // edge_1 should still be there
    await expect(createdEdge_1).toBeVisible();
  });
  test('should update connection distance and revert on negative or invalid input', async ({ page }) => {
    const node_0 = await addNode(page, 'Dummy', 0);
    const node_1 = await addNode(page, 'Dummy', 1);

    // Move first node to the left so the edge and its distance field are clearly accessible
    await dragElementByOffset(page, node_0, -300, 0);

    const sourcePort = node_0.getByTestId('port-output-output_1');
    const targetPort = node_1.getByTestId('port-input-input_1');
    await connectElements(page, sourcePort, targetPort);

    const createdEdge = page.getByTestId('edge-0');
    await expect(createdEdge).toBeVisible();

    // Locate the distance input inside the edge's foreignObject container
    const edgeInputContainer = page.getByTestId('edge-input-0');
    await expect(edgeInputContainer).toBeVisible();
    const distanceInput = edgeInputContainer.locator('input');
    await expect(distanceInput).toBeVisible();

    // 1. Set a valid positive distance and wait for backend confirmation (HTTP 204)
    const validPatchPromise = page.waitForResponse(
      (resp) => resp.url().includes('/connections') && resp.request().method() === 'PATCH'
    );
    await distanceInput.click();
    await distanceInput.fill('0.2 m');
    await distanceInput.press('Enter');

    const validResponse = await validPatchPromise;
    expect(validResponse.status()).toBe(204);

    // Capture the formatted valid value (e.g. "200 mm" or "0.2 m")
    const confirmedValidValue = await distanceInput.inputValue();
    expect(confirmedValidValue).not.toContain('-');

    // 2. Attempt to enter a negative distance; backend must reject with HTTP 400 and UI must revert
    const invalidPatchPromise = page.waitForResponse(
      (resp) => resp.url().includes('/connections') && resp.request().method() === 'PATCH'
    );
    await distanceInput.click();
    await distanceInput.fill('-0.05 m');
    await distanceInput.press('Enter');

    const invalidResponse = await invalidPatchPromise;
    expect(invalidResponse.status()).toBe(400);

    // Verify that the input field reverted to the previous valid distance
    await expect(distanceInput).toHaveValue(confirmedValidValue);

    // 3. Attempt to enter a malformed value without unit; frontend validation must revert immediately
    await distanceInput.click();
    await distanceInput.fill('42');
    await distanceInput.press('Enter');
    await expect(distanceInput).toHaveValue(confirmedValidValue);
  });
});