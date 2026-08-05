import { execSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = execSync('git rev-parse --show-toplevel', { encoding: 'utf8' }).trim();

const findings = [];
const flag = (file, msg) => findings.push({ file, msg });

const files = execSync('git ls-files', { cwd: ROOT, encoding: 'utf8' })
	.split('\n')
	.filter(Boolean)
	.filter(
		(f) =>
			!/(pnpm-lock\.yaml$)|(Cargo\.lock$)|(\.(png|jpe?g|gif|ico|icns|svg|woff2?|lock)$)|(paraglide\/)/.test(
				f
			)
	)
	.filter((f) => !f.endsWith('scripts/check-secrets.mjs'));

const PATTERNS = [
	[/-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----/, 'private key'],
	[/AKIA[0-9A-Z]{16}/, 'AWS access key id'],
	[/GOCSPX-[A-Za-z0-9_-]{20,}/, 'Google OAuth client secret'],
	[/xox[baprs]-[A-Za-z0-9-]{10,}/, 'Slack token'],
	[/gh[pousr]_[A-Za-z0-9]{30,}/, 'GitHub token'],
	// The vault key is derived from the mnemonic — a committed one is a full
	// compromise of every note that phrase ever encrypted.
	[/mnemonic\s*[:=]\s*["'][a-z]+(?: [a-z]+){11}["']/i, 'BIP-39 mnemonic'],
	// Minisign key that signs updater artifacts; lives only in GitHub secrets.
	[
		/(?:TAURI_SIGNING_PRIVATE_KEY|TAURI_PRIVATE_KEY)\s*[:=]\s*["']?(?!\s*$)(?!\$\{\{)[^\s"']{16,}/,
		'Tauri updater signing key'
	],
	// S3 credentials belong in the OS keychain / the user's own config, never here.
	[/(?:aws_)?secret_access_key\s*[:=]\s*["'][A-Za-z0-9/+=]{30,}["']/i, 'S3 secret access key']
];

for (const f of files) {
	let content;
	try {
		content = readFileSync(join(ROOT, f), 'utf8');
	} catch {
		continue;
	}
	for (const [re, label] of PATTERNS) {
		if (re.test(content)) flag(f, `looks like a ${label}`);
	}
}

if (findings.length) {
	console.error('✖ check:secrets — potential secrets in tracked files:\n');
	for (const { file, msg } of findings) console.error(`  ${file}: ${msg}`);
	console.error(
		'\nRotate the credential immediately if real. If it is a false positive, refine scripts/check-secrets.mjs.'
	);
	process.exit(1);
}
console.log('✓ check:secrets — no tracked secrets');
