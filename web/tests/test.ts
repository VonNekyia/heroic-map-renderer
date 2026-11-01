import { expect, test as basis } from '@playwright/test';

export type { Browser, CDPSession, Locator, Page } from '@playwright/test';
export { expect };

/** Playwrights `test`, nur fällt jeder Test, in dessen `page` ein Fehler unbehandelt bleibt. */
export const test = basis.extend({
  page: async ({ page }, use) => {
    const fehler: string[] = [];
    page.on('pageerror', (e) => fehler.push(e.message));
    await use(page);
    expect(fehler).toEqual([]);
  },
});
