// Headless Chromium test for editor/index.html. Run: node editor/tests/editor.test.mjs
import { createRequire } from 'node:module';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';

const require = createRequire(import.meta.url);
let playwright;
try { playwright = require('playwright'); } catch { playwright = require('/opt/node22/lib/node_modules/playwright'); }
const { chromium } = playwright;

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '..', '..');
const html = pathToFileURL(path.join(root, 'editor', 'index.html')).href;
// Always rebuild so parity is checked against the current Rust rules; fail if the binary is older than the sources.
const build = spawnSync('cargo', ['build', '--release', '-p', 'keymap', '--bin', 'keymap-sim'], { cwd: root, encoding: 'utf8' });
console.log('cargo build --release -p keymap --bin keymap-sim -> exit ' + build.status);
const sim = ['target/release/keymap-sim', 'target/release/keymap-sim.exe']
  .map(p => path.join(root, p)).find(p => fs.existsSync(p));
const srcDir = path.join(root, 'crates/keymap/src');
const newestSrc = Math.max(...fs.readdirSync(srcDir, { recursive: true }).filter(f => f.endsWith('.rs')).map(f => fs.statSync(path.join(srcDir, f)).mtimeMs));
const simFresh = !!sim && fs.statSync(sim).mtimeMs >= newestSrc;
const shot = process.env.EDITOR_SHOT || path.join(os.tmpdir(), 'editor.png');
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'editor-test-'));

let failures = 0, checks = 0;
const ok = (cond, msg) => { checks++; if (!cond) { failures++; console.log('FAIL: ' + msg); } else console.log('ok:   ' + msg); };
ok(build.status === 0 && simFresh, 'keymap-sim is built and not older than crates/keymap/src/*.rs');
const eq = (a, b, msg) => ok(JSON.stringify(a) === JSON.stringify(b), `${msg} (got ${JSON.stringify(a)}${JSON.stringify(a) === JSON.stringify(b) ? '' : ', want ' + JSON.stringify(b)})`);

function validate(text, label) {
  if (!sim) { ok(false, 'keymap-sim binary not found (cargo build first)'); return false; }
  const f = path.join(tmp, label.replace(/[^\w.-]+/g, '_') + '.json');
  fs.writeFileSync(f, text);
  const r = spawnSync(sim, ['validate', f], { encoding: 'utf8' });
  ok(r.status === 0, `keymap-sim validate ${label} -> exit ${r.status} ${r.status ? (r.stdout + r.stderr).trim() : ''}`);
  return r.status === 0;
}
function normalize(j) {
  const r4 = v => Math.round(v * 10000) / 10000;
  const o = { version: j.version, name: j.name ?? '', app: j.app ?? '', orientation: j.orientation, toggle_key: j.toggle_key ?? 'Backquote', panic_key: j.panic_key ?? 'F8', controls: [] };
  for (const c of j.controls ?? []) {
    if (c.type === 'joystick') o.controls.push({ type: 'joystick', label: c.label ?? '', x: r4(c.x), y: r4(c.y), radius: c.radius, up: c.up, down: c.down, left: c.left, right: c.right });
    if (c.type === 'look') o.controls.push({ type: 'look', label: c.label ?? '', x: r4(c.x), y: r4(c.y), sensitivity: c.sensitivity ?? 1, invert_y: c.invert_y ?? false, reach: c.reach ?? 0.3, release_after_ms: c.release_after_ms ?? 0 });
    if (c.type === 'button') o.controls.push({ type: 'button', label: c.label ?? '', x: r4(c.x), y: r4(c.y), key: c.key, mode: c.mode ?? 'hold' });
  }
  return o;
}

const browser = await chromium.launch();
const ctx = await browser.newContext({ viewport: { width: 1366, height: 768 }, acceptDownloads: true });
const page = await ctx.newPage();
const consoleErrors = [], badRequests = [];
page.on('console', m => { if (m.type() === 'error') consoleErrors.push(m.text()); });
page.on('pageerror', e => consoleErrors.push('pageerror: ' + e.message));
page.on('request', r => { const u = r.url(); if (!/^(file|data|blob|about):/.test(u)) badRequests.push(u); });

