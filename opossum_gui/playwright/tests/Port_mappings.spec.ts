import { test, expect } from '@playwright/test';
import { dragElementByOffset, addNode } from './helpers/editor_actions';

test.describe('Opossum checklist: 3. Port mappings', () => {

  test.beforeEach(async ({ page }) => {
    // Open the editor before every test
    await page.goto('/');
  });

  test('map input and output port to the group, undo removes the handles one by one and redo brings them back', async ({ page }) => {
    // Add two nodes and put them next to each other
    const node_0 = await addNode(page, 'Dummy', 0);
    const node_1 = await addNode(page, 'Dummy', 1);
    await dragElementByOffset(page, node_0, -300, 0);

    // TODO: replace this block by the real way to convert both nodes into a group
    await node_0.click();
    await node_1.click({ modifiers: ['Control'] });
    await page.getByRole('button', { name: 'Node', exact: true }).click();
    await page.getByRole('button', { name: 'Convert to Group' }).click();

    // In the root tab there is now only the group node
    const group = page.locator('[data-testid^="node-"]');
    await expect(group).toHaveCount(1);
    const handles = group.locator('[data-testid^="port-"]');
    const handlesBefore = await handles.count();

    // Double click opens the group in the tab "new group"
    await group.dblclick();
    await expect(page.getByRole('tab', { name: 'new group' })).toBeVisible(); // TODO: check if it is really role "tab"

    // Right click the free input port of the first node and map it
    await node_0.getByTestId('port-input-input_1').click({ button: 'right' });
    await page.getByRole('menuitem', { name: 'Map Port to Group' }).click(); // TODO: menuitem or button?

    // Right click the free output port of the second node and map it
    await node_1.getByTestId('port-output-output_1').click({ button: 'right' });
    await page.getByRole('menuitem', { name: 'Map Port to Group' }).click();

    // Go back to the root tab: the group node shows two new handles
    await page.getByRole('tab', { name: 'unsaved' }).click();
    await expect(handles).toHaveCount(handlesBefore + 2);

    // Press Ctrl+Z: only the last added handle disappears
    await page.keyboard.press('Control+z');
    await expect(handles).toHaveCount(handlesBefore + 1);

    // Press Ctrl+Z again: the other handle disappears
    await page.keyboard.press('Control+z');
    await expect(handles).toHaveCount(handlesBefore);

    // Press Ctrl+Y twice: both handles come back one by one
    await page.keyboard.press('Control+y');
    await expect(handles).toHaveCount(handlesBefore + 1);
    await page.keyboard.press('Control+y');
    await expect(handles).toHaveCount(handlesBefore + 2);
  });
});