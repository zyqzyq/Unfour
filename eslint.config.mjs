import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: [
      "**/.astro/**",
      "**/.next/**",
      "**/build/**",
      "**/coverage/**",
      "**/dist/**",
      "**/ds-bundle/**",
      "**/generated/**",
      "**/node_modules/**",
      "**/target/**",
      "**/*.js",
      "**/*.mjs",
      "**/*.cjs",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    languageOptions: {
      globals: { ...globals.browser, ...globals.node },
    },
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "react-refresh/only-export-components": "warn",
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/no-unused-vars": [
        "warn",
        {
          argsIgnorePattern: "^_",
          varsIgnorePattern: "^_",
          caughtErrorsIgnorePattern: "^_",
        },
      ],
      "@typescript-eslint/no-empty-object-type": "off",
      "@typescript-eslint/no-unused-expressions": "off",
      "complexity": ["warn", 15],
      "max-depth": ["warn", 5],
      "max-lines": [
        "warn",
        {
          max: 600,
          skipBlankLines: true,
          skipComments: true,
        },
      ],
      "max-lines-per-function": [
        "warn",
        {
          IIFEs: true,
          max: 150,
          skipBlankLines: true,
          skipComments: true,
        },
      ],
      "max-params": ["warn", 5],
      // Hook return values are plain objects, not refs — false positives with current plugin version
      "react-hooks/refs": "warn",
      // Derived-state syncing via useEffect is an intentional pattern in this codebase
      "react-hooks/set-state-in-effect": "warn",
      // Overly aggressive in current plugin version; downgrade to avoid false positives
      "react-hooks/immutability": "warn",
    },
  },
  {
    files: ["**/*.test.{ts,tsx}", "**/*.spec.{ts,tsx}"],
    rules: {
      "complexity": ["warn", 25],
      "max-depth": ["warn", 6],
      "max-lines": [
        "warn",
        {
          max: 1200,
          skipBlankLines: true,
          skipComments: true,
        },
      ],
      "max-lines-per-function": [
        "warn",
        {
          IIFEs: true,
          max: 250,
          skipBlankLines: true,
          skipComments: true,
        },
      ],
      "max-params": ["warn", 8],
    },
  },
  {
    files: ["**/*.d.ts"],
    rules: {
      "complexity": "off",
      "max-depth": "off",
      "max-lines": "off",
      "max-lines-per-function": "off",
      "max-params": "off",
    },
  },
);
