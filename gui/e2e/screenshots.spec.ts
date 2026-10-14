import { expect, test } from "@playwright/test";

test("main window", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("strip-music")).toBeVisible();
  await page.waitForTimeout(400);
  await page.screenshot({ path: "screenshots/main.png" });
});

test("disconnected", async ({ page }) => {
  await page.goto("/?disconnected=1");
  await expect(page.getByText("disconnected")).toBeVisible();
  await page.screenshot({ path: "screenshots/disconnected.png" });
});
