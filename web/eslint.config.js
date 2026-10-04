import js from '@eslint/js';
import globals from 'globals';
import tseslint from 'typescript-eslint';

/** Die Grenze zwischen Grundkarte und Skins, siehe docs/frontend.md, „Skins“. */
const GRENZE = 'Ein Skin importiert nur aus seinem Ordner, Leaflet und heroic-map-renderer/skin-api.';

export default tseslint.config(
  { ignores: ['dist', 'dist-skin', 'public', 'test-results', 'playwright-report', 'lighthouse', '.lighthouseci'] },
  js.configs.recommended,
  tseslint.configs.recommendedTypeChecked,
  {
    languageOptions: {
      globals: globals.browser,
      parserOptions: {
        // Die Konfigurationsdateien selbst stehen in keinem tsconfig.
        projectService: { allowDefaultProject: ['eslint.config.js', 'lighthouserc.cjs'] },
        tsconfigRootDir: import.meta.dirname,
      },
    },
  },
  {
    files: ['tests/**', 'skins/*/tests/**', 'playwright.config.ts', 'vite.config.ts', 'lighthouserc.cjs'],
    languageOptions: { globals: globals.node },
  },
  {
    // Die Grundkarte lädt einen Skin nur in main.ts, per import().
    files: ['src/**', 'tests/**'],
    rules: {
      'no-restricted-imports': [
        'error',
        {
          paths: [{ name: 'virtual:skin', message: 'Den Skin lädt main.ts nur per import().' }],
          patterns: [{ regex: '(^|/)skins/', message: 'Die Grundkarte importiert keinen Skin.' }],
        },
      ],
    },
  },
  {
    files: ['skins/*/*.ts'],
    rules: {
      'no-restricted-imports': [
        'error',
        { patterns: [{ regex: '^(?!leaflet$|heroic-map-renderer/skin-api$|\\./)', message: GRENZE }] },
      ],
    },
  },
  {
    // Die Tests eines Skins dazu Playwright, Node und die Kameras der
    // Grundkarte aus web/tests/kamera.ts.
    files: ['skins/*/tests/**'],
    rules: {
      'no-restricted-imports': [
        'error',
        {
          patterns: [
            {
              regex:
                '^(?!leaflet$|heroic-map-renderer/skin-api$|\\./|\\.\\./[^.]|@playwright/test$|node:|\\.\\./\\.\\./\\.\\./tests/kamera$)',
              message: GRENZE,
            },
          ],
        },
      ],
    },
  },
);
