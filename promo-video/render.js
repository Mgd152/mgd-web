// Usage: node render.js <outDir> [--page template.html] [--query build=/work/<slug>/build] [--frames 0,120,600] [--workers 4] [--q 92]
const http = require('http'), fs = require('fs'), path = require('path');
const { chromium } = require('playwright');

const ROOT = __dirname;
const args = process.argv.slice(2);
const outDir = path.resolve(args[0] || 'out/frames');
const opt = (k, d) => { const i = args.indexOf('--' + k); return i >= 0 ? args[i + 1] : d; };
const pick = opt('frames', null), workers = +opt('workers', 4), quality = +opt('q', 92);
fs.mkdirSync(outDir, { recursive: true });

const types = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css', '.png': 'image/png', '.jpg': 'image/jpeg', '.woff2': 'font/woff2' };
const server = http.createServer((req, res) => {
  const p = path.join(ROOT, decodeURIComponent(req.url.split('?')[0]));
  if (!p.startsWith(ROOT) || !fs.existsSync(p) || fs.statSync(p).isDirectory()) { res.writeHead(404); return res.end(); }
  res.writeHead(200, { 'Content-Type': types[path.extname(p)] || 'application/octet-stream' });
  fs.createReadStream(p).pipe(res);
});

(async () => {
  await new Promise(r => server.listen(0, '127.0.0.1', r));
  const query = opt('query', '');
  const url = `http://127.0.0.1:${server.address().port}/renderer/${opt('page', 'template.html')}${query ? '?' + query : ''}`;
  const browser = await chromium.launch({ args: ['--force-color-profile=srgb', '--font-render-hinting=none'] });
  const open = async () => {
    const page = await browser.newPage({ viewport: { width: 1080, height: 1920 }, deviceScaleFactor: 1 });
    page.on('pageerror', e => console.error('PAGE ERROR', e.message));
    page.on('console', m => { if (m.type() === 'error') console.error('console', m.text()); });
    await page.goto(url);
    await page.waitForFunction(() => window.READY === true, null, { timeout: 60000 });
    return page;
  };
  const first = await open();
  const N = await first.evaluate(() => window.NFRAMES);
  const list = pick ? pick.split(',').map(Number) : [...Array(N).keys()];
  const pages = [first];
  for (let w = 1; w < Math.min(workers, list.length); w++) pages.push(await open());
  const t0 = Date.now(); let done = 0, tDraw = 0, tShot = 0;
  await Promise.all(pages.map(async (page, w) => {
    for (let j = w; j < list.length; j += pages.length) {
      const i = list[j];
      let tm = Date.now(); await page.evaluate(i => window.renderFrame(i), i); tDraw += Date.now() - tm; tm = Date.now();
      await page.screenshot({ path: path.join(outDir, String(i).padStart(5, '0') + '.jpg'), type: 'jpeg', quality }); tShot += Date.now() - tm;
      if (++done % 60 === 0) console.log(`${done}/${list.length} frames, ${((Date.now() - t0) / done).toFixed(0)} ms/frame`);
    }
  }));
  console.log(`done ${list.length} frames in ${((Date.now() - t0) / 1000).toFixed(1)}s (per frame: draw ${(tDraw / list.length).toFixed(0)} ms, screenshot ${(tShot / list.length).toFixed(0)} ms)`);
  await browser.close(); server.close();
})().catch(e => { console.error(e); process.exit(1); });
