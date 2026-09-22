import { defineConfig, devices } from '@playwright/test';

const HOST = '127.0.0.1';
const PORT = 4173;

export default defineConfig({
  testDir: './tests',
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? 'list' : 'html',
  use: { baseURL: `http://${HOST}:${PORT}` },
  projects: [{ name: 'chromium', use: devices['Desktop Chrome'] }],
  // Geprueft wird der fertige Build, nicht der Dev-Server: ausgeliefert
  // wird schliesslich der Build.
  webServer: {
    // Host ausdruecklich: sonst bindet Vite unter Windows nur an ::1,
    // und Playwright wartet vergeblich auf 127.0.0.1.
    command: `npm run build && npm run preview -- --host ${HOST} --port ${PORT} --strictPort`,
    url: `http://${HOST}:${PORT}`,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
