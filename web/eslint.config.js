import js from '@eslint/js';
import globals from 'globals';
import tseslint from 'typescript-eslint';

/** Die Grenze zwischen Grundkarte und Skins, siehe docs/frontend.md, „Skins“. */
const GRENZE = 'Ein Skin importiert nur aus seinem Ordner, Leaflet und heroic-map-renderer/skin-api.';

/**
 * Zwei JSDoc-Blöcke direkt hintereinander, ohne Leerzeile: Der erste hängt
 * an nichts, denn TypeScript nimmt den letzten. Etwa wenn eine neue
 * Konstante zwischen ein JSDoc und seine Funktion rutscht. Ein Kopf der Datei
 * mit Leerzeile danach zählt nicht.
 *
 * @type {import('eslint').Rule.RuleModule}
 */
const jsdocDoppelt = {
  meta: { type: 'problem', messages: { doppelt: 'Zwei JSDoc-Blöcke hintereinander: Der erste hängt an nichts.' } },
  create(context) {
    const quelle = context.sourceCode;
    return {
      Program() {
        const bloecke = quelle.getAllComments().filter((k) => k.type === 'Block' && k.value.startsWith('*'));
        for (let i = 1; i < bloecke.length; i++) {
          if (/^[ \t]*\r?\n?[ \t]*$/.test(quelle.text.slice(bloecke[i - 1].range[1], bloecke[i].range[0]))) context.report({ loc: bloecke[i - 1].loc, messageId: 'doppelt' });
        }
      },
    };
  },
};

export default tseslint.config(
  { ignores: ['dist', 'dist-skin', 'dist-tablett', 'public', 'test-results', 'playwright-report', 'lighthouse', '.lighthouseci'] },
  js.configs.recommended,
  tseslint.configs.recommendedTypeChecked,
  { plugins: { eigen: { rules: { 'jsdoc-doppelt': jsdocDoppelt } } }, rules: { 'eigen/jsdoc-doppelt': 'error' } },
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
  // Die Gegenproben sind reines JavaScript ohne Typen, siehe docs/entwicklung/tests.md, „Mutationen“.
  { files: ['tests/*.mjs'], ...tseslint.configs.disableTypeChecked },
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