// background images made in the browser itself
async function makePng(w, h, label) {
  const p = await ctx.newPage();
  const b64 = await p.evaluate(([w, h, label]) => {
    const c = document.createElement('canvas'); c.width = w; c.height = h;
    const g = c.getContext('2d'); const gr = g.createLinearGradient(0, 0, w, h);
    gr.addColorStop(0, '#2b6cb0'); gr.addColorStop(1, '#2f855a'); g.fillStyle = gr; g.fillRect(0, 0, w, h);
    g.fillStyle = '#fff'; g.font = '20px sans-serif'; g.fillText(label, 20, 40);
    for (let i = 1; i < 10; i++) { g.strokeStyle = 'rgba(255,255,255,.25)'; g.strokeRect(i * w / 10, 0, 1, h); }
    return c.toDataURL('image/png').split(',')[1];
  }, [w, h, label]);
  await p.close();
  return Buffer.from(b64, 'base64');
}

await page.goto(html);
await page.waitForSelector('#svg');
eq(await page.evaluate(() => KeymapEditor.KEY_NAMES.length), 116, 'KEY_NAMES count');
eq(await page.evaluate(() => new Set(KeymapEditor.KEY_NAMES).size), 116, 'KEY_NAMES unique');
ok((await page.title()).length > 0 && (await page.getAttribute('html', 'lang')) === 'vi', 'title + lang=vi');

// ---- build a profile through the UI ----
await page.setInputFiles('#imgFile', { name: 'shot.png', mimeType: 'image/png', buffer: await makePng(360, 780, 'portrait screenshot') });
await page.waitForFunction(() => KeymapEditor.state.img);
eq(await page.inputValue('#pf_orientation'), 'portrait', 'orientation from portrait image');
ok(await page.isVisible('#bg'), 'background image shown');

await page.fill('#pf_name', 'UI Test');
await page.fill('#pf_app', 'multi-touch test app');
await page.click('#addJoystick');
await page.click('#addLook');
ok(await page.isDisabled('#addLook'), 'second look disabled');
await page.click('#addButton');
await page.click('#addButton');
eq(await page.evaluate(() => KeymapEditor.state.profile.controls.map(c => c.type)), ['joystick', 'look', 'button', 'button'], 'four controls added');
ok(await page.isDisabled('#btnDownload'), 'export disabled while button keys are unbound');
ok((await page.textContent('#vstatus')).includes('lỗi'), 'summary reports errors');
ok((await page.textContent('#vlist')).includes('controls[3].key'), 'summary names controls[3].key');

// drag joystick + resize
await page.evaluate(() => document.querySelector('[data-ctl="1"]').dispatchEvent(new Event('x')));
const box = async sel => { const b = await page.locator(sel).first().boundingBox(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 }; };
const stageBox = await page.locator('#svg').boundingBox();
const jPos = await box('[data-ctl="1"] circle >> nth=1');   // centre dot
await page.mouse.move(jPos.x, jPos.y); await page.mouse.down();
await page.mouse.move(jPos.x + 40, jPos.y - 60, { steps: 5 }); await page.mouse.up();
const j = await page.evaluate(() => KeymapEditor.state.profile.controls[0]);
ok(Math.abs(j.x - ((jPos.x + 40 - stageBox.x) / stageBox.width)) < 0.002 && Math.abs(j.y - ((jPos.y - 60 - stageBox.y) / stageBox.height)) < 0.002, `joystick dragged to (${j.x}, ${j.y})`);
const h0 = await page.evaluate(() => KeymapEditor.state.profile.controls[0].radius);
const hp = await page.locator('[data-handle="1"]').boundingBox();
await page.mouse.move(hp.x + hp.width / 2, hp.y + hp.height / 2); await page.mouse.down();
await page.mouse.move(hp.x + hp.width / 2 + 30, hp.y + hp.height / 2, { steps: 4 }); await page.mouse.up();
const h1 = await page.evaluate(() => KeymapEditor.state.profile.controls[0].radius);
ok(h1 > h0, `joystick radius resized ${h0} -> ${h1}`);
// drag clamps at edge
const lp = await box('[data-ctl="2"] circle');
await page.mouse.move(lp.x, lp.y); await page.mouse.down(); await page.mouse.move(stageBox.x + stageBox.width + 200, stageBox.y - 200, { steps: 4 }); await page.mouse.up();
const lk = await page.evaluate(() => KeymapEditor.state.profile.controls[1]);
ok(lk.x === 1 && lk.y === 0, `look drag clamped to [0,1] (${lk.x}, ${lk.y})`);
await page.fill('#f_x', '0.7'); await page.fill('#f_y', '0.5'); await page.fill('#f_reach', '0.25'); await page.fill('#f_sensitivity', '1.5');
await page.fill('#f_release_after_ms', '500'); await page.check('[data-f=invert_y]'); await page.fill('#f_label', 'Camera');

