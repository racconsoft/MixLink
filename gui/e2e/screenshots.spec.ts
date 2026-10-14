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

test("channel dialog and app picker", async ({ page }) => {
  await page.goto("/");
  await page.getByTitle("Add channel").click();
  await page.screenshot({ path: "screenshots/channel-dialog.png" });
  await page.getByText("Cancel").click();
  await page.getByTestId("strip-browser").getByTitle("Assign apps").click();
  await page.screenshot({ path: "screenshots/app-picker.png" });
});
