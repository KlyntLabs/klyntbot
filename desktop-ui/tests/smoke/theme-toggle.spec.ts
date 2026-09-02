import { expect, test } from "@playwright/test";

test("theme toggle light/dark via personalization settings", async ({ page }) => {
  await page.goto("/");
  await page.goto("/#/settings/personalization");

  await page.getByRole("button", { name: "Light" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");

  await page.getByRole("button", { name: "Dark" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
});
