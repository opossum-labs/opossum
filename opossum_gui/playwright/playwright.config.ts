// Import Node.js path module to handle file system paths correctly
import path from 'path';
import { defineConfig, devices } from '@playwright/test';

// Define custom Playwright browsers directory allowed by AppLocker
process.env.PLAYWRIGHT_BROWSERS_PATH = 'C:\\Users\\ueisenb\\AppData\\Local\\0_gsi_executables\\ms-playwright';

export default defineConfig({
  // Directory where test artifacts (screenshots, videos, traces) will be saved
  outputDir: path.join(__dirname, 'test-results'),

  // Directory containing E2E test files
  testDir: './tests',
  
  // Maximum execution time for a single test (30 seconds)
  timeout: 30 * 1000,

  use: {
    // Base URL matching the explicit IP and port 8085 of the web server
    baseURL: 'http://127.0.0.1:8085',

    // Record trace files for failed test retries
    trace: 'on-first-retry',
  },

  // Configure projects to use Firefox instead of Chromium
  projects: [
    {
      name: 'firefox',
      use: { ...devices['Desktop Firefox'] },
    },
  ],

  // Automatically start your Dioxus web server before running tests
  webServer: {
    // Command to launch the Dioxus web development server on port 8085
    command: 'dx serve --platform web --port 8085 --addr 127.0.0.1',

    // Target URL that Playwright polls to verify the server is active
    url: 'http://127.0.0.1:8085',

    // Reuse a running server during local development
    reuseExistingServer: !process.env.CI,

    // Extended timeout (180s) to allow initial Rust WASM compilation
    timeout: 180 * 1000,

    // Pipe server stdout and stderr to the console to see Rust compile output/errors
    stdout: 'pipe',
    stderr: 'pipe',
  },
});