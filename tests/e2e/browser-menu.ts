import type { Page } from "@playwright/test";

export const browserMenuTrigger = (page: Page) => page.getByRole("button", { name: "浏览器菜单", exact: true });

export async function openBrowserMenu(page: Page) {
  await browserMenuTrigger(page).click();
  return page.getByRole("menu", { name: "浏览器菜单", exact: true });
}

export async function chooseBrowserMenuItem(page: Page, name: string) {
  const menu = await openBrowserMenu(page);
  await menu.getByRole("menuitem", { name, exact: true }).click();
}
