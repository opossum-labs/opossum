import { test, expect } from '@playwright/test';
import { dragElementByOffset, connectElements, addNode } from './helpers/editor_actions';

test.describe('Opossum checklist: 4. Groups', () => {

  test.beforeEach(async ({ page }) => {
    // Open the editor before every test
    await page.goto('/');
  });

  test.describe('Creating a group', () => {

    test('convert two connected nodes to a group, undo brings them back, redo creates the group again', async ({ page }) => {
      // Helpers to count all nodes and all connections on the screen
      const nodes = page.locator('[data-testid^="node-"]');
      const edges = page.locator('[data-testid^="edge-"]');

      // Add two nodes
      const node_0 = await addNode(page, 'Dummy', 0);
      const node_1 = await addNode(page, 'Dummy', 1);

      // Move the first node away, so the two nodes do not sit on top of each other
      await dragElementByOffset(page, node_0, -150, 0);

      // Connect the two nodes with a line
      const sourcePort = node_0.getByTestId('port-output-output_1');
      const targetPort = node_1.getByTestId('port-input-input_1');
      await connectElements(page, sourcePort, targetPort);
      await expect(nodes).toHaveCount(2);
      await expect(edges).toHaveCount(1);

      // Select both nodes by drawing a rectangle around them
      const box_0 = await node_0.boundingBox();
      const box_1 = await node_1.boundingBox();
      expect(box_0).not.toBeNull();
      expect(box_1).not.toBeNull();
      await page.mouse.move(box_0!.x - 30, box_0!.y - 30);
      await page.mouse.down();
      await page.mouse.move(box_1!.x + box_1!.width + 30, box_1!.y + box_1!.height + 30, { steps: 10 });
      await page.mouse.up();

      // Click the green "Convert to Group" button
      await page.getByRole('button', { name: /convert to group/i }).click();

      // The group appears and replaces the two nodes
      const group = nodes.filter({ hasText: 'new group' });
      await expect(group).toBeVisible();
      await expect(nodes).toHaveCount(1);

      // Open the group: the connection between the two nodes is still there
      await group.dblclick();
      await expect(nodes).toHaveCount(2);
      await expect(edges).toHaveCount(1);

      // Go back to the root tab and click the group so the editor listens to the keyboard
      await page.getByRole('tab', { name: 'unsaved' }).click();
      await group.click();

      // Press Ctrl+Z once: the two nodes and their line are back
      // (we count them, because restored nodes may get new names)
      await page.keyboard.press('Control+z');
      await expect(nodes).toHaveCount(2);
      await expect(edges).toHaveCount(1);
      await expect(group).not.toBeVisible();

      // Press Ctrl+Y once: the group is back
      await page.keyboard.press('Control+y');
      await expect(group).toBeVisible();
      await expect(nodes).toHaveCount(1);
    });
  });
});

  test.describe('Preserving existing mappings', () => {

    test('convert the inner nodes of a group to a subgroup, the group keeps its connection', async ({ page }) => {
      // Helpers to count all nodes and all connections on the screen
      const nodes = page.locator('[data-testid^="node-"]');
      const edges = page.locator('[data-testid^="edge-"]');

      // The green button that converts the selected nodes to a group
      const convertButton = page.getByRole('button', { name: /convert to group/i });

      // Add three nodes: node 0 and node 1 go into the group, node 2 stays outside
      const node_0 = await addNode(page, 'Dummy', 0);
      const node_1 = await addNode(page, 'Dummy', 1);
      const node_2 = await addNode(page, 'Dummy', 2);

      // New nodes lie on top of each other, so move them apart (the last one first)
      await dragElementByOffset(page, node_2, 450, 0);
      await dragElementByOffset(page, node_1, 150, 0);
      await dragElementByOffset(page, node_0, -150, 0);

      // Connect the nodes: node 0 -> node 1 -> node 2
      await connectElements(
        page,
        node_0.getByTestId('port-output-output_1'),
        node_1.getByTestId('port-input-input_1')
      );
      await connectElements(
        page,
        node_1.getByTestId('port-output-output_1'),
        node_2.getByTestId('port-input-input_1')
      );
      await expect(nodes).toHaveCount(3);
      await expect(edges).toHaveCount(2);

      // Find out where node 0 and node 1 are on the screen
      const box_0 = await node_0.boundingBox();
      const box_1 = await node_1.boundingBox();
      expect(box_0).not.toBeNull();
      expect(box_1).not.toBeNull();

      // Work out a rectangle around node 0 and node 1 only
      const left = box_0!.x - 30;
      const right = box_1!.x + box_1!.width + 30;
      const top = Math.min(box_0!.y, box_1!.y) - 30;
      const bottom = Math.max(box_0!.y + box_0!.height, box_1!.y + box_1!.height) + 30;

      // Draw the rectangle on the empty canvas with the mouse
      await page.mouse.move(left, top);
      await page.mouse.down();
      await page.mouse.move(right, bottom, { steps: 10 });
      await page.mouse.up();

      // Convert the two selected nodes to a group
      await expect(convertButton).toBeVisible();
      await convertButton.click();

      // Now there are two nodes on the screen (the group and the outside node)
      // and one line between them
      const group = nodes.filter({ hasText: 'new group' });
      await expect(group).toHaveCount(1);
      await expect(nodes).toHaveCount(2);
      await expect(edges).toHaveCount(1);

      // Open the group with a double-click
      await group.dblclick();

      // Inside the group we should see the two inner nodes
      // (only visible nodes, so the nodes of the main canvas are not counted)
      const inner = page.locator('[data-testid^="node-"]:visible');
      await expect(inner).toHaveCount(2);

      // Find out where the two inner nodes are on the screen
      const inner_0 = await inner.nth(0).boundingBox();
      const inner_1 = await inner.nth(1).boundingBox();
      expect(inner_0).not.toBeNull();
      expect(inner_1).not.toBeNull();

      // Work out a rectangle around both inner nodes
      const inner_left = Math.min(inner_0!.x, inner_1!.x) - 30;
      const inner_right = Math.max(inner_0!.x + inner_0!.width, inner_1!.x + inner_1!.width) + 30;
      const inner_top = Math.min(inner_0!.y, inner_1!.y) - 30;
      const inner_bottom = Math.max(inner_0!.y + inner_0!.height, inner_1!.y + inner_1!.height) + 30;

      // Draw the rectangle on the empty canvas with the mouse
      await page.mouse.move(inner_left, inner_top);
      await page.mouse.down();
      await page.mouse.move(inner_right, inner_bottom, { steps: 10 });
      await page.mouse.up();

      // Convert both inner nodes to a subgroup
      await expect(convertButton).toBeVisible();
      await convertButton.click();

      // Inside the group there is now only the subgroup
      await expect(inner).toHaveCount(1);
    });
  });

  test.describe('Drag and drop into a group', () => {

    test('drag a node onto a group, undo brings it back outside, redo puts it inside again', async ({ page }) => {
      // Helper to count all nodes on the screen
      const nodes = page.locator('[data-testid^="node-"]');

      // The green button that converts the selected nodes to a group
      const convertButton = page.getByRole('button', { name: /convert to group/i });

      // Add three nodes: node 0 and node 1 go into the group, node 2 stays outside
      const node_0 = await addNode(page, 'Dummy', 0);
      const node_1 = await addNode(page, 'Dummy', 1);
      const node_2 = await addNode(page, 'Dummy', 2);

      // New nodes lie on top of each other, so move them apart (the last one first)
      await dragElementByOffset(page, node_2, 450, 0);
      await dragElementByOffset(page, node_1, 150, 0);
      await dragElementByOffset(page, node_0, -150, 0);

      // Connect node 0 with node 1
      await connectElements(
        page,
        node_0.getByTestId('port-output-output_1'),
        node_1.getByTestId('port-input-input_1')
      );

      // Find out where node 0 and node 1 are on the screen
      const box_0 = await node_0.boundingBox();
      const box_1 = await node_1.boundingBox();
      expect(box_0).not.toBeNull();
      expect(box_1).not.toBeNull();

      // Work out a rectangle around node 0 and node 1 only
      const left = box_0!.x - 30;
      const right = box_1!.x + box_1!.width + 30;
      const top = Math.min(box_0!.y, box_1!.y) - 30;
      const bottom = Math.max(box_0!.y + box_0!.height, box_1!.y + box_1!.height) + 30;

      // Draw the rectangle on the empty canvas with the mouse
      await page.mouse.move(left, top);
      await page.mouse.down();
      await page.mouse.move(right, bottom, { steps: 10 });
      await page.mouse.up();

      // Convert the two selected nodes to a group
      await expect(convertButton).toBeVisible();
      await convertButton.click();

      // The group is the node called "new group", the node left outside is called "dummy"
      const group = nodes.filter({ hasText: 'new group' });
      const lone = nodes.filter({ hasText: 'dummy' });
      await expect(group).toHaveCount(1);
      await expect(lone).toHaveCount(1);
      await expect(nodes).toHaveCount(2);

      // Work out how far the outside node has to move to land in the middle of the group
      const group_box = await group.boundingBox();
      const lone_box = await lone.boundingBox();
      expect(group_box).not.toBeNull();
      expect(lone_box).not.toBeNull();
      const offset_x = (group_box!.x + group_box!.width / 2) - (lone_box!.x + lone_box!.width / 2);
      const offset_y = (group_box!.y + group_box!.height / 2) - (lone_box!.y + lone_box!.height / 2);

      // Drag the outside node onto the group
      await dragElementByOffset(page, lone, offset_x, offset_y);

      // The node is a member of the group now, so only the group is left on the screen
      await expect(nodes).toHaveCount(1);
      await expect(lone).toHaveCount(0);

      // Click the group so the editor listens to the keyboard
      await group.click();

      // Press Ctrl+Z only once: the node should be outside the group again
      await page.keyboard.press('Control+z');
      await expect(nodes).toHaveCount(2);
      await expect(lone).toHaveCount(1);

      // Press Ctrl+Y only once: the node should be inside the group again
      await page.keyboard.press('Control+y');
      await expect(nodes).toHaveCount(1);
      await expect(lone).toHaveCount(0);
    });
  });