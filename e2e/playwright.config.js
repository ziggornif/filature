const { defineConfig } = require('@playwright/test');
module.exports = defineConfig({
  testDir: './tests', timeout: 30_000, expect: { timeout: 8_000 }, fullyParallel: false, workers: 1,
  reporter: [['list'], ['html', { open: 'never', outputFolder: 'playwright-report' }]],
  use: { baseURL: process.env.FILATURE_E2E_BASE_URL || 'http://127.0.0.1:18081', trace: 'retain-on-failure' },
  projects: [{ name: 'light', use: { colorScheme: 'light' } }, { name: 'dark', use: { colorScheme: 'dark' } }],
});
