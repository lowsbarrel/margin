import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative, sep } from 'node:path';

const ROOT = process.cwd();
const violations = [];
const fail = (file, msg) => violations.push({ file, msg });
const rel = (p) => relative(ROOT, p).split(sep).join('/');
const read = (p) => readFileSync(p, 'utf8');

const SKIP_DIRS = new Set(['node_modules', 'paraglide', 'target', 'gen', 'build']);
function walk(dir, match, fn) {
	if (!existsSync(dir)) return;
	for (const name of readdirSync(dir)) {
		const p = join(dir, name);
		if (statSync(p).isDirectory()) {
			if (SKIP_DIRS.has(name) || name.startsWith('.')) continue;
			walk(p, match, fn);
		} else if (match.some((suffix) => name.endsWith(suffix))) {
			fn(p, read(p));
		}
	}
}

// One version, three files. A release tag builds from Cargo.toml, the updater
// compares against tauri.conf.json, and drift between them ships an app that
// reports the wrong version and never sees the update.
{
	const versions = {
		'package.json': JSON.parse(read('package.json')).version,
		'src-tauri/tauri.conf.json': JSON.parse(read('src-tauri/tauri.conf.json')).version,
		'src-tauri/Cargo.toml': read('src-tauri/Cargo.toml').match(/^version\s*=\s*"([^"]+)"/m)?.[1]
	};
	const distinct = [...new Set(Object.values(versions))];
	if (distinct.length > 1) {
		for (const [file, v] of Object.entries(versions)) fail(file, `version is ${v}`);
	}
}

// Every #[tauri::command] must reach generate_handler! in run(). One that
// doesn't compiles and ships fine; it fails only at runtime, as an unhandled
// "command not found" from whatever screen calls it.
//
// collect_commands! (the tauri-specta registry that produces bindings.ts) is
// deliberately a subset: raw-byte Request/Response commands aren't
// representable in specta and are listed only in generate_handler!.
if (existsSync('src-tauri/src/lib.rs')) {
	const libRs = read('src-tauri/src/lib.rs');
	const handler = libRs.match(/generate_handler!\[([\s\S]*?)\]/)?.[1] ?? '';
	const registered = new Set(handler.match(/[A-Za-z_][A-Za-z0-9_]*/g) ?? []);
	walk('src-tauri/src', ['.rs'], (p, c) => {
		if (rel(p).endsWith('src-tauri/src/bin/gen_bindings.rs')) return;
		for (const m of c.matchAll(/#\[tauri::command[^\]]*\][\s\S]{0,200}?\bfn\s+([a-z0-9_]+)/g)) {
			if (!registered.has(m[1])) {
				fail(
					rel(p),
					`command "${m[1]}" is missing from generate_handler! in src-tauri/src/lib.rs (calling it fails at runtime)`
				);
			}
		}
	});
}

// The frontend talks to Rust through the generated, typed `commands` object.
// A raw invoke() bypasses the types, so the signature can drift silently.
const INVOKE_ALLOW = new Set([
	'src/lib/bindings.ts',
	'src/lib/crypto/bridge.ts',
	'src/lib/fs/bridge.ts',
	'src/lib/s3/bridge.ts'
]);
walk('src', ['.ts', '.svelte'], (p, c) => {
	if (/@tauri-apps\/api\/core/.test(c) && !INVOKE_ALLOW.has(rel(p))) {
		fail(
			rel(p),
			'raw invoke() — call the generated `commands` from $lib/bindings, or add a bridge seam and allowlist it in scripts/check-invariants.mjs'
		);
	}
});

// Untrusted note content reaches the DOM; {@html} is where that becomes XSS.
const HTML_ALLOW = new Set(['src/lib/components/SidebarSearch.svelte']);
walk('src', ['.svelte'], (p, c) => {
	if (/\{@html\b/.test(c) && !HTML_ALLOW.has(rel(p))) {
		fail(
			rel(p),
			'{@html} outside the allowlist — escape the input, or add the file to HTML_ALLOW in scripts/check-invariants.mjs'
		);
	}
});

// AGENTS.md: "No barrel files" — for code we write. Vendored trees keep the
// import convention of their generator: shadcn-svelte emits one index.ts per
// component and rewrites it on every `add`, and movingicons exists to be
// drop-in compatible with `lucide-svelte`.
const BARREL_ALLOW = new Set(['src/lib/ui/index.ts', 'src/lib/components/movingicons/index.ts']);
const barrelExempt = (f) => BARREL_ALLOW.has(f) || f.startsWith('src/lib/components/ui/');
walk('src/lib', ['index.ts'], (p, c) => {
	if (/^\s*export\s+(\*|\{)[^;]*\bfrom\b/m.test(c) && !barrelExempt(rel(p))) {
		fail(
			rel(p),
			'barrel file — import from the concrete module instead (AGENTS.md: "No barrel files")'
		);
	}
});

// Colours are semantic tokens resolved from data-theme; a literal hex in a
// style block is a colour that cannot follow the theme. The lightbox is the
// exception — its chrome sits on an always-opaque dark scrim, so it is fixed
// on purpose rather than theme-dependent.
const HEX_ALLOW = new Set(['src/lib/components/ImageLightbox.svelte']);
walk('src/lib/components', ['.svelte'], (p, c) => {
	if (HEX_ALLOW.has(rel(p))) return;
	const style = c.match(/<style[^>]*>([\s\S]*?)<\/style>/)?.[1] ?? '';
	if (/(?<![\w-])#[0-9a-fA-F]{3,8}\b(?![\w-])/.test(style)) {
		fail(
			rel(p),
			'literal hex colour in a <style> block — use a var(--color-*) token from $lib/styles/tokens.css'
		);
	}
});

if (violations.length) {
	console.error('✖ check:invariants — AGENTS.md invariant violations:\n');
	for (const { file, msg } of violations) console.error(`  ${file}: ${msg}`);
	process.exit(1);
}
console.log('✓ check:invariants — no violations');
