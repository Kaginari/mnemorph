# Third-party notices

Mnemorph adapts material from the projects below. Each notice is kept with the material it
covers, as its licence asks.

## Cloudflare security-audit-skill

- **Upstream:** <https://github.com/cloudflare/security-audit-skill>
- **Pinned commit:** `c1c8a8c1471069fb0e188eeaff69b8e8db6564a8`
- **Licence:** MIT, © 2025–2026 Cloudflare, Inc.
- **What Mnemorph adapts:** the Dark Elf's audit discipline — its playbooks of attack classes,
  the finding contract (a line-free root-cause fingerprint, the "needs validation" verdict,
  severity capped at demonstrated impact), the coverage ledger and independent verification
  ([races](docs/02-races.md#-dark-elf--the-adversary)), and the rules of its
  `validate-findings.cjs` and `validate-coverage-ledger.cjs` that `crates/mnemorph-proving` holds
  a finding record and a coverage ledger to. The playbooks and the validators are rewritten in
  Mnemorph's own words, format and language, not copied; material taken from a later upstream
  commit moves the pin above.
- **Adapted upstream files** (`skills/security-audit/`): `ATTACK-CLASSES.md`, `HUNTING.md`,
  `AI-AND-LLM.md`, `SUPPLY-CHAIN-AND-RELEASE.md`, `WEB-PROTOCOL-AND-AUTH.md`,
  `MEMORY-SAFETY-AND-BINARY.md`, `PROTOCOLS-RPC-AND-MESSAGING.md`,
  `RESOURCE-EXHAUSTION-AND-AVAILABILITY.md`, `DATA-ISOLATION-AND-LIFECYCLE.md`,
  `DESKTOP-MOBILE-AND-LOCAL-IPC.md`, `CLIENT-SIDE.md` and `CLOUD-AND-DEPLOYMENT.md`, into the
  per-class playbooks under `crates/mnemorph-races/playbooks/`; `validate-findings.cjs` and
  `validate-coverage-ledger.cjs` into `crates/mnemorph-proving`.

```text
MIT License

Copyright (c) 2025-2026 Cloudflare, Inc.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
