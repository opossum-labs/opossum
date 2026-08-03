import { Page, Locator, expect } from '@playwright/test';

/**
 * Drags a DOM element by a relative pixel offset (deltaX, deltaY).
 *
 * @param page - The active Playwright Page object.
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
  steps: number = 10
): Promise<void> {
  // Ensure the target element is rendered and visible
  await expect(element).toBeVisible();

  // Retrieve bounding box to calculate current position
  const box = await element.boundingBox();
  if (!box) {
    throw new Error('Failed to retrieve bounding box for the target element.');
  }

  // Calculate center coordinates of the element
  const startX = box.x + box.width / 2;
  const startY = box.y + box.height / 2;

  // Calculate target position
  const targetX = startX + deltaX;
  const targetY = startY + deltaY;

  // Execute the mouse interaction sequence
  await page.mouse.move(startX, startY);
  await page.mouse.down();
  await page.mouse.move(targetX, targetY, { steps });
  await page.mouse.up();
}

/**
 * Performs a drag and drop action from a source element to a target element.
 * Useful for connecting ports or dropping items into target drop zones.
 *
 * @param page - The active Playwright Page object.
 * @param sourceElement - Locator for the element where the drag starts (e.g., output port).
 * @param targetElement - Locator for the element where the drag ends (e.g., input port).
 * @param steps - Number of intermediate mousemove events (default: 15) for smooth dragging.
 */
export async function connectElements(
  page: Page,
  sourceElement: Locator,
  targetElement: Locator,
  steps: number = 15
): Promise<void> {
  // Verify visibility of both components
  await expect(sourceElement).toBeVisible();
  await expect(targetElement).toBeVisible();

  // Fetch bounding boxes for both elements
  const sourceBox = await sourceElement.boundingBox();
  const targetBox = await targetElement.boundingBox();

  if (!sourceBox || !targetBox) {
    throw new Error('Failed to retrieve bounding box for source or target element.');
  }

  // Compute exact center point for start and destination
  const fromX = sourceBox.x + sourceBox.width / 2;
  const fromY = sourceBox.y + sourceBox.height / 2;

  const toX = targetBox.x + targetBox.width / 2;
  const toY = targetBox.y + targetBox.height / 2;

  // Execute connection drag sequence
  await page.mouse.move(fromX, fromY);
  await page.mouse.down();
  await page.mouse.move(toX, toY, { steps });
  await page.mouse.up();
}
