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
    g.addColorStop(0, '#9aa6b8'); g.addColorStop(0.45, '#3a4556'); g.addColorStop(1, '#0c1018'); x.fillStyle = g; x.fillRect(0, 0, 4, 256);
    const m = new THREE.Mesh(new THREE.PlaneGeometry(40, 40), new THREE.MeshBasicMaterial({ map: new THREE.CanvasTexture(c), side: THREE.DoubleSide })); m.position.set(0, 0, 18); env.add(m); }   // soft wall behind the viewer
  strip(0.35, 14, 0xffffff, 7, 2.5, 1, 16, 0, Math.PI);      // a thin bright strip behind the viewer: the line of light that runs down each bevel
  const pm = new THREE.PMREMGenerator(r); scene.environment = pm.fromScene(env, 0.01).texture;

  // ---- brushed-steel roughness map: fine scratches running along the blade ----
  const tex = (() => { const c = document.createElement('canvas'); c.width = 64; c.height = 1024; const x = c.getContext('2d');
    x.fillStyle = 'rgb(70,70,70)'; x.fillRect(0, 0, 64, 1024);
    let s = 7; const R = () => (s = (s * 16807) % 2147483647) / 2147483647;
    for (let i = 0; i < 700; i++) { const g = 40 + R() * 80 | 0; x.fillStyle = `rgb(${g},${g},${g})`; x.fillRect(R() * 64, R() * 1024, 0.6 + R() * 0.8, 40 + R() * 400); }
    const t = new THREE.CanvasTexture(c); t.wrapS = t.wrapT = THREE.RepeatWrapping; return t; })();

  const steel = new THREE.MeshPhysicalMaterial({ color: 0xe6eaf0, metalness: 1, roughness: 0.32, roughnessMap: tex, clearcoat: 0.35, clearcoatRoughness: 0.12, envMapIntensity: 1.7 });
  const edgeM = new THREE.MeshPhysicalMaterial({ color: 0xf4f7fb, metalness: 1, roughness: 0.06, envMapIntensity: 2.2 });   // honed edge: mirror bright
  const fullerM = new THREE.MeshPhysicalMaterial({ color: 0xaab2bd, metalness: 1, roughness: 0.42, roughnessMap: tex, envMapIntensity: 1.3 });
  const iron = new THREE.MeshPhysicalMaterial({ color: 0xb4bcc8, metalness: 1, roughness: 0.25, envMapIntensity: 2.0 });
  const leather = new THREE.MeshStandardMaterial({ color: 0x241913, roughness: 0.8, metalness: 0 });
  const wire = new THREE.MeshPhysicalMaterial({ color: 0xc9ced6, metalness: 1, roughness: 0.22, envMapIntensity: 1.8 });

  // ---- blade: built ring by ring along its length; flat-shaded facets so the bevels read as hard, sharp planes ----
  function bladeGeometry() {
    const L = 9.2, N = 200, pos = { body: [], edge: [], fuller: [] }, uvs = { body: [], edge: [], fuller: [] };
    const ring = y => {   // half-width, half-thickness, fuller depth at height y
      const u = y / L, base = 0.46 - 0.1 * u, tp = u < 0.66 ? 1 : Math.pow(Math.max(0, Math.cos((u - 0.66) / 0.34 * Math.PI / 2)), 0.75), w = base * tp + 0.0005;   // straight taper, then an ogival point
      const t = 0.11 * (1 - 0.6 * u), f = u < 0.68 ? Math.min(1, u / 0.04) * Math.min(1, (0.68 - u) / 0.08) : 0;
      return { w, t, f };
    };
    // cross-section, left to right on the front face (z >= 0); the back face is mirrored
    const section = (y) => { const { w, t, f } = ring(y), e = Math.min(0.11, w * 0.45), fw = 0.11 * f;
      return [[-w, 0], [-w + e, t * 0.55], [-fw * 1.9, t], [-fw, t - 0.05 * f], [fw, t - 0.05 * f], [fw * 1.9, t], [w - e, t * 0.55], [w, 0]]; };
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
    groups.forEach(([s, c, m]) => g.addGroup(s, c, m)); g.computeVertexNormals();   // non-indexed: every facet keeps its own normal
    return g;
  }
  const BLADE = bladeGeometry();

  function sword() {
    const g = new THREE.Group();
    g.add(new THREE.Mesh(BLADE, [steel, edgeM, fullerM]));
    // ricasso block where the blade meets the guard
    const ric = new THREE.Mesh(new THREE.BoxGeometry(0.62, 0.34, 0.26), iron); ric.position.y = 0.1; g.add(ric);
    // crossguard: a bar of rounded section swept along a gentle curve, with a central block and quillon terminals
    const path = new THREE.CatmullRomCurve3([new THREE.Vector3(-1.7, 0.22, 0), new THREE.Vector3(-0.8, -0.02, 0), new THREE.Vector3(0, -0.08, 0), new THREE.Vector3(0.8, -0.02, 0), new THREE.Vector3(1.7, 0.22, 0)]);
    const sec = new THREE.Shape(); { const a = 0.15, b = 0.11, rr = 0.05; sec.moveTo(-a + rr, -b); sec.lineTo(a - rr, -b); sec.quadraticCurveTo(a, -b, a, -b + rr); sec.lineTo(a, b - rr); sec.quadraticCurveTo(a, b, a - rr, b); sec.lineTo(-a + rr, b); sec.quadraticCurveTo(-a, b, -a, b - rr); sec.lineTo(-a, -b + rr); sec.quadraticCurveTo(-a, -b, -a + rr, -b); }
    g.add(new THREE.Mesh(new THREE.ExtrudeGeometry(sec, { steps: 80, extrudePath: path, bevelEnabled: false }), iron));
    const block = new THREE.Mesh(new THREE.CylinderGeometry(0.2, 0.2, 0.36, 8), iron); block.rotation.x = Math.PI / 2; block.position.y = -0.08; g.add(block);
    for (const sx of [-1, 1]) { const q = new THREE.Mesh(new THREE.SphereGeometry(0.2, 24, 16), iron); q.scale.set(1, 1.2, 0.85); q.position.set(sx * 1.72, 0.26, 0); g.add(q); }
    // grip: a tapered wooden core wrapped in dark leather: overlapping diagonal turns of strap, with two raised risers
    const gripLen = 1.9, prof = []; for (let i = 0; i <= 40; i++) { const gy = gripLen * i / 40, rr = gy < 0.95 ? 0.18 + 0.05 * gy / 0.95 : 0.23 - 0.04 * (gy - 0.95) / 0.95; prof.push(new THREE.Vector2(rr, -gy)); }
    const leatherTex = (() => { const c = document.createElement('canvas'); c.width = 256; c.height = 512; const x = c.getContext('2d');
      x.fillStyle = '#4a3324'; x.fillRect(0, 0, 256, 512);
      for (let k = -6; k < 20; k++) { const y = k * 34; const g = x.createLinearGradient(0, y, 0, y + 34); g.addColorStop(0, '#120c08'); g.addColorStop(0.25, '#6a4a33'); g.addColorStop(0.8, '#4a3324'); g.addColorStop(1, '#0d0906');   // one turn of strap, shaded at the overlap
        x.save(); x.translate(0, y); x.transform(1, 0.35, 0, 1, 0, 0); x.fillStyle = g; x.fillRect(-40, 0, 340, 34); x.restore(); }
      let s = 3; const R = () => (s = (s * 16807) % 2147483647) / 2147483647; for (let i = 0; i < 2500; i++) { x.fillStyle = `rgba(0,0,0,${0.08 + R() * 0.12})`; x.fillRect(R() * 256, R() * 512, 1.5, 1.5); }   // leather grain
      const t = new THREE.CanvasTexture(c); t.colorSpace = THREE.SRGBColorSpace; t.wrapS = t.wrapT = THREE.RepeatWrapping; t.repeat.set(2, 1); return t; })();
    const leatherWrap = new THREE.MeshStandardMaterial({ map: leatherTex, bumpMap: leatherTex, bumpScale: 0.6, roughness: 0.72, metalness: 0 });
    const grip = new THREE.Mesh(new THREE.LatheGeometry(prof, 48), leatherWrap); grip.position.y = -0.33; g.add(grip);
    for (const gy of [0.62, 1.28]) { const riser = new THREE.Mesh(new THREE.TorusGeometry(0.232, 0.028, 10, 40), leatherWrap); riser.rotation.x = Math.PI / 2; riser.position.y = -0.33 - gy; g.add(riser); }
    for (const y of [-0.36, -2.22]) { const ferrule = new THREE.Mesh(new THREE.CylinderGeometry(0.25, 0.25, 0.12, 32), iron); ferrule.position.y = y; g.add(ferrule); }
    // wheel pommel with a bevelled rim and a peen block
    const wheel = new THREE.Mesh(new THREE.LatheGeometry([new THREE.Vector2(0, -0.17), new THREE.Vector2(0.32, -0.17), new THREE.Vector2(0.44, -0.08), new THREE.Vector2(0.44, 0.08), new THREE.Vector2(0.32, 0.17), new THREE.Vector2(0, 0.17)], 48), iron);
    wheel.rotation.x = Math.PI / 2; wheel.position.y = -2.62; g.add(wheel);
    const boss = new THREE.Mesh(new THREE.CylinderGeometry(0.16, 0.2, 0.42, 24), steel); boss.rotation.x = Math.PI / 2; boss.position.y = -2.62; g.add(boss);
    const peen = new THREE.Mesh(new THREE.BoxGeometry(0.16, 0.16, 0.16), iron); peen.position.y = -3.1; g.add(peen);
    const outer = new THREE.Group(); g.position.y = -3.0; outer.add(g);   // pivot near the middle of the whole sword
    return outer;
  }

  // two crossed behind the card, three further back and smaller (fog takes them into the dark)
  const S = [
    { g: sword(), p: [0, -0.2, -2.5], rz: 0.5, ry: 0.0, sc: 1.05 }, { g: sword(), p: [0, -0.2, -2.9], rz: -0.5, ry: 0.0, sc: 1.05 },
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
