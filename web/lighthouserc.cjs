// Lighthouse gegen den fertigen Build, Kacheln als dist/tiles daneben.
// Schwellen und Begründung: docs/entwicklung/ci.md, „Lighthouse“.
const PORT = 4174;

module.exports = {
  ci: {
    collect: {
      url: [`http://127.0.0.1:${PORT}/`],
      startServerCommand: `npx vite preview --host 127.0.0.1 --port ${PORT} --strictPort`,
      startServerReadyPattern: 'Local',
      numberOfRuns: 10,
      settings: {
        chromeFlags: '--headless=new --no-sandbox --disable-dev-shm-usage',
        onlyCategories: ['performance', 'accessibility', 'best-practices', 'seo'],
      },
    },
    assert: {
      assertions: {
        'categories:seo': ['error', { minScore: 0.9, aggregationMethod: 'median' }],
        'categories:accessibility': ['error', { minScore: 0.9, aggregationMethod: 'median' }],
        'categories:best-practices': ['error', { minScore: 0.9, aggregationMethod: 'median' }],
        'cumulative-layout-shift': ['error', { maxNumericValue: 0.1, aggregationMethod: 'median' }],
        'total-blocking-time': ['error', { maxNumericValue: 300, aggregationMethod: 'median' }],
        'categories:performance': ['warn', { minScore: 0.5, aggregationMethod: 'median' }],
        'largest-contentful-paint': ['warn', { maxNumericValue: 4000, aggregationMethod: 'median' }],
        'document-title': 'error',
        'meta-description': 'error',
        'http-status-code': 'error',
        'is-crawlable': 'error',
        'robots-txt': 'error',
      },
    },
    upload: { target: 'filesystem', outputDir: 'lighthouse' },
  },
};
