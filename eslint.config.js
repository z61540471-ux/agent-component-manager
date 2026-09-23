import js from '@eslint/js';
import ts from 'typescript-eslint';
export default ts.config(
  { ignores: ['dist/**', 'node_modules/**', 'src-tauri/**'] },
  js.configs.recommended,
  ...ts.configs.recommended,
  { files: ['src/**/*.{ts,tsx}'], rules: { '@typescript-eslint/consistent-type-imports': 'off' } },
);