// key capture
async function bind(selector, pressFn) {
  await page.click(selector);
  ok(await page.isVisible('#capbar'), 'capture bar visible');
  await pressFn();
}
await page.click('[data-pick="3"]');
await bind('#ctlPanel [data-kt^="c:3:key"]', () => page.keyboard.press('Tab'));
eq(await page.evaluate(() => KeymapEditor.state.profile.controls[2].key), 'Tab', 'Tab captured (not a focus move)');
ok(!(await page.isVisible('#capbar')), 'capture ended');
await page.click('[data-pick="4"]');
await bind('#ctlPanel [data-kt^="c:4:key"]', () => page.keyboard.press('Space'));
eq(await page.evaluate(() => KeymapEditor.state.profile.controls[3].key), 'Space', 'Space captured');
// duplicate key detected inline
await bind('#ctlPanel [data-kt^="c:4:key"]', () => page.keyboard.press('Tab'));
ok((await page.textContent('#ctlPanel [data-err="controls[3].key"]')).includes('controls[2].key'), 'duplicate key shown inline');
await bind('#ctlPanel [data-kt^="c:4:key"]', () => page.mouse.click(5, 5, { button: 'middle' }));
eq(await page.evaluate(() => KeymapEditor.state.profile.controls[3].key), 'MouseMiddle', 'middle mouse button captured');
await page.selectOption('#f_mode', 'tap');
await page.click('[data-pick="3"]');
await bind('#ctlPanel [data-kt^="c:3:key"]', () => page.keyboard.press('MetaLeft'));
eq(await page.evaluate(() => KeymapEditor.state.profile.controls[2].key), 'SuperLeft', 'MetaLeft -> SuperLeft');
await page.evaluate(() => { window.__ctx = []; window.addEventListener('contextmenu', e => window.__ctx.push(e.defaultPrevented), true); window.__clicks = 0; window.addEventListener('auxclick', e => { if (!e.defaultPrevented) window.__clicks++; }, true); });
await bind('#ctlPanel [data-kt^="c:3:key"]', () => page.mouse.click(5, 5, { button: 'right' }));
eq(await page.evaluate(() => KeymapEditor.state.profile.controls[2].key), 'MouseRight', 'right mouse -> MouseRight');
ok((await page.evaluate(() => window.__ctx)).length > 0 && (await page.evaluate(() => window.__ctx)).every(v => v), 'contextmenu after right-click bind is defaultPrevented');
// left click bind over a real button must not also activate it
await bind('#ctlPanel [data-kt^="c:3:key"]', () => page.click('#addJoystick'));
eq(await page.evaluate(() => KeymapEditor.state.profile.controls[2].key), 'MouseLeft', 'left-click over + Joystick binds MouseLeft');
eq(await page.evaluate(() => KeymapEditor.state.profile.controls.length), 4, 'the click that bound the key did not add a joystick');
await bind('#ctlPanel [data-kt^="c:3:key"]', () => page.mouse.click(5, 5, { button: 'right' }));
eq(await page.evaluate(() => KeymapEditor.state.profile.controls[2].key), 'MouseRight', 'rebind MouseRight');
await page.click('#addJoystick'); // normal clicks work again right after
eq(await page.evaluate(() => KeymapEditor.state.profile.controls.length), 5, 'normal click works after capture');
await page.click('#btnDel');
eq(await page.evaluate(() => KeymapEditor.state.profile.controls.length), 4, 'extra joystick removed');
await page.click('[data-pick="3"]');
// toggle/panic: mouse refused, key accepted
await bind('#kb_toggle', () => page.mouse.click(5, 5, { button: 'left' }));
ok(await page.isVisible('#capbar') && (await page.textContent('#capmsg')).includes('bàn phím'), 'mouse refused for toggle key');
await page.keyboard.press('F2');
eq(await page.evaluate(() => KeymapEditor.state.profile.toggle_key), 'F2', 'toggle_key = F2');
await bind('#kb_panic', () => page.keyboard.press('F3'));
eq(await page.evaluate(() => KeymapEditor.state.profile.panic_key), 'F3', 'panic_key = F3');
// unsupported key refused, Hủy cancels, double Escape cancels, single Escape binds
await page.click('#kb_panic'); await page.keyboard.press('AudioVolumeUp');
ok(await page.isVisible('#capbar'), 'unsupported key refused (still capturing)');
await page.click('#capcancel'); ok(!(await page.isVisible('#capbar')), 'Hủy cancels');
await page.click('#kb_panic'); await page.keyboard.press('Escape'); await page.keyboard.press('Escape');
eq(await page.evaluate(() => KeymapEditor.state.profile.panic_key), 'F3', 'double Escape cancels');
await page.click('#kb_panic'); await page.keyboard.press('Escape'); await page.waitForTimeout(1200);
eq(await page.evaluate(() => KeymapEditor.state.profile.panic_key), 'Escape', 'single Escape binds Escape');
await bind('#kb_panic', () => page.keyboard.press('F3'));
// make the mouse-right button distinct from other things; free joystick keys are WASD
ok((await page.textContent('#vstatus')).includes('hợp lệ') && !(await page.textContent('#vstatus')).includes('lỗi'), 'profile valid: ' + (await page.textContent('#vstatus')));
ok(await page.isEnabled('#btnDownload'), 'export enabled when valid');

