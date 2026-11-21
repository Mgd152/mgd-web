// Real 3D swords (three.js) for songs about battle: two longswords crossed behind the card and a few more hanging in the depth.
// Physically based steel with an environment map, so the blades catch light like metal; the hits flash a cold light across them.
export async function makeSwords(THREE, canvas, opts) {
  const { W, H, cool, hot } = opts;
  const r = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, preserveDrawingBuffer: true });
  r.setPixelRatio(1); r.setSize(W, H, false); r.setClearColor(0x000000, 0);
  r.toneMapping = THREE.ACESFilmicToneMapping; r.toneMappingExposure = 1.6; r.outputColorSpace = THREE.SRGBColorSpace;
  const scene = new THREE.Scene();
  const cam = new THREE.PerspectiveCamera(30, W / H, 0.1, 100); cam.position.set(0, 0, 22);

  // environment: a dark room with a few bright cool panels -> crisp reflections on the steel
  const envScene = new THREE.Scene(); envScene.background = new THREE.Color(0x05070b);
  const panel = (w, h, col, x, y, z, ry = 0, rx = 0) => { const m = new THREE.Mesh(new THREE.PlaneGeometry(w, h), new THREE.MeshBasicMaterial({ color: col, side: THREE.DoubleSide })); m.position.set(x, y, z); m.rotation.set(rx, ry, 0); envScene.add(m); };
  panel(14, 5, 0xffffff, 0, 9, 0, 0, Math.PI / 2); panel(16, 12, 0x2a3442, 0, 0, 9);  panel(3, 10, new THREE.Color(cool).multiplyScalar(2.5), -9, 0, 0, Math.PI / 2); panel(3, 10, new THREE.Color(hot).multiplyScalar(1.6), 9, 1, -2, -Math.PI / 2); panel(10, 2, 0x8899aa, 0, -2, -9);
  const pm = new THREE.PMREMGenerator(r); scene.environment = pm.fromScene(envScene, 0.02).texture;

  const steel = new THREE.MeshPhysicalMaterial({ color: 0xe8edf3, metalness: 1, roughness: 0.3, clearcoat: 0.5, clearcoatRoughness: 0.2, envMapIntensity: 1.8 });
  const dark = new THREE.MeshPhysicalMaterial({ color: 0x6a7280, metalness: 1, roughness: 0.4, envMapIntensity: 1.5 });
  const brass = new THREE.MeshPhysicalMaterial({ color: 0xa8b4c4, metalness: 1, roughness: 0.32, envMapIntensity: 1.5 });
  const leather = new THREE.MeshStandardMaterial({ color: 0x1a1410, roughness: 0.75, metalness: 0 });

  function sword() {
    const g = new THREE.Group();
    // blade: a long diamond section that narrows to the point, bevelled so the edges catch the light
    const L = 9, w0 = 0.42, s = new THREE.Shape();
    s.moveTo(-w0, 0); s.lineTo(-w0 * 0.86, L * 0.82); s.quadraticCurveTo(-w0 * 0.6, L * 0.95, 0, L); s.quadraticCurveTo(w0 * 0.6, L * 0.95, w0 * 0.86, L * 0.82); s.lineTo(w0, 0); s.closePath();
    const blade = new THREE.Mesh(new THREE.ExtrudeGeometry(s, { depth: 0.04, bevelEnabled: true, bevelThickness: 0.07, bevelSize: 0.16, bevelSegments: 3, curveSegments: 10 }), steel);
    blade.position.z = -0.02; g.add(blade);
    const fuller = new THREE.Mesh(new THREE.BoxGeometry(0.1, L * 0.62, 0.14), dark); fuller.position.set(0, L * 0.34, 0); g.add(fuller);   // the groove down the middle
    const guard = new THREE.Mesh(new THREE.BoxGeometry(3.0, 0.32, 0.38), dark); guard.position.y = -0.12; g.add(guard);
    for (const sx of [-1, 1]) { const tip = new THREE.Mesh(new THREE.SphereGeometry(0.24, 20, 14), brass); tip.position.set(sx * 1.55, -0.12, 0); g.add(tip); }
    const grip = new THREE.Mesh(new THREE.CylinderGeometry(0.2, 0.22, 2.1, 20), leather); grip.position.y = -1.35; g.add(grip);
    for (let k = 0; k < 6; k++) { const wrap = new THREE.Mesh(new THREE.TorusGeometry(0.215, 0.035, 8, 24), brass); wrap.rotation.x = Math.PI / 2; wrap.position.y = -0.5 - k * 0.34; g.add(wrap); }
    const pommel = new THREE.Mesh(new THREE.SphereGeometry(0.42, 24, 18), brass); pommel.scale.set(1, 0.8, 0.7); pommel.position.y = -2.65; g.add(pommel);
    g.traverse(o => o.isMesh && o.geometry.translate(0, -3.2, 0));   // pivot near the middle of the sword
    return g;
  }
  // two crossed in the middle, three more scattered further back
  const S = [
    { g: sword(), p: [0, -0.2, -2.5], rz: 0.5, ry: 0.0, sc: 1.05 }, { g: sword(), p: [0, -0.2, -2.9], rz: -0.5, ry: 0.0, sc: 1.05 },
    { g: sword(), p: [-5.6, 6.5, -9], rz: 2.6, ry: 0.5, sc: 0.75 }, { g: sword(), p: [6, -6.8, -8], rz: -2.4, ry: -0.4, sc: 0.8 }, { g: sword(), p: [5.4, 7.6, -14], rz: 0.4, ry: 0.9, sc: 0.7 },
  ];
  S.forEach(o => { o.g.scale.setScalar(o.sc); scene.add(o.g); });
  scene.fog = new THREE.Fog(0x05070b, 20, 42);
  const key = new THREE.DirectionalLight(0xffffff, 1.2); key.position.set(-4, 8, 10); scene.add(key);
  const flash = new THREE.PointLight(new THREE.Color(cool).lerp(new THREE.Color(0xffffff), 0.6), 0, 60, 1.2); flash.position.set(3, 6, 8); scene.add(flash);
  const rim = new THREE.DirectionalLight(new THREE.Color(hot), 0.8); rim.position.set(6, -2, -6); scene.add(rim); scene.add(new THREE.HemisphereLight(0xcfe2ff, 0x10141c, 0.9));

  return function draw(t, hit, bn) {
    S.forEach((o, i) => {
      const far = i > 1, ph = i * 1.9;
      o.g.position.set(o.p[0] + (far ? Math.sin(t * 0.25 + ph) * 0.8 : 0), o.p[1] + (far ? Math.cos(t * 0.21 + ph) * 0.7 : 0.15 * Math.sin(t * 0.9)) + (far ? 0 : 0.25 * bn), o.p[2]);
      o.g.rotation.set(0.12 * Math.sin(t * 0.4 + ph), o.ry + (far ? t * 0.35 : 0.35 * Math.sin(t * 0.45 + i * Math.PI)), o.rz + (far ? t * 0.08 * (i % 2 ? 1 : -1) : (i ? -1 : 1) * 0.04 * bn));
    });
    flash.intensity = 260 * hit * hit; key.intensity = 1.0 + 0.6 * hit;
    r.render(scene, cam);
  };
}
