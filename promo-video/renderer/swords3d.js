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

  function sword() {
    const g = new THREE.Group();
    g.add(new THREE.Mesh(BLADE, [steel, edgeM, fullerM]));
    g.add(new THREE.Mesh(GUARD, polished));
    for (const sx of [-1, 1]) { const cap = new THREE.Mesh(new THREE.SphereGeometry(0.055, 16, 12), polished); cap.position.set(sx * 1.32, 0.2, 0); g.add(cap); }   // rounded ends
    const lang = new THREE.Mesh(new THREE.ConeGeometry(0.16, 0.34, 4), polished); lang.rotation.set(0, Math.PI / 4, 0); lang.scale.set(1, 1, 0.55); lang.position.y = 0.2; g.add(lang);   // the small point the guard makes over the blade
    // grip: short, slightly waisted, black leather, one riser in the middle
    const gl = 1.55, prof = []; for (let i = 0; i <= 30; i++) { const gy = gl * i / 30, w = Math.abs(gy / gl - 0.5); prof.push(new THREE.Vector2(0.17 - 0.025 * (1 - 2 * w) + 0.02 * (gy < 0.12 ? 1 - gy / 0.12 : 0), -0.09 - gy)); }
    g.add(new THREE.Mesh(new THREE.LatheGeometry(prof, 40), blackLeather));
    const riser = new THREE.Mesh(new THREE.TorusGeometry(0.155, 0.022, 10, 40), blackLeather); riser.rotation.x = Math.PI / 2; riser.position.y = -0.09 - gl / 2; g.add(riser);
    // wheel pommel: bevelled rim, recessed face with a domed boss, then the peen cap
    const py = -0.09 - gl - 0.31;
    const wheel = new THREE.Mesh(new THREE.LatheGeometry([new THREE.Vector2(0, -0.15), new THREE.Vector2(0.24, -0.15), new THREE.Vector2(0.27, -0.09), new THREE.Vector2(0.36, -0.06), new THREE.Vector2(0.38, 0), new THREE.Vector2(0.36, 0.06), new THREE.Vector2(0.27, 0.09), new THREE.Vector2(0.24, 0.15), new THREE.Vector2(0, 0.15)], 64), polished);
    wheel.rotation.x = Math.PI / 2; wheel.position.y = py; g.add(wheel);
    for (const sz of [-1, 1]) { const boss = new THREE.Mesh(new THREE.SphereGeometry(0.2, 32, 16, 0, Math.PI * 2, 0, Math.PI / 2), polished); boss.scale.set(1, 0.35, 1); boss.rotation.x = sz * Math.PI / 2; boss.position.set(0, py, sz * 0.12); g.add(boss); }
    const peen = new THREE.Mesh(new THREE.CylinderGeometry(0.07, 0.1, 0.14, 24), polished); peen.position.y = py - 0.44; g.add(peen);
    const dome = new THREE.Mesh(new THREE.SphereGeometry(0.07, 20, 10, 0, Math.PI * 2, 0, Math.PI / 2), polished); dome.rotation.x = Math.PI; dome.position.y = py - 0.51; g.add(dome);
    const outer = new THREE.Group(); g.position.y = -3.4; outer.add(g);   // pivot near the middle of the whole sword
    return outer;
  }

  // two crossed behind the card, three further back and smaller (fog takes them into the dark)
  const S = [
    { g: sword(), p: [0, 0.7, -2.5], rz: 0.46, ry: 0.0, sc: 1.05 }, { g: sword(), p: [0, 0.7, -2.9], rz: -0.46, ry: 0.0, sc: 1.05 },
    { g: sword(), p: [-5.6, 6.5, -9], rz: 2.6, ry: 0.5, sc: 0.75 }, { g: sword(), p: [6, -6.8, -8], rz: -2.4, ry: -0.4, sc: 0.8 }, { g: sword(), p: [5.4, 7.6, -14], rz: 0.4, ry: 0.9, sc: 0.7 },
  ];
  if (solo) { S.length = 1; S[0].p = [0, 0, 0]; S[0].rz = 0.0; S[0].ry = 0.6; S[0].sc = 1.4; }   // test page: one sword, upright
  S.forEach(o => { o.g.scale.setScalar(o.sc); scene.add(o.g); });
  scene.fog = new THREE.Fog(0x05070b, 22, 44);
  const key = new THREE.DirectionalLight(0xffffff, 1.4); key.position.set(-5, 9, 10); scene.add(key);
  const rim = new THREE.DirectionalLight(new THREE.Color(cool), 1.2); rim.position.set(6, -3, -6); scene.add(rim);
  const flash = new THREE.PointLight(new THREE.Color(cool).lerp(new THREE.Color(0xffffff), 0.65), 0, 60, 1.2); flash.position.set(3, 6, 9); scene.add(flash);

  return function draw(t, hit, bn) {
    S.forEach((o, i) => {
      const far = i > 1, ph = i * 1.9;
      o.g.position.set(o.p[0] + (far ? Math.sin(t * 0.25 + ph) * 0.8 : 0), o.p[1] + (far ? Math.cos(t * 0.21 + ph) * 0.7 : 0.15 * Math.sin(t * 0.9)) + (far ? 0 : 0.25 * bn), o.p[2]);
      // the crossed pair turns a little on its long axis, so the strip lights run up and down the blades
      o.g.rotation.set(0.1 * Math.sin(t * 0.4 + ph), o.ry + (far ? t * 0.35 : 0.42 * Math.sin(t * 0.5 + i * Math.PI)), o.rz + (far ? t * 0.08 * (i % 2 ? 1 : -1) : (i ? -1 : 1) * 0.04 * bn));
    });
    flash.intensity = 300 * hit * hit; key.intensity = 1.2 + 0.7 * hit;
    r.render(scene, cam);
  };
}