// export via download + preview
const [dl] = await Promise.all([page.waitForEvent('download'), page.click('#btnDownload')]);
eq(dl.suggestedFilename(), 'UI_Test.json', 'download file name');
const dlPath = path.join(tmp, 'download.json'); await dl.saveAs(dlPath);
const exported = fs.readFileSync(dlPath, 'utf8');
validate(exported, 'ui-built download');
const preview = await page.inputValue('#jsonOut');
eq(preview, exported, 'preview textarea equals download');
const ej = JSON.parse(exported);
eq(Object.keys(ej), ['version', 'name', 'app', 'orientation', 'toggle_key', 'panic_key', 'controls'], 'top-level keys exact');
eq(ej.controls[0].up, 'KeyW', 'joystick default WASD');
eq(ej.controls[1].release_after_ms, 500, 'look release_after_ms exported');
ok(exported.endsWith('}\n') && exported.includes('\n  "version"'), '2-space indent + trailing newline');
await page.screenshot({ path: shot });
console.log('screenshot ->', shot);

// ---- orientation toggle keeps coordinates and warns ----
await page.selectOption('#pf_orientation', 'landscape');
ok((await page.textContent('#notice')).includes('tọa độ'), 'orientation change warns');
eq((JSON.parse(await page.inputValue('#jsonOut'))).controls[0].x, ej.controls[0].x, 'coordinates kept after orientation change');
ok(!(await page.isVisible('#bg')), 'portrait image hidden in landscape profile');
await page.selectOption('#pf_orientation', 'portrait');

// ---- delete / duplicate ----
await page.click('[data-pick="3"]'); await page.click('#btnDup');
eq(await page.evaluate(() => KeymapEditor.state.profile.controls.length), 5, 'duplicate adds control');
eq(await page.evaluate(() => KeymapEditor.state.profile.controls[3].key), '', 'duplicate clears key');
ok(await page.isDisabled('#btnDownload'), 'export disabled after duplicate (unbound key)');
await page.click('#btnDel');
eq(await page.evaluate(() => KeymapEditor.state.profile.controls.length), 4, 'delete removes control');

