#!/usr/bin/env node
/**
 * Resolution parity check against Dart Sass.
 *
 * Compiles each entry point with Dart Sass and compares its `loadedUrls`
 * (the exact set of files Sass loads) with the files sass-dep reports as
 * reachable from that entry point. Any difference fails the run.
 *
 * Usage:
 *   node scripts/sass-parity.mjs <sass-dep-binary> <root> <entry> [--load-path <dir>]...
 *
 * `sass` is resolved from web/node_modules (installed by `npm ci` in web/).
 */

import { execFileSync } from "node:child_process";
import { realpathSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(new URL("../web/package.json", import.meta.url));
const sass = require("sass");

/**
 * Parses command-line arguments.
 * @param {string[]} argv - Arguments after the script name
 * @returns {{ binary: string, root: string, entry: string, loadPaths: string[] }} Parsed options
 */
function parseArgs(argv) {
	const [binary, root, entry, ...rest] = argv;
	if (!binary || !root || !entry) {
		console.error("usage: sass-parity.mjs <sass-dep-binary> <root> <entry> [--load-path <dir>]...");
		process.exit(2);
	}

	const loadPaths = [];
	for (let i = 0; i < rest.length; i += 2) {
		if (rest[i] !== "--load-path" || !rest[i + 1]) {
			console.error(`unexpected argument: ${rest[i]}`);
			process.exit(2);
		}
		loadPaths.push(rest[i + 1]);
	}

	return { binary: path.resolve(binary), root: path.resolve(root), entry, loadPaths };
}

/**
 * Returns the canonical paths of every file Dart Sass loads for an entry.
 * @param {string} root - Project root
 * @param {string} entry - Entry point relative to the root
 * @param {string[]} loadPaths - Load paths relative to the root
 * @returns {Set<string>} Canonical file paths
 */
function sassLoadedFiles(root, entry, loadPaths) {
	const result = sass.compile(path.join(root, entry), {
		loadPaths: loadPaths.map((p) => path.join(root, p)),
		logger: sass.Logger.silent,
	});

	return new Set(result.loadedUrls.filter((url) => url.protocol === "file:").map((url) => realpathSync(fileURLToPath(url))));
}

/**
 * Returns the canonical paths of every file sass-dep reaches from an entry.
 * @param {string} binary - Path to the sass-dep binary
 * @param {string} root - Project root
 * @param {string} entry - Entry point relative to the root
 * @param {string[]} loadPaths - Load paths relative to the root
 * @returns {{ files: Set<string>, unresolved: object[] }} Reachable files and unresolved imports
 */
function sassDepReachableFiles(binary, root, entry, loadPaths) {
	const args = ["--root", root, "-q", "analyze", entry, ...loadPaths.flatMap((p) => ["-I", p])];
	const output = JSON.parse(execFileSync(binary, args, { maxBuffer: 1 << 28 }));
	const files = Object.values(output.nodes)
		.filter((node) => node.metrics.depth !== null)
		.map((node) => realpathSync(node.path));

	return { files: new Set(files), unresolved: output.analysis.unresolved };
}

/**
 * Runs the comparison and exits non-zero on any mismatch.
 */
function main() {
	const { binary, root, entry, loadPaths } = parseArgs(process.argv.slice(2));
	const expected = sassLoadedFiles(root, entry, loadPaths);
	const { files: actual, unresolved } = sassDepReachableFiles(binary, root, entry, loadPaths);

	const missing = [...expected].filter((file) => !actual.has(file));
	const extra = [...actual].filter((file) => !expected.has(file));
	const ok = missing.length === 0 && extra.length === 0 && unresolved.length === 0;

	console.log(`${ok ? "ok  " : "FAIL"} ${entry}: sass=${expected.size} sass-dep=${actual.size} missing=${missing.length} extra=${extra.length} unresolved=${unresolved.length}`);
	for (const file of missing) console.log(`  missing ${path.relative(root, file)}`);
	for (const file of extra) console.log(`  extra   ${path.relative(root, file)}`);
	for (const item of unresolved) console.log(`  unresolved ${item.from} -> ${item.target}`);

	process.exit(ok ? 0 : 1);
}

main();
