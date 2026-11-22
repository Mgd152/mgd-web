// Real 3D longswords (three.js) for songs about battle: two crossed behind the card, a few more hanging in the depth.
// Modelled rather than drawn: a hollow-ground blade with distal taper, a real fuller groove and honed cutting edges,
// a curved crossguard with quillon terminals, a leather grip under a twisted wire wrap and a wheel pommel.
// Brushed steel (roughness map running along the blade) under long studio strip lights, so the edges throw thin, sharp highlights.
export async function makeSwords(THREE, canvas, opts) {
  const { W, H, cool, hot, closeup = 0, cy = null, solo = 0 } = opts;
  const r = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, preserveDrawingBuffer: true });
  r.setPixelRatio(1); r.setSize(W, H, false); r.setClearColor(0x000000, 0);
  r.toneMapping = THREE.ACESFilmicToneMapping; r.toneMappingExposure = 1.25; r.outputColorSpace = THREE.SRGBColorSpace;
  const scene = new THREE.Scene();
  const cam = new THREE.PerspectiveCamera(28, W / H, 0.1, 100); cam.position.set(0, 0, 24); if (closeup) { cam.position.set(0, cy ?? (closeup > 1 ? -1.5 : 1), 24 - closeup * 7); cam.lookAt(0, cy ?? 0, -3); }   // test page: look closer

  // ---- studio environment: long thin strip lights give the crisp lines of light along the blades ----
  const env = new THREE.Scene(); env.background = new THREE.Color(0x030407);
  const strip = (w, h, col, k, x, y, z, rx = 0, ry = 0, rz = 0) => { const m = new THREE.Mesh(new THREE.PlaneGeometry(w, h), new THREE.MeshBasicMaterial({ color: new THREE.Color(col).multiplyScalar(k), side: THREE.DoubleSide })); m.position.set(x, y, z); m.rotation.set(rx, ry, rz); env.add(m); };
  strip(0.6, 16, 0xffffff, 6, -6, 2, 6, 0, 0.6);           // tall strip, front left
  strip(0.4, 16, 0xffffff, 4, 7, 0, 5, 0, -0.7);           // tall strip, front right
  strip(16, 0.5, 0xffffff, 5, 0, 8, 2, Math.PI / 2.4);     // long top strip
  strip(10, 6, cool, 1.6, -9, -2, -4, 0, Math.PI / 2);     // coloured fill from the side, in the cover's colour
  strip(6, 6, hot, 0.9, 9, 4, -6, 0, -Math.PI / 2);
  strip(30, 30, 0x0b0f16, 1, 0, 0, -14);                   // dark backdrop
  { const c = document.createElement('canvas'); c.width = 4; c.height = 256; const x = c.getContext('2d'), g = x.createLinearGradient(0, 0, 0, 256);
    g.addColorStop(0, '#c4cfdd'); g.addColorStop(0.45, '#5a6678'); g.addColorStop(1, '#141a24'); x.fillStyle = g; x.fillRect(0, 0, 4, 256);
    const m = new THREE.Mesh(new THREE.PlaneGeometry(40, 40), new THREE.MeshBasicMaterial({ map: new THREE.CanvasTexture(c), side: THREE.DoubleSide })); m.position.set(0, 0, 18); env.add(m); }   // soft wall behind the viewer
  strip(0.35, 14, 0xffffff, 7, 2.5, 1, 16, 0, Math.PI);      // a thin bright strip behind the viewer: the line of light that runs down each bevel
  const pm = new THREE.PMREMGenerator(r); scene.environment = pm.fromScene(env, 0.01).texture;

  // ---- brushed-steel roughness map: fine scratches running along the blade ----
  const tex = (() => { const c = document.createElement('canvas'); c.width = 64; c.height = 1024; const x = c.getContext('2d');
    x.fillStyle = 'rgb(70,70,70)'; x.fillRect(0, 0, 64, 1024);
    let s = 7; const R = () => (s = (s * 16807) % 2147483647) / 2147483647;
    for (let i = 0; i < 700; i++) { const g = 40 + R() * 80 | 0; x.fillStyle = `rgb(${g},${g},${g})`; x.fillRect(R() * 64, R() * 1024, 0.6 + R() * 0.8, 40 + R() * 400); }
    const t = new THREE.CanvasTexture(c); t.wrapS = t.wrapT = THREE.RepeatWrapping; return t; })();

  const steel = new THREE.MeshPhysicalMaterial({ color: 0xeef1f5, metalness: 1, roughness: 0.2, roughnessMap: tex, clearcoat: 0.35, clearcoatRoughness: 0.12, envMapIntensity: 1.7 });
  const edgeM = new THREE.MeshPhysicalMaterial({ color: 0xf4f7fb, metalness: 1, roughness: 0.06, envMapIntensity: 2.2 });   // honed edge: mirror bright
  const fullerM = new THREE.MeshPhysicalMaterial({ color: 0xaab2bd, metalness: 1, roughness: 0.42, roughnessMap: tex, envMapIntensity: 1.3 });
  const iron = new THREE.MeshPhysicalMaterial({ color: 0xb4bcc8, metalness: 1, roughness: 0.25, envMapIntensity: 2.0 });
  const leather = new THREE.MeshStandardMaterial({ color: 0x241913, roughness: 0.8, metalness: 0 });
  const wire = new THREE.MeshPhysicalMaterial({ color: 0xc9ced6, metalness: 1, roughness: 0.22, envMapIntensity: 1.8 });

  // ---- modelled on a real arming sword (owner's reference): a long blade tapering straight to a needle point,
  //      diamond section with a central ridge, a short fuller under the guard, a slim curved crossguard,
  //      a short black leather grip with one riser, a wheel pommel with a recessed boss, and a small peen cap ----
  const L = 9.6;
  function bladeGeometry() {
    const N = 220, pos = { body: [], edge: [], fuller: [] }, uvs = { body: [], edge: [], fuller: [] };
    const ring = y => { const u = y / L;
      const w = 0.34 * Math.pow(1 - u, 0.82) * (u > 0.93 ? Math.sqrt((1 - u) / 0.07) : 1) + 0.0004;    // steady profile taper, sharpened at the very tip
      const t = 0.085 * (1 - 0.55 * u) + 0.012;                                                         // distal taper
      const f = u < 0.26 ? Math.min(1, u / 0.02) * Math.min(1, (0.26 - u) / 0.07) : 0;                  // short fuller near the guard
      return { w, t, f }; };
    const section = y => { const { w, t, f } = ring(y), e = Math.min(0.07, w * 0.4), fw = 0.06 * f;
      return [[-w, 0], [-w + e, t * 0.32], [-fw * 1.8, t * (1 - 0.15 * f)], [-fw, t - 0.04 * f], [fw, t - 0.04 * f], [fw * 1.8, t * (1 - 0.15 * f)], [w - e, t * 0.32], [w, 0]]; };
    const kind = i => (i === 0 || i === 6) ? 'edge' : (i === 2 || i === 3 || i === 4) ? 'fuller' : 'body';
    for (let k = 0; k < N; k++) {
      const y0 = L * k / N, y1 = L * (k + 1) / N, a = section(y0), b = section(y1);
      for (const side of [1, -1]) for (let i = 0; i < 7; i++) {
        const P = (s, y, j) => [s[j][0], y, side * s[j][1]], A = P(a, y0, i), B = P(a, y0, i + 1), C = P(b, y1, i + 1), D = P(b, y1, i);
        const tri = side > 0 ? [A, B, C, A, C, D] : [A, C, B, A, D, C], k_ = kind(i);
        tri.forEach(v => { pos[k_].push(...v); uvs[k_].push(v[0] + 0.5, v[1] / L); });
      }
    }
    const g = new THREE.BufferGeometry(), all = [], uv = [], groups = [];
    let start = 0; for (const [name, mi] of [['body', 0], ['edge', 1], ['fuller', 2]]) { all.push(...pos[name]); uv.push(...uvs[name]); groups.push([start, pos[name].length / 3, mi]); start += pos[name].length / 3; }
    g.setAttribute('position', new THREE.Float32BufferAttribute(all, 3)); g.setAttribute('uv', new THREE.Float32BufferAttribute(uv, 2));
    groups.forEach(([s, c, m]) => g.addGroup(s, c, m)); g.computeVertexNormals();
    return g;
  }
  const BLADE = bladeGeometry();
  const polished = new THREE.MeshPhysicalMaterial({ color: 0xe9edf2, metalness: 1, roughness: 0.3, clearcoat: 0.6, clearcoatRoughness: 0.1, envMapIntensity: 2.6 });
  const blackLeather = (() => { const c = document.createElement('canvas'); c.width = 128; c.height = 256; const x = c.getContext('2d'); x.fillStyle = '#17171a'; x.fillRect(0, 0, 128, 256);
    let s = 5; const R = () => (s = (s * 16807) % 2147483647) / 2147483647; for (let i = 0; i < 1800; i++) { const v = 10 + R() * 30 | 0; x.fillStyle = `rgb(${v},${v},${v + 2})`; x.fillRect(R() * 128, R() * 256, 1.4, 1.4); }
    const t = new THREE.CanvasTexture(c); t.colorSpace = THREE.SRGBColorSpace; return new THREE.MeshStandardMaterial({ map: t, bumpMap: t, bumpScale: 0.4, roughness: 0.55, metalness: 0 }); })();

  // crossguard: a slim bar, thickest in the middle, thinning and curving gently towards the blade at both ends
  function guardGeometry() {
    const N = 64, half = 1.32, pts = [];
    const sec = u => { const a = Math.abs(u); return { y: 0.2 * Math.pow(a, 2.2), h: 0.085 - 0.035 * a, d: 0.1 - 0.04 * a }; };   // height of the curve, half-height and half-depth of the bar
    const g = new THREE.BufferGeometry(), P = [], idx = [], M = 12;
    for (let i = 0; i <= N; i++) { const u = -1 + 2 * i / N, { y, h, d } = sec(u), x = u * half;
      for (let j = 0; j < M; j++) { const a = j / M * Math.PI * 2, c = Math.cos(a), s_ = Math.sin(a);   // rounded-rectangle-ish section (superellipse)
        const sx = Math.sign(c) * Math.pow(Math.abs(c), 0.5), sy = Math.sign(s_) * Math.pow(Math.abs(s_), 0.5); P.push(x, y + sy * h, sx * d); } }
    for (let i = 0; i < N; i++) for (let j = 0; j < M; j++) { const a = i * M + j, b = i * M + (j + 1) % M, c = a + M, d = b + M; idx.push(a, c, b, b, c, d); }
    g.setAttribute('position', new THREE.Float32BufferAttribute(P, 3)); g.setIndex(idx); g.computeVertexNormals(); return g;
  }
  const GUARD = guardGeometry();

  // ---- fantasy-RPG dressing (owner: 'like Final Fantasy'): gold filigree inlaid in the lower blade, a line of runes that glows
  //      in the cover's colour and flares on the hits, gilded guard and pommel, cut gems, a grip wrapped in dark cord with gold diamonds ----
  const mkTex = (w, h, fn, srgb) => { const c = document.createElement('canvas'); c.width = w; c.height = h; fn(c.getContext('2d'), w, h); const t = new THREE.CanvasTexture(c); if (srgb) t.colorSpace = THREE.SRGBColorSpace; t.anisotropy = 8; return t; };
  const filigree = (x, w, h, col, lw, glowOnly) => {   // canvas top = blade tip; the ornament sits in the lower half, symmetric about the ridge
    const Y = v => (1 - v) * h, X = u => u * w; x.strokeStyle = col; x.fillStyle = col; x.lineCap = 'round'; x.lineJoin = 'round';
    if (!glowOnly) {
      x.lineWidth = lw;
      for (const sd of [-1, 1]) {   // two long tendrils with curls, mirrored
        x.beginPath(); for (let i = 0; i <= 300; i++) { const v = 0.035 + 0.5 * i / 300, a = 0.13 * (1 - i / 300) * (0.55 + 0.45 * Math.sin(i / 300 * Math.PI * 9)); const px = X(0.5 + sd * (0.06 + a)), py = Y(v); i ? x.lineTo(px, py) : x.moveTo(px, py); } x.stroke();
        for (let k = 0; k < 9; k++) { const v = 0.06 + k * 0.05, r = w * 0.05 * (1 - k / 11); x.beginPath(); x.arc(X(0.5 + sd * (0.2 - 0.012 * k)), Y(v), r, sd > 0 ? Math.PI * 0.6 : Math.PI * 0.4, sd > 0 ? Math.PI * 2.1 : -Math.PI * 1.1, sd < 0); x.stroke(); }
      }
      x.lineWidth = lw * 0.8; for (let k = 0; k < 3; k++) { const v = 0.035 + k * 0.012; x.beginPath(); x.moveTo(X(0.18), Y(v)); x.lineTo(X(0.82), Y(v)); x.stroke(); }   // bands at the base
      x.beginPath(); x.moveTo(X(0.5), Y(0.60)); x.lineTo(X(0.47), Y(0.555)); x.lineTo(X(0.5), Y(0.53)); x.lineTo(X(0.53), Y(0.555)); x.closePath(); x.fill();   // finial
    } else {
      x.lineWidth = lw; let s_ = 11; const R = () => (s_ = (s_ * 16807) % 2147483647) / 2147483647;
      for (let k = 0; k < 13; k++) {   // runes down the ridge: small angular glyphs
        const cy = Y(0.085 + k * 0.035), cx = X(0.5), u = w * 0.035; x.beginPath();
        const pts = [[-1, -1], [1, -1], [1, 1], [-1, 1], [0, -1.3], [0, 1.3], [-1, 0], [1, 0]];
        let p0 = pts[Math.floor(R() * 8)]; x.moveTo(cx + p0[0] * u, cy + p0[1] * u);
        for (let j = 0; j < 4; j++) { const q = pts[Math.floor(R() * 8)]; x.lineTo(cx + q[0] * u, cy + q[1] * u); } x.stroke();
      }
      x.lineWidth = lw * 0.6; x.beginPath(); x.moveTo(X(0.5), Y(0.56)); x.lineTo(X(0.5), Y(0.93)); x.stroke();   // a thin line of light up the ridge to the point
    }
  };
  const FANCY = !!opts.fancy;   // gold filigree, runes and gems; off = plain polished steel like the owner's reference sword (the owner found the gold version did not fit)
  if (FANCY) steel.map = mkTex(256, 2048, (x, w, h) => { x.fillStyle = '#ffffff'; x.fillRect(0, 0, w, h); filigree(x, w, h, '#d9a441', 5, false); }, true);
  if (FANCY) steel.emissiveMap = mkTex(256, 2048, (x, w, h) => { x.fillStyle = '#000'; x.fillRect(0, 0, w, h); x.shadowColor = '#fff'; x.shadowBlur = 10; filigree(x, w, h, '#ffffff', 4, true); }, false);
  const gemCol = new THREE.Color(hot); { const hsl = {}; gemCol.getHSL(hsl); gemCol.setHSL(hsl.h, 0.95, 0.55); }
  if (FANCY) { steel.emissive = gemCol.clone(); steel.emissiveIntensity = 0.8; }
  const gold = !FANCY ? polished : new THREE.MeshPhysicalMaterial({ color: 0xf0c060, metalness: 1, roughness: 0.3, clearcoat: 0.5, clearcoatRoughness: 0.1, envMapIntensity: 3.2, emissive: 0x4a3208, emissiveIntensity: 1 });
  const gemM = !FANCY ? polished : new THREE.MeshPhysicalMaterial({ color: gemCol, emissive: gemCol, emissiveIntensity: 1.2, metalness: 0, roughness: 0.04, clearcoat: 1, clearcoatRoughness: 0.02, envMapIntensity: 2.5, flatShading: true });
  const gripM = !FANCY ? blackLeather : new THREE.MeshStandardMaterial({ roughness: 0.6, metalness: 0.15, map: mkTex(256, 256, (x, w, h) => { x.fillStyle = '#10131c'; x.fillRect(0, 0, w, h);
      x.strokeStyle = '#1d2333'; x.lineWidth = 9; for (let k = -8; k < 16; k++) { x.beginPath(); x.moveTo(k * 32, 0); x.lineTo(k * 32 + 256, 256); x.stroke(); x.beginPath(); x.moveTo(k * 32 + 256, 0); x.lineTo(k * 32, 256); x.stroke(); }   // crossed cord
      x.fillStyle = '#d9a441'; for (let i = 0; i < 8; i++) for (let j = 0; j < 8; j++) { const cx = i * 32 + (j % 2 ? 16 : 0), cy = j * 32 + 16; x.beginPath(); x.moveTo(cx, cy - 7); x.lineTo(cx + 5, cy); x.lineTo(cx, cy + 7); x.lineTo(cx - 5, cy); x.closePath(); x.fill(); } }, true) });
  if (FANCY) { gripM.map.wrapS = gripM.map.wrapT = THREE.RepeatWrapping; gripM.map.repeat.set(2, 2); }

  function sword() {
    const g = new THREE.Group();
    g.add(new THREE.Mesh(BLADE, [steel, edgeM, fullerM]));
    // gilded collar over the base of the blade, winged like on fantasy swords, with a gem set in each face
    const collar = new THREE.Shape(); collar.moveTo(-0.42, 0); collar.lineTo(-0.62, 0.34); collar.lineTo(-0.3, 0.3); collar.lineTo(-0.2, 0.62); collar.lineTo(0, 0.95); collar.lineTo(0.2, 0.62); collar.lineTo(0.3, 0.3); collar.lineTo(0.62, 0.34); collar.lineTo(0.42, 0); collar.closePath();
    if (FANCY) { const col = new THREE.Mesh(new THREE.ExtrudeGeometry(collar, { depth: 0.2, bevelEnabled: true, bevelThickness: 0.035, bevelSize: 0.035, bevelSegments: 2 }), gold); col.position.set(0, 0.06, -0.1); g.add(col);
    for (const sz of [-1, 1]) { const gem = new THREE.Mesh(new THREE.OctahedronGeometry(0.15, 0), gemM); gem.scale.set(0.8, 1.35, 0.45); gem.position.set(0, 0.36, sz * 0.15); g.add(gem); } }
    g.add(new THREE.Mesh(GUARD, gold));
    for (const sx of [-1, 1]) { const cap = new THREE.Mesh(FANCY ? new THREE.OctahedronGeometry(0.1, 0) : new THREE.SphereGeometry(0.055, 16, 12), gemM); cap.position.set(sx * 1.32, 0.2, 0); g.add(cap); }   // rounded ends
    // grip: short, slightly waisted, black leather, one riser in the middle
    const gl = 1.55, prof = []; for (let i = 0; i <= 30; i++) { const gy = gl * i / 30, w = Math.abs(gy / gl - 0.5); prof.push(new THREE.Vector2(0.17 - 0.025 * (1 - 2 * w) + 0.02 * (gy < 0.12 ? 1 - gy / 0.12 : 0), -0.09 - gy)); }
    g.add(new THREE.Mesh(new THREE.LatheGeometry(prof, 40), gripM));
    const riser = new THREE.Mesh(new THREE.TorusGeometry(0.158, 0.026, 10, 40), gold); riser.rotation.x = Math.PI / 2; riser.position.y = -0.09 - gl / 2; g.add(riser);
    for (const fy of [-0.1, -0.09 - gl + 0.01]) { const fer = new THREE.Mesh(new THREE.CylinderGeometry(0.2, 0.2, 0.09, 32), gold); fer.position.y = fy; g.add(fer); }
    // wheel pommel: bevelled rim, recessed face with a domed boss, then the peen cap
    const py = -0.09 - gl - 0.31;
    const wheel = new THREE.Mesh(new THREE.LatheGeometry([new THREE.Vector2(0, -0.15), new THREE.Vector2(0.24, -0.15), new THREE.Vector2(0.27, -0.09), new THREE.Vector2(0.36, -0.06), new THREE.Vector2(0.38, 0), new THREE.Vector2(0.36, 0.06), new THREE.Vector2(0.27, 0.09), new THREE.Vector2(0.24, 0.15), new THREE.Vector2(0, 0.15)], 64), gold);
    wheel.rotation.x = Math.PI / 2; wheel.position.y = py; g.add(wheel);
    for (const sz of [-1, 1]) { const boss = new THREE.Mesh(new THREE.SphereGeometry(0.2, FANCY ? 8 : 32, FANCY ? 3 : 16, 0, Math.PI * 2, 0, Math.PI / 2), gemM); boss.scale.set(1, FANCY ? 0.5 : 0.35, 1); boss.rotation.x = sz * Math.PI / 2; boss.position.set(0, py, sz * 0.12); g.add(boss); }
    const peen = new THREE.Mesh(new THREE.CylinderGeometry(0.07, 0.1, 0.14, 24), gold); peen.position.y = py - 0.44; g.add(peen);
    const dome = new THREE.Mesh(new THREE.SphereGeometry(0.07, 20, 10, 0, Math.PI * 2, 0, Math.PI / 2), gold); dome.rotation.x = Math.PI; dome.position.y = py - 0.51; g.add(dome);
    const outer = new THREE.Group(); g.position.y = -3.4; outer.add(g);   // pivot near the middle of the whole sword
    return outer;
  }

  // a fan of blades behind the card, like a throne of swords: hilts hidden behind the card, blades radiating up and out.
  // Before the drop only the points peek out; on the drop the fan snaps open, and every hit thrusts the blades outward in a wave.
  const NS = 9, S = []; for (let i = 0; i < NS; i++) { const th = (-112 + 224 * i / (NS - 1)) * Math.PI / 180; S.push({ g: sword(), th, sc: 0.6 - 0.07 * Math.abs(i - (NS - 1) / 2) / ((NS - 1) / 2) }); }
  if (solo) { S.length = 1; S[0].th = 0; S[0].sc = 1.4; S[0].solo = true; }
  S.forEach(o => { o.g.scale.setScalar(o.sc); scene.add(o.g); });
  scene.fog = new THREE.Fog(0x05070b, 22, 44);
  const key = new THREE.DirectionalLight(0xffffff, 1.4); key.position.set(-5, 9, 10); scene.add(key);
  const rim = new THREE.DirectionalLight(new THREE.Color(cool), 1.2); rim.position.set(6, -3, -6); scene.add(rim);
  const flash = new THREE.PointLight(new THREE.Color(cool).lerp(new THREE.Color(0xffffff), 0.65), 0, 60, 1.2); flash.position.set(3, 6, 9); scene.add(flash);

  const drop = opts.drop ?? 0, eob = x => { x = Math.min(1, Math.max(0, x)); const c = 2.2; return 1 + (c + 1) * Math.pow(x - 1, 3) + c * Math.pow(x - 1, 2); };
  return function draw(t, hit, bn, tr = t) {
    const open = eob((tr - drop) / 0.45), sway = 0.05 * Math.sin(tr * 0.5);
    S.forEach((o, i) => {
      if (o.solo) { o.g.position.set(0, 0, 0); o.g.rotation.set(0, 0.6 + t * 0.3, 0); return; }
      const wave = Math.max(0, Math.sin(tr * 7 - i * 0.7)) * 0.25 * hit, R = 1.2 + 2.5 * open + 0.55 * bn + wave, th = o.th * (0.72 + 0.28 * open) + sway;
      o.g.position.set(Math.sin(th) * R, 0.9 + Math.cos(th) * R, -3 - 0.12 * Math.abs(i - (NS - 1) / 2));
      o.g.rotation.set(0, 0.5 * Math.sin(t * 0.6 + i * 0.9), -th);   // each blade rolls a little on its own axis, so the light runs along it
    });
    if (FANCY) { steel.emissiveIntensity = 0.8 + 2.6 * hit; gemM.emissiveIntensity = 1.1 + 2.8 * hit; }
    flash.intensity = 300 * hit * hit; key.intensity = 1.2 + 0.7 * hit;
    r.render(scene, cam);
  };
}
