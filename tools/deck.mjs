// The presentation, from assets/deck/mnemorph.html to mnemorph.pdf beside it, one 16:9 page a
// slide, through the board's pinned Chromium. Run from packages/board: node ../../tools/deck.mjs
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const { chromium } = createRequire(resolve('package.json'))('@playwright/test');

const html = resolve('../../assets/deck/mnemorph.html');
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 } });
await page.goto(pathToFileURL(html).href, { waitUntil: 'networkidle' });
await page.evaluate(() => document.fonts.ready);
await page.pdf({
  path: html.replace(/\.html$/, '.pdf'),
  width: '1920px',
  height: '1080px',
  printBackground: true,
});
if (process.argv[2]) {
  const slides = page.locator('.slide');
  for (let i = 0; i < (await slides.count()); i++) {
    await slides.nth(i).screenshot({ path: `${process.argv[2]}/slide-${i + 1}.png` });
  }
  const over = await page.evaluate(() =>
    [...document.querySelectorAll('.slide')].flatMap((s, i) =>
      [...s.querySelectorAll('h2,h3,p,li,.card,.step')]
        .filter(
          (e) =>
            e.scrollWidth > e.clientWidth + 2 ||
            e.scrollHeight > e.clientHeight + 2 ||
            e.getBoundingClientRect().bottom > s.getBoundingClientRect().bottom - 30,
        )
        .map((e) => `slide ${i + 1}: ${e.tagName} ${e.textContent.trim().slice(0, 40)}`),
    ),
  );
  console.log(over.length ? over.join('\n') : 'nothing overflows');
}
await browser.close();
