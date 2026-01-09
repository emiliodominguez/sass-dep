import js from "@eslint/js";
import globals from "globals";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import tseslint from "typescript-eslint";
import jsdocPlugin from "eslint-plugin-jsdoc";
import importPlugin from "eslint-plugin-import";

export default tseslint.config(
	{ ignores: ["dist"] },
	{
		extends: [js.configs.recommended, ...tseslint.configs.recommended],
		files: ["**/*.{ts,tsx}"],
		languageOptions: {
			ecmaVersion: 2022,
			globals: globals.browser,
			parserOptions: {
				ecmaFeatures: {
					jsx: true,
				},
			},
		},
		plugins: {
			"react-hooks": reactHooks,
			"react-refresh": reactRefresh,
			jsdoc: jsdocPlugin,
			import: importPlugin,
		},
		rules: {
			// React
			...reactHooks.configs.recommended.rules,
			"react-refresh/only-export-components": ["warn", { allowConstantExport: true }],

			// Code quality
			eqeqeq: ["warn", "always"],
			"func-style": ["warn", "declaration"],
			"no-duplicate-imports": "error",
			"no-unused-expressions": ["error", { allowShortCircuit: true }],
			"no-var": "error",
			"no-useless-constructor": "error",

			// Import ordering
			"import/order": [
				"warn",
				{
					groups: [["builtin", "external"], "internal", ["parent", "sibling", "index"], "unknown"],
					"newlines-between": "always",
					pathGroups: [
						{ pattern: "react", group: "external", position: "before" },
						{ pattern: "@xyflow/**", group: "external" },
						{ pattern: "**/*.scss", group: "unknown", position: "after" },
					],
					pathGroupsExcludedImportTypes: ["react"],
				},
			],
			"sort-imports": [
				"warn",
				{
					ignoreCase: false,
					ignoreDeclarationSort: true,
					ignoreMemberSort: false,
					memberSyntaxSortOrder: ["none", "all", "multiple", "single"],
					allowSeparatedGroups: true,
				},
			],

			// JSDoc
			"jsdoc/no-multi-asterisks": "warn",
			"jsdoc/require-jsdoc": [
				"warn",
				{
					require: {
						FunctionDeclaration: true,
						MethodDefinition: false,
						ClassDeclaration: false,
						ArrowFunctionExpression: false,
						FunctionExpression: false,
					},
					checkConstructors: false,
				},
			],
			"jsdoc/require-param-description": "warn",
			"jsdoc/require-param-name": "warn",
			"jsdoc/require-returns-description": "off",

			// TypeScript
			"@typescript-eslint/no-shadow": "warn",
			"@typescript-eslint/no-empty-function": "error",
			"@typescript-eslint/consistent-type-imports": ["error", { fixStyle: "inline-type-imports" }],
			"@typescript-eslint/no-unused-vars": [
				"error",
				{
					argsIgnorePattern: "^_",
					caughtErrors: "none",
				},
			],
			"@typescript-eslint/naming-convention": [
				"warn",
				{ selector: "interface", format: ["PascalCase"] },
				{ selector: "typeAlias", format: ["PascalCase"] },
				{ selector: "typeParameter", format: ["PascalCase"] },
				{ selector: "enum", format: ["PascalCase"] },
				{ selector: "enumMember", format: ["PascalCase"] },
			],
		},
	},
);