// ---- clipboard / copy ----
await ctx.grantPermissions(['clipboard-read', 'clipboard-write']).catch(() => {});
await page.click('#btnCopy');
await page.waitForFunction(() => document.querySelector('#importMsg').textContent.length > 0, null, { timeout: 5000 }).catch(() => {});
ok((await page.textContent('#importMsg')).length > 0, 'copy reports a result: ' + (await page.textContent('#importMsg')));

// ---- landscape image sets orientation ----
await page.click('#btnNew');
await page.setInputFiles('#imgFile', { name: 'land.png', mimeType: 'image/png', buffer: await makePng(780, 360, 'landscape') });
await page.waitForFunction(() => KeymapEditor.state.img && KeymapEditor.state.img.w === 780);
eq(await page.inputValue('#pf_orientation'), 'landscape', 'orientation from landscape image');
const sb = await page.locator('#stage').boundingBox();
ok(Math.abs(sb.width / sb.height - 780 / 360) < 0.02, 'stage keeps image aspect');

// ---- import samples + shared vectors ----
async function importFile(file) {
  await page.setInputFiles('#jsonFile', file);
  await page.waitForFunction(() => document.querySelector('#importMsg').textContent.length > 0);
  await page.waitForTimeout(30);
}
const lists = [
  ['profiles', path.join(root, 'profiles')],
  ['valid', path.join(root, 'crates/keymap/tests/data/valid')],
];
for (const [tag, dir] of lists) {
  const files = fs.existsSync(dir) ? fs.readdirSync(dir).filter(f => f.endsWith('.json')).sort() : [];
  ok(tag === 'profiles' ? files.length >= 2 : files.length > 0, `${tag}: ${files.length} json files found`);
  for (const f of files) {
    await page.evaluate(() => { document.querySelector('#importMsg').textContent = ''; });
    await importFile(path.join(dir, f));
    const msg = await page.textContent('#importMsg');
    const outText = await page.inputValue('#jsonOut');
    const src = JSON.parse(fs.readFileSync(path.join(dir, f), 'utf8').replace(/^﻿/, ''));
    ok(msg.includes('hợp lệ'), `${tag}/${f}: import reports valid`);
    eq(JSON.parse(outText), normalize(src), `${tag}/${f}: re-export semantically equal`);
    validate(outText, `${tag}-${f}`);
  }
}
const invDir = path.join(root, 'crates/keymap/tests/data/invalid');
if (fs.existsSync(invDir)) {
  const expected = JSON.parse(fs.readFileSync(path.join(invDir, 'expected.json'), 'utf8'));
  for (const [f, want] of Object.entries(expected)) {
    await page.evaluate(() => { const m = document.querySelector('#importMsg'); m.textContent = ''; m.dataset.firstPath = ''; });
    await importFile(path.join(invDir, f));
    eq(await page.getAttribute('#importMsg', 'data-first-path'), want, `invalid/${f}: first problem path`);
  }
}
// paste import
await page.fill('#jsonOut', '{"version":1,"orientation":"portrait","controls":[{"type":"button","x":0.5,"y":0.5,"key":"KeyJ"}]}');
await page.click('#btnPaste');
eq(await page.evaluate(() => KeymapEditor.state.profile.controls.length), 1, 'paste import loads profile');

