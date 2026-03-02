import { readdirSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const testDir = new URL('../test/', import.meta.url);
const files = readdirSync(testDir, { withFileTypes: true })
  .filter((entry) => entry.isFile() && entry.name.endsWith('.test.ts'))
  .map((entry) => join('test', entry.name));

if (files.length === 0) {
  console.error('[sidecar:test] No test files found under test/*.test.ts');
  process.exit(1);
}

const result = spawnSync('tsx', ['--test', ...files], {
  stdio: 'inherit',
  shell: true,
});

process.exit(result.status ?? 1);
