import js from '@eslint/js';
import globals from 'globals';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  { ignores: ['dist', 'dist-tablett', 'public','test-results', 'playwright-report', 'lighthouse', '.lighthouseci'] },
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
    files: ['tests/**', 'playwright.config.ts', 'vite.config.ts', 'lighthouserc.cjs'],
    languageOptions: { globals: globals.node },
  },
);