// ---- resize handles stay reachable ----
const cid = i => page.evaluate(i => KeymapEditor.state.profile.controls[i]._id, i);
const hit = async i => page.evaluate(id => {
  const h = document.querySelector(`[data-handle="${id}"]`); if (!h) return 'none';
  const r = h.getBoundingClientRect(); const e = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2);
  return e === h ? 'handle' : (e && e.id) || e.tagName;
}, await cid(i));
async function dragHandle(i, dx, dy) {
  const id = await cid(i);
  const b = await page.locator(`[data-handle="${id}"]`).boundingBox(); const x = b.x + b.width / 2, y = b.y + b.height / 2;
  await page.mouse.move(x, y); await page.mouse.down(); await page.mouse.move(x + dx, y + dy, { steps: 4 }); await page.mouse.up();
}
await page.click('#btnNew');
await page.click('#addLook');
eq(await hit(0), 'handle', 'default look handle is hittable');
const r0 = await page.evaluate(() => KeymapEditor.state.profile.controls[0].reach);
await dragHandle(0, -20, -20);
ok((await page.evaluate(() => KeymapEditor.state.profile.controls[0].reach)) !== r0, 'default look handle resizes');
for (const [x, y] of [[0.95, 0.5], [0.5, 0.97], [0.97, 0.97]]) {
  await page.fill('#f_x', String(x)); await page.fill('#f_y', String(y)); await page.fill('#f_reach', '0.3');
  eq(await hit(0), 'handle', `look at (${x}, ${y}) handle is hittable`);
}
await page.fill('#f_x', '0.95'); await page.fill('#f_y', '0.5');
const r1 = await page.evaluate(() => KeymapEditor.state.profile.controls[0].reach);
await dragHandle(0, -25, 0);
ok((await page.evaluate(() => KeymapEditor.state.profile.controls[0].reach)) !== r1, 'edge look handle resizes');
await page.click('#addJoystick'); await page.fill('#f_x', '0.97'); await page.fill('#f_radius', '0.2');
eq(await hit(1), 'handle', 'joystick at right edge handle is hittable');
// emptying a number field must not break rendering
await page.fill('#f_radius', ''); await page.fill('#f_x', '');
ok((await page.textContent('#vstatus')).includes('lỗi'), 'empty number fields are reported as errors');
eq(await page.evaluate(() => document.querySelector('#jsonOut').value.includes('NaN')), false, 'no NaN text in preview');
// layout at 1366x768: no page scroll, status + export buttons visible
const lay = await page.evaluate(() => ({ sh: document.documentElement.scrollHeight, ih: innerHeight, st: document.querySelector('#vstatus').getBoundingClientRect().bottom, bt: document.querySelector('#btnDownload').getBoundingClientRect().bottom }));
ok(lay.sh <= lay.ih && lay.st <= lay.ih && lay.bt <= lay.ih, `no page scroll at 1366x768, status/export visible (${JSON.stringify(lay)})`);
// numeric import rounding + strictness
await page.fill('#jsonOut', '{"version":1,"orientation":"portrait","controls":[{"type":"joystick","x":0.5,"y":0.5,"radius":0.00001,"up":"KeyW","down":"KeyS","left":"KeyA","right":"KeyD"}]}');
await page.click('#btnPaste');
ok((await page.textContent('#vstatus')).includes('lỗi') && (await page.textContent('#vlist')).includes('controls[0].radius'), 'radius 0.00001 (exports as 0) is flagged at import');
for (const [txt, what] of [['{"version":1.0,"orientation":"portrait"}', 'version 1.0'], ['{"version":1,"orientation":"portrait","controls":[{"type":"look","x":0.5,"y":0.5,"release_after_ms":1e2}]}', 'release_after_ms 1e2'], ['{"version":1,"orientation":"portrait","controls":[{"type":"look","x":1e999,"y":0.5}]}', 'x 1e999']]) {
  await page.fill('#jsonOut', txt); await page.click('#btnPaste');
  ok((await page.textContent('#importMsg')).startsWith('Không nhập được'), `import rejects ${what}`);
}
await page.fill('#jsonOut', '{"version":2,"orientation":"portrait"}'); await page.click('#btnPaste');
ok(await page.isVisible('#btnVer1'), 'version != 1 offers "Đặt version = 1"');
await page.click('#btnVer1');
ok((await page.textContent('#vstatus')).includes('hợp lệ'), 'version fixed in UI');
eq(await page.evaluate(() => ['kb_toggle', 'kb_panic'].map(i => !!document.getElementById(i).getAttribute('aria-labelledby'))), [true, true], 'key buttons have aria-labelledby');

// ---- no network, no console errors ----
eq(badRequests, [], 'no network requests');
eq(consoleErrors, [], 'no console errors');

await browser.close();
console.log(`\n${checks - failures}/${checks} checks passed`);
process.exit(failures ? 1 : 0);
