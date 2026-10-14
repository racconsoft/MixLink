import { expect, test } from "@playwright/test";

test("main window", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("strip-music")).toBeVisible();
  await page.waitForTimeout(400);
  await page.screenshot({ path: "screenshots/main.png" });
});

test.describe("small viewport", () => {
  test.use({ viewport: { width: 900, height: 560 } });

  test("main window at small window size", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("strip-music")).toBeVisible();
    await page.waitForTimeout(400);
    await page.screenshot({ path: "screenshots/main-small.png" });
  });
});

test("disconnected", async ({ page }) => {
  await page.goto("/?disconnected=1");
  await expect(page.getByText("MixLink service is not running")).toBeVisible();
  await page.screenshot({ path: "screenshots/disconnected.png" });
});

test("settings and warning", async ({ page }) => {
  await page.goto("/?warning=1");
  await expect(page.getByRole("alert")).toBeVisible();
  await page.getByTitle("Settings").click();
  await page.screenshot({ path: "screenshots/settings.png" });
});

test("channel dialog and app picker", async ({ page }) => {
  await page.goto("/");
  await page.getByTitle("Add channel").click();
  await page.screenshot({ path: "screenshots/channel-dialog.png" });
  await page.getByText("Cancel").click();
  await page.getByTestId("strip-browser").getByTitle("Assign apps").click();
  await page.screenshot({ path: "screenshots/app-picker.png" });
});

test("app picker with many apps is not clipped", async ({ page }) => {
  await page.goto("/?manyapps=1");
  await page.getByTestId("strip-browser").getByTitle("Assign apps").click();
  await expect(page.getByRole("dialog", { name: "Assign apps" })).toBeVisible();
  await page.screenshot({ path: "screenshots/app-picker-many.png" });
});
