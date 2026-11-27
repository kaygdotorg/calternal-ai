/**
 * Reading List and Tasks E2E verification helpers invoked by `host-chat.mjs`
 * against the same running `calternal-host` serving Chat, Reading List, and Tasks.
 */
import assert from "node:assert/strict";
import path from "node:path";

/** Drive Reading List and Tasks through the real `/host` UI and capture 1280 + 390 light/dark shots. */
export async function verifyReadingListAndTasks(a, phone, origin, shotDir) {
  await a.page.waitForSelector('[data-switcher] [data-app="calternal-reading-list"]');
  await a.page.waitForSelector('[data-switcher] [data-app="calternal-tasks"]');

  // Ensure light mode on desktop
  await a.page.evaluate(() => document.documentElement.setAttribute("data-scheme", "light"));

  // --- Reading List: book icon, add link, computed `site`, item page, `Mark read`, board, search ---
  await a.page.click('[data-switcher] [data-app="calternal-reading-list"]');
  await a.page.waitForSelector('[data-cid="calternal-reading-list.unread"] [data-icon="book"]');
  await a.page.click('[data-canvas] button:has-text("New Reading")');
  await a.page.fill('input[name="url"]', "https://example.org/notes/layout");
  await a.page.fill('input[name="title"]', "Layout study");
  await a.page.click('button:has-text("Create Reading")');

  await a.page.waitForSelector('[data-canvas] .row-wrap:has-text("Layout study"):has-text("example.org")');
  await a.page.waitForTimeout(300);
  await a.page.screenshot({ path: path.join(shotDir, "reading-1280-light.png") });
  await a.page.evaluate(() => document.documentElement.setAttribute("data-scheme", "dark"));
  await a.page.waitForTimeout(250);
  await a.page.screenshot({ path: path.join(shotDir, "reading-1280-dark.png") });
  await a.page.evaluate(() => document.documentElement.setAttribute("data-scheme", "light"));

  // Open item page (must render ItemPageShell, NOT records composer)
  await a.page.click('[data-canvas] .row-wrap button:has-text("Layout study")');
  await a.page.waitForSelector('[data-item-page] [data-field="site"]:has-text("example.org")');
  assert.equal(await a.page.locator('form[aria-label="Composer"]').count(), 0, "Reading item must not mount composer");

  // Mark read -> action disappears (`row.when` flips to false)
  await a.page.click('[data-item-page] [data-primary-op="mark read"]');
  await a.page.waitForSelector('[data-item-page] [data-primary-op="mark read"]', { state: "detached" });

  // Switch to All -> Board view -> verify item in `Yes` lane (`data-col="true"`) + search filter
  await a.page.click('[data-sidebar-body] [data-cid="calternal-reading-list.all"] .tog');
  await a.page.waitForSelector('[data-view-tab="board"]');
  await a.page.click('[data-view-tab="board"]');
  await a.page.waitForSelector('[data-board] [data-col="true"] :text("Layout study")');
  await a.page.fill('input[aria-label="Search All"]', "example.org");
  await a.page.waitForSelector('[data-board] [data-col="true"] :text("Layout study")');
  await a.page.fill('input[aria-label="Search All"]', "no-such-domain");
  await a.page.waitForSelector('[data-board] [data-col="true"] :text("Layout study")', { state: "detached" });
  await a.page.fill('input[aria-label="Search All"]', "");

  // --- Tasks: typed form, grouped list, totals, board lane change on Done, second task total update ---
  await a.page.click('[data-switcher] [data-app="calternal-tasks"]');
  await a.page.waitForSelector('[data-cid="calternal-tasks.open"] [data-icon="check"]');
  await a.page.click('[data-canvas] button:has-text("New Task")');
  await a.page.fill('input[name="title"]', "Ship multi-app client");
  await a.page.fill('input[name="due"]', "2026-12-01");
  await a.page.selectOption('select[name="priority"]', "high");
  await a.page.fill('input[name="tags"]', "web, ui");
  await a.page.click('button:has-text("Create Task")');

  await a.page.click('[data-sidebar-body] [data-cid="calternal-tasks.all"] .tog');
  await a.page.waitForSelector('[data-collection-totals]:has-text("1")');
  await a.page.waitForSelector('[data-group="todo"] :text("Ship multi-app client")');
  await a.page.waitForTimeout(300);
  await a.page.screenshot({ path: path.join(shotDir, "tasks-list-1280-light.png") });
  await a.page.evaluate(() => document.documentElement.setAttribute("data-scheme", "dark"));
  await a.page.waitForTimeout(250);
  await a.page.screenshot({ path: path.join(shotDir, "tasks-list-1280-dark.png") });
  await a.page.evaluate(() => document.documentElement.setAttribute("data-scheme", "light"));

  // Switch to Board -> complete task via `Done` -> see lane change `todo` -> `done`
  await a.page.click('[data-view-tab="board"]');
  await a.page.waitForSelector('[data-board] [data-col="todo"] :text("Ship multi-app client")');
  await a.page.click('[data-board] [data-col="todo"] [data-primary-op="done"]');
  await a.page.waitForSelector('[data-board] [data-col="done"] :text("Ship multi-app client")');
  assert.equal(await a.page.locator('[data-board] [data-col="todo"] :text("Ship multi-app client")').count(), 0);

  // Add second task -> verify live total updates from 1 to 2
  await a.page.click('[data-canvas] button:has-text("New Task")');
  await a.page.fill('input[name="title"]', "Review PR");
  await a.page.click('button:has-text("Create Task")');
  await a.page.waitForSelector('[data-collection-totals]:has-text("2")');
  await a.page.waitForSelector('[data-board] [data-col="todo"] :text("Review PR")');
  await a.page.waitForTimeout(300);
  await a.page.screenshot({ path: path.join(shotDir, "tasks-board-1280-light.png") });
  await a.page.evaluate(() => document.documentElement.setAttribute("data-scheme", "dark"));
  await a.page.waitForTimeout(250);
  await a.page.screenshot({ path: path.join(shotDir, "tasks-board-1280-dark.png") });
  await a.page.evaluate(() => document.documentElement.setAttribute("data-scheme", "light"));

  // --- Phone 390 light & dark screenshots for Reading List & Tasks ---
  await phone.page.evaluate(() => sessionStorage.clear());
  await phone.page.goto(`${origin}/host?scheme=light`);
  await phone.page.waitForSelector('[data-switcher] [data-app="calternal-reading-list"]');
  await phone.page.click('[data-switcher] [data-app="calternal-reading-list"]');
  await phone.page.waitForSelector('[data-sidebar-body] [data-cid="calternal-reading-list.all"]');
  await phone.page.click('[data-sidebar-body] [data-cid="calternal-reading-list.all"] .tog');
  await phone.page.waitForSelector('[data-open-title]:has-text("All")');
  await phone.page.waitForTimeout(350);
  await phone.page.screenshot({ path: path.join(shotDir, "reading-390-light.png") });
  await phone.page.evaluate(() => document.documentElement.setAttribute("data-scheme", "dark"));
  await phone.page.waitForTimeout(250);
  await phone.page.screenshot({ path: path.join(shotDir, "reading-390-dark.png") });

  await phone.page.click('button[aria-label="Back to the sidebar"]');
  await phone.page.waitForSelector('[data-switcher] [data-app="calternal-tasks"]');
  await phone.page.click('[data-switcher] [data-app="calternal-tasks"]');
  await phone.page.waitForSelector('[data-sidebar-body] [data-cid="calternal-tasks.all"]');
  await phone.page.click('[data-sidebar-body] [data-cid="calternal-tasks.all"] .tog');
  await phone.page.waitForSelector('[data-view-tab="board"]');
  await phone.page.click('[data-view-tab="board"]');
  await phone.page.waitForTimeout(350);
  await phone.page.screenshot({ path: path.join(shotDir, "tasks-390-dark.png") });
  await phone.page.evaluate(() => document.documentElement.setAttribute("data-scheme", "light"));
  await phone.page.waitForTimeout(250);
  await phone.page.screenshot({ path: path.join(shotDir, "tasks-390-light.png") });

  // Return desktop to Chat for the host-restart check
  await a.page.click('[data-switcher] [data-app="calternal-chat"]');
  await a.page.waitForSelector('[data-sidebar-body] [data-cid="calternal-chat.chats"] button:has-text("Release prep v2")');
  await a.page.click('[data-sidebar-body] [data-cid="calternal-chat.chats"] button:has-text("Release prep v2") >> nth=0');
  await a.page.waitForSelector('[data-open-title]:has-text("Release prep v2")');
}
