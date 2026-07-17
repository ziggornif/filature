const { test, expect } = require('@playwright/test');
const AxeBuilder = require('@axe-core/playwright').default;
const fs = require('node:fs');
const path = require('node:path');
const username = process.env.FILATURE_E2E_USERNAME || 'a11y';
const password = process.env.FILATURE_E2E_PASSWORD || 'filature-a11y';
const demoBackup = path.resolve(__dirname, '../fixtures/demo-instance.json');

async function seed(page) {
  await page.goto('/login');
  await page.locator('#username').fill(username);
  await page.locator('#password').fill(password);
  await Promise.all([page.waitForURL((u) => u.pathname === '/'), page.locator('button[type=submit]').click()]);
  const response = await page.request.post('/settings/import', { multipart: {
    backup: { name: 'demo-instance.json', mimeType: 'application/json', buffer: fs.readFileSync(demoBackup) },
    confirm_replace: 'yes',
  }});
  expect(response.ok(), `demo import failed: ${response.status()}`).toBeTruthy();
}

async function routes(page) {
  await page.goto('/spools');
  const spool = await page.locator('tbody a[href^="/spools/"]:not([href$="/edit"])').first().getAttribute('href');
  await page.goto('/printers');
  const printer = await page.locator('a[href^="/printers/"][href$="/edit"]').first().getAttribute('href');
  expect(spool, 'demo backup must contain a spool').toBeTruthy();
  expect(printer, 'demo backup must contain a printer').toBeTruthy();
  return [['dashboard','/'],['spools','/spools'],['spool-detail',spool],['spool-new','/spools/new'],['printers','/printers'],['printer-edit',printer],['materials','/materials'],['settings','/settings'],['locations','/settings/locations'],['manufacturers','/settings/manufacturers']];
}

function report(vs) { return vs.map((v) => `${v.id} [${v.impact}] ${v.help}\n${v.nodes.map((n) => `  - ${n.target.join(' ')}: ${n.failureSummary}`).join('\n')}`).join('\n\n'); }
test.beforeEach(async ({ page }) => seed(page));

test('axe — aucun écart WCAG A/AA critical ou serious hors contraste', async ({ page }) => {
  for (const [name, route] of await routes(page)) {
    await page.goto(route);
    const result = await new AxeBuilder({ page }).withTags(['wcag2a','wcag2aa','wcag21a','wcag21aa']).disableRules(['color-contrast']).analyze();
    const violations = result.violations.filter((v) => ['critical','serious'].includes(v.impact));
    expect(violations, `${name} (${route})\n${report(violations)}`).toEqual([]);
  }
});

test('B1 — tous les champs éditables ont un nom accessible, y compris après swap htmx', async ({ page }) => {
  for (const route of ['/materials','/settings/locations','/settings/manufacturers']) {
    await page.goto(route);
    const result = await new AxeBuilder({ page }).withRules(['label']).analyze();
    expect(result.violations, `${route}\n${report(result.violations)}`).toEqual([]);
  }
  await page.goto('/spools/new/details?condition=new');
  await expect(page.locator('input[type=color]')).toHaveAccessibleName(/.+/);
  await page.goto('/materials');
  const input = page.locator('#materials-table-body input[name=density]').first();
  await input.fill('1.25');
  await Promise.all([page.waitForResponse((r) => r.request().method() === 'PUT' && /\/materials\//.test(r.url())), input.press('Tab')]);
  await expect(page.locator('#materials-table-body input[name=density]').first()).toHaveAccessibleName(/.+/);
});

test('B2 — le sélecteur de sensibilité matériau a un nom accessible', async ({ page }) => {
  await page.goto('/materials');
  for (const select of await page.locator('select[name=sensitivity]').all()) await expect(select).toHaveAccessibleName(/.+/);
});

test('B3 — les contrôles icône imprimante ont un nom accessible, y compris après swap htmx', async ({ page }) => {
  // Un nom accessible réel contient des lettres : le glyphe seul (« ✎ »/« ✕ »)
  // ne suffit pas, sinon l'écart B3 (nom = icône) passerait inaperçu.
  const realName = /\p{L}{2,}/u;
  await page.goto('/printers');
  await expect(page.locator('a.printer-edit').first()).toHaveAccessibleName(realName);
  const unload = page.locator('form.slot-unload button').first();
  await expect(unload).toHaveAccessibleName(realName);
  await Promise.all([page.waitForResponse((r) => r.request().method() === 'POST' && /\/printers\/.*\/slots\//.test(r.url())), unload.click()]);
  await expect(page.locator('a.printer-edit').first()).toHaveAccessibleName(realName);
});

test('M2 — le compteur filtré des bobines est une live-region mise à jour', async ({ page }) => {
  await page.goto('/spools');
  const status = page.locator('.spools-count');
  await expect(status).toHaveAttribute('role','status');
  await expect(status).toHaveAttribute('aria-live','polite');
  const before = (await page.locator('#spools-filtered-count').textContent()).trim();
  // Le filtre est déclenché par `keyup` (htmx) ; `fill()` n'émet pas de keyup,
  // il faut donc taper caractère par caractère et attendre le swap OOB du compteur.
  await Promise.all([
    page.waitForResponse((r) => r.request().method() === 'GET' && /\/spools\/rows/.test(r.url())),
    page.locator('#spool-search').pressSequentially('__no_spool_matches__', { delay: 20 }),
  ]);
  await expect(page.locator('#spools-filtered-count')).not.toHaveText(before);
});

test('M3 — le détail bobine possède un h1 portant son identité', async ({ page }) => {
  const all = await routes(page); await page.goto(all.find(([n]) => n === 'spool-detail')[1]);
  await expect(page.locator('main h1')).toHaveCount(1); await expect(page.locator('main h1')).not.toHaveText('');
});

test('M4 — tous les en-têtes de colonne de données ont scope=col', async ({ page }) => {
  for (const route of ['/spools','/materials','/settings/locations','/settings/manufacturers']) {
    await page.goto(route); const headers = page.locator('table thead th');
    expect(await headers.count(), `${route} must have headers`).toBeGreaterThan(0);
    for (const header of await headers.all()) await expect(header).toHaveAttribute('scope','col');
  }
});

test('m1 — le lien d’évitement cible et focalise le contenu principal', async ({ page }) => {
  await page.goto('/'); const skip = page.locator('body > a.skip-link');
  await expect(skip).toHaveAttribute('href','#main'); await skip.focus(); await expect(skip).toBeVisible(); await skip.press('Enter'); await expect(page.locator('#main')).toBeFocused();
});

test('M1/M5 — contraste WCAG AA (advisory)', async ({ page }, testInfo) => {
  const reports = [];
  for (const [name, route] of await routes(page)) { await page.goto(route); const result = await new AxeBuilder({ page }).withRules(['color-contrast']).analyze(); if (result.violations.length) reports.push(`${name} (${route})\n${report(result.violations)}`); }
  const body = reports.join('\n\n');
  await testInfo.attach('color-contrast-advisory.txt', { body: body || 'No color-contrast violations.', contentType: 'text/plain' });
  if (body) console.warn(`\nAccessibility contrast advisory:\n${body}`);
  if (process.env.A11Y_ENFORCE_CONTRAST === '1') expect(reports, body).toEqual([]);
});
