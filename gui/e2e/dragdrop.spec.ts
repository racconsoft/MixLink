import { expect, test } from "@playwright/test";

test("dragging an app icon onto another strip assigns it there", async ({ page }) => {
  await page.goto("/");
  const firefox = page.getByTestId("strip-browser").getByTitle("Firefox", { exact: true });
  const game = page.getByTestId("strip-game");
  await expect(firefox).toBeVisible();
  await expect(game.getByTitle("Firefox", { exact: true })).toHaveCount(0);
  await firefox.dragTo(game);
  await expect(game.getByTitle("Firefox", { exact: true })).toBeVisible();
  await expect(page.getByTestId("strip-browser").getByTitle("Firefox", { exact: true })).toHaveCount(0);
});
