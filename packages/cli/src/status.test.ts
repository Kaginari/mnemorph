import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { PassThrough } from 'node:stream';
import { stdio } from '@mnemorph/sdk';
import { expect, test } from 'vitest';
import { parse } from './args.js';
import { FakeCore } from './fake.js';
import { statusOnce } from './status.js';

function io() {
  const out = new PassThrough();
  let stdout = '';
  out.on('data', (d) => {
    stdout += d;
  });
  return { stdin: new PassThrough(), stdout: out, stderr: new PassThrough(), text: () => stdout };
}

test('status is a command of its own, and takes no words', () => {
  expect(parse(['status', '--world', 'w'])).toEqual({
    kind: 'status',
    world: 'w',
    core: undefined,
  });
  expect(parse(['status', 'now']).kind).toBe('error');
  expect(parse(['status', '--json']).kind).toBe('error');
});

test('status names the build, the core, the key and the sandbox, and says when nothing caps', async () => {
  const t = io();
  const code = await statusOnce({ kind: 'status' }, t, new FakeCore());
  expect(code).toBe(0);
  const out = t.text();
  expect(out).toMatch(/^mnemorph 0\.0\.0 \(source\)$/m);
  expect(out).toContain('core     0.0.0 (core)');
  expect(out).toContain('key      none — the provider takes no key');
  expect(out).toContain('sandbox  none — the guard helper is missing');
  expect(out).toContain('budget   no caps set');
});

const daemon = resolve(__dirname, '../../../target/debug/mnemorphd');

test.runIf(existsSync(daemon))(
  "the real core's status names where the key comes from, never the key",
  async () => {
    const dir = mkdtempSync(join(tmpdir(), 'mnemorph-status-'));
    try {
      mkdirSync(join(dir, '.mnemorph'));
      writeFileSync(
        join(dir, '.mnemorph/config.local.toml'),
        '[providers.x]\nkind = "anthropic"\nkey_env = "MNEMORPH_TEST_KEY"\n[models]\nprovider = "x"\ndefault = "m"\n[budget]\nday = 5000\n',
      );
      const t = io();
      const transport = stdio({
        command: daemon,
        world: dir,
        env: {
          PATH: process.env.PATH,
          HOME: dir,
          XDG_STATE_HOME: dir,
          MNEMORPH_TEST_KEY: 'sk-never-shown',
        },
      });
      expect(await statusOnce({ kind: 'status' }, t, transport)).toBe(0);
      const out = t.text();
      expect(out).toMatch(/^core {5}0\.0\.0 \([0-9a-f]+\) — /m);
      expect(out).toMatch(/^key {6}MNEMORPH_TEST_KEY, named by providers\.x\.key_env in /m);
      expect(out).toMatch(/^sandbox {2}\S/m);
      expect(out).toContain('budget   left 5.0k wt (day)');
      expect(out).not.toContain('sk-never-shown');
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  },
);
