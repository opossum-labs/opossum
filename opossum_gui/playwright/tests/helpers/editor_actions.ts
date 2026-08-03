import { Page, Locator, expect } from '@playwright/test';

/**
 * Drags a DOM element by a relative pixel offset (deltaX, deltaY).
 *
 * @param page - The active Playwright Page instance.
 * @param element - The Playwright Locator pointing to the element to drag.
 * @param deltaX - The horizontal distance to move in pixels (positive = right, negative = left).
 * @param deltaY - The vertical distance to move in pixels (positive = down, negative = up).
 * @param steps - Number of intermediate mousemove events (default: 10) to trigger drag handlers.
 */
export async function dragElementByOffset(
  page: Page,
  element: Locator,
  deltaX: number,
  deltaY: number,
  steps: number = 5
): Promise<void> {
  await expect(element).toBeVisible();

  const box = await element.boundingBox();
  if (!box) {
    throw new Error('Failed to retrieve bounding box for the target element.');
  }

  const startX = box.x + box.width / 2;
  const startY = box.y + box.height / 2;

  const targetX = startX + deltaX;
  const targetY = startY + deltaY;

  await page.mouse.move(startX, startY);
  await page.mouse.down();
  await page.mouse.move(targetX, targetY, { steps });
  await page.mouse.up();
}

/**
 * Performs a drag-and-drop connection between a source port and a target port.
 *
 * @param page - The active Playwright Page instance.
 * @param sourceElement - Locator for the drag origin (e.g., output port).
 * @param targetElement - Locator for the drag destination (e.g., input port).
 * @param steps - Number of intermediate mousemove events (default: 15) for smooth dragging.
 */
export async function connectElements(
  page: Page,
  sourceElement: Locator,
  targetElement: Locator,
  steps: number = 5
): Promise<void> {
  await expect(sourceElement).toBeVisible();
  await expect(targetElement).toBeVisible();

  const sourceBox = await sourceElement.boundingBox();
  const targetBox = await targetElement.boundingBox();

  if (!sourceBox || !targetBox) {
    throw new Error('Failed to retrieve bounding box for source or target element.');
  }

  const fromX = sourceBox.x + sourceBox.width / 2;
  const fromY = sourceBox.y + sourceBox.height / 2;

  const toX = targetBox.x + targetBox.width / 2;
  const toY = targetBox.y + targetBox.height / 2;

  await page.mouse.move(fromX, fromY);
  await page.mouse.down();
  await page.mouse.move(toX, toY, { steps });
  await page.mouse.up();
}

/**
 * Deletes an edge element by directly focusing its SVG path and pressing the 'Delete' key.
 * Bypasses Playwright's actionability checks when foreignObject overlays are present.
 *
 * @param page - The active Playwright Page instance.
 * @param edgeElement - Locator for the SVG path edge element.
 */
export async function deleteEdge(
  page: Page,
  edgeElement: Locator
): Promise<void> {
  await expect(edgeElement).toBeVisible();
  await edgeElement.focus();
  await page.keyboard.press('Delete');
  await expect(edgeElement).not.toBeVisible();
}

/**
 * Adds a new optical node to the scenery editor workspace via the Edit menu.
 *
 * @param page - The active Playwright Page instance.
 * @param nodeType - The type of node to create (e.g., 'Dummy', 'Lens', 'Mirror'). Defaults to 'Dummy'.
 * @param expectedIndex - Optional expected node index (e.g., 0 for "node-0") to return its specific Locator.
 * @returns The Playwright Locator pointing to the newly created node.
 */
export async function addNode(
  page: Page,
  nodeType: string = 'Dummy',
  expectedIndex?: number
): Promise<Locator> {
  // 1. Open the Edit menu
  const editMenu = page.getByRole('button', { name: 'Edit' });
  await expect(editMenu).toBeVisible();
  await editMenu.click();

  // 2. Click the 'Add Node' menu item
  const addNodeOption = page.getByRole('button', { name: 'Add Node' });
  await expect(addNodeOption).toBeVisible();
  await addNodeOption.click();

  // 3. Select the requested node type option
  const nodeOption = page.getByRole('button', { name: nodeType });
  await expect(nodeOption).toBeVisible();
  await nodeOption.click();

  // 4. Return locator if an index is specified, otherwise return the last node found in DOM
  if (expectedIndex !== undefined) {
    const createdNode = page.getByTestId(`node-${expectedIndex}`);
    await expect(createdNode).toBeVisible();
    return createdNode;
  }

  const fallbackNode = page.locator('[data-testid^="node-"]').last();
  await expect(fallbackNode).toBeVisible();
  return fallbackNode;
}