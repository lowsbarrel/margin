import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import ts from 'typescript';

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

const INVOKE_ALLOW = new Set([
	'src/lib/bindings.ts',
	'src/lib/ai/bridge.ts',
	'src/lib/crypto/bridge.ts',
	'src/lib/fs/bridge.ts',
	'src/lib/s3/bridge.ts',
	'src/lib/terminal/bridge.ts'
]);
walk('src', ['.ts', '.svelte'], (p, c) => {
	if (/@tauri-apps\/api\/core/.test(c) && !INVOKE_ALLOW.has(rel(p))) {
		fail(
			rel(p),
			'raw invoke() — call the generated `commands` from $lib/bindings, or add a bridge seam and allowlist it in scripts/check-invariants.mjs'
		);
	}
});

const HTML_ALLOW = new Set();
walk('src', ['.svelte'], (p, c) => {
	if (/\{@html\b/.test(c) && !HTML_ALLOW.has(rel(p))) {
		fail(
			rel(p),
			'{@html} outside the allowlist — escape the input, or add the file to HTML_ALLOW in scripts/check-invariants.mjs'
		);
	}
});

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

const MAX_LINES = 400;
const MAX_COMMENTS = 2;
const DIRECTIVE =
	/^(eslint-|@ts-|svelte-ignore|prettier-ignore|@vite-ignore|<reference\s|@type\s*\{|@satisfies\s*\{)/;
const generated = (f) => f === 'src/lib/bindings.ts' || f.startsWith('src/lib/components/ui/');

function jsComments(code, offset = 0, kind = ts.ScriptKind.TS) {
	const sf = ts.createSourceFile('x.ts', code, ts.ScriptTarget.Latest, false, kind);
	const found = new Map();
	const visit = (node) => {
		for (const r of [
			...(ts.getLeadingCommentRanges(code, node.pos) ?? []),
			...(ts.getTrailingCommentRanges(code, node.pos) ?? [])
		]) {
			found.set(r.pos, { pos: offset + r.pos, text: code.slice(r.pos, r.end) });
		}
		for (const child of node.getChildren(sf)) visit(child);
	};
	visit(sf);
	return [...found.values()];
}

function cssComments(code, offset = 0) {
	const out = [];
	for (let i = 0; i < code.length; i++) {
		const c = code[i];
		if (c === '"' || c === "'") {
			for (i++; i < code.length && code[i] !== c; i++) if (code[i] === '\\') i++;
		} else if (c === '/' && code[i + 1] === '*') {
			const end = code.indexOf('*/', i + 2);
			const stop = end < 0 ? code.length : end + 2;
			out.push({ pos: offset + i, text: code.slice(i, stop) });
			i = stop - 1;
		}
	}
	return out;
}

function rustComments(code) {
	const out = [];
	const ident = /[A-Za-z0-9_]/;
	for (let i = 0; i < code.length; i++) {
		const c = code[i];
		if (c === '/' && code[i + 1] === '/') {
			const end = code.indexOf('\n', i);
			const stop = end < 0 ? code.length : end;
			out.push({ pos: i, text: code.slice(i, stop) });
			i = stop;
		} else if (c === '/' && code[i + 1] === '*') {
			let depth = 1;
			let j = i + 2;
			while (j < code.length && depth) {
				const opens = code.startsWith('/*', j);
				const closes = !opens && code.startsWith('*/', j);
				depth += opens ? 1 : closes ? -1 : 0;
				j += opens || closes ? 2 : 1;
			}
			out.push({ pos: i, text: code.slice(i, j) });
			i = j - 1;
		} else if ((c === 'r' || c === 'b') && !ident.test(code[i - 1] ?? '')) {
			const raw = /^b?r(#*)"/.exec(code.slice(i, i + 260));
			if (raw) {
				const close = code.indexOf(`"${raw[1]}`, i + raw[0].length);
				i = (close < 0 ? code.length : close) + raw[1].length;
			}
		} else if (c === '"') {
			for (i++; i < code.length && code[i] !== '"'; i++) if (code[i] === '\\') i++;
		} else if (c === "'") {
			if (code[i + 1] === '\\') i = code.indexOf("'", i + 3);
			else if (code[i + 2] === "'") i += 2;
			else if (code[i + 3] === "'" && code.codePointAt(i + 1) > 0xffff) i += 3;
		}
	}
	return out;
}

function svelteComments(code) {
	const out = [];
	let markup = code;
	for (const m of code.matchAll(/(<script[^>]*>)([\s\S]*?)<\/script>/g)) {
		out.push(...jsComments(m[2], m.index + m[1].length));
		markup = markup.replace(m[0], ' '.repeat(m[0].length));
	}
	for (const m of code.matchAll(/(<style[^>]*>)([\s\S]*?)<\/style>/g)) {
		out.push(...cssComments(m[2], m.index + m[1].length));
		markup = markup.replace(m[0], ' '.repeat(m[0].length));
	}
	for (const m of markup.matchAll(/<!--[\s\S]*?-->/g)) out.push({ pos: m.index, text: m[0] });
	return out;
}

function commentsOf(file, code) {
	if (file.endsWith('.rs')) return rustComments(code);
	if (file.endsWith('.css')) return cssComments(code);
	if (file.endsWith('.svelte')) return svelteComments(code);
	return jsComments(code, 0, file.endsWith('.ts') ? ts.ScriptKind.TS : ts.ScriptKind.JS);
}

function checkSource(p, code) {
	const file = rel(p);
	if (generated(file)) return;
	const lines = code.split('\n').length - (code.endsWith('\n') ? 1 : 0);
	if (lines > MAX_LINES) {
		fail(file, `${lines} lines — split it by responsibility (limit ${MAX_LINES})`);
	}
	const lineOf = (pos) => code.slice(0, pos).split('\n').length;
	const comments = commentsOf(file, code)
		.map((c) => ({
			...c,
			line: lineOf(c.pos),
			body: c.text.replace(/^(\/\/+!?|\/\*+!?|<!--)\s*/, '')
		}))
		.filter((c) => !DIRECTIVE.test(c.body))
		.sort((a, b) => a.pos - b.pos);
	const doc = comments.find((c) => /^(\/\/\/|\/\/!|\/\*\*|\/\*!)/.test(c.text));
	const multi = comments.find(
		(c, i) => c.text.includes('\n') || (i > 0 && comments[i - 1].line + 1 === c.line)
	);
	if (doc)
		fail(`${file}:${doc.line}`, 'doc comment — delete it; names and types document the code');
	else if (multi)
		fail(
			`${file}:${multi.line}`,
			'multi-line comment — a comment is one line, if it exists at all'
		);
	if (comments.length > MAX_COMMENTS) {
		fail(
			file,
			`${comments.length} comments (limit ${MAX_COMMENTS}) — keep only the non-obvious constraints`
		);
	}
}

walk('src', ['.ts', '.js', '.svelte', '.css'], checkSource);
walk('src-tauri/src', ['.rs'], checkSource);
walk('scripts', ['.mjs', '.js'], checkSource);
for (const f of readdirSync('.')) if (/\.config\.(js|ts)$/.test(f)) checkSource(f, read(f));

walk('../.github', ['.yml', '.yaml'], (p, c) => {
	c.split('\n').forEach((line, i) => {
		const bare = line.replace(/'[^']*'|"(?:\\.|[^"\\])*"/g, '');
		if (/^\s*#|\s#(?!\{)/.test(bare))
			fail(`${rel(p)}:${i + 1}`, 'comment in CI config — CI carries none');
	});
});

if (violations.length) {
	console.error('✖ check:invariants — AGENTS.md invariant violations:\n');
	for (const { file, msg } of violations) console.error(`  ${file}: ${msg}`);
	process.exit(1);
}
console.log('✓ check:invariants — no violations');
