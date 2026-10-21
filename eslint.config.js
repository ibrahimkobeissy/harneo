import js from "@eslint/js";
import globals from "globals";
import reactHooks from "eslint-plugin-react-hooks";
import prettier from "eslint-config-prettier";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: [
      "design/",
      ".impeccable/",
      "dist/",
      "target/",
      "src-tauri/gen/",
      "node_modules/",
      ".claude/",
    ],
  },
  js.configs.recommended,
  tseslint.configs.recommended,
  reactHooks.configs.flat.recommended,
  {
    files: ["src/**/*.{ts,tsx}"],
    languageOptions: { globals: globals.browser },
  },
  {
    files: ["*.config.{js,ts}", ".dependency-cruiser.cjs"],
    languageOptions: { globals: globals.node },
  },
  prettier,
);
