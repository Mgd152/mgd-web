// A real 3D model (glTF) shown under the card: lit like a product shot, turning slowly, bouncing and flashing with the hits.
// Models used so far are Khronos glTF sample assets (CC BY 4.0, Eric Chadwick / Darmstadt Graphics Group): credit them in the post.
export async function makeModel(THREE, GLTFLoader, canvas, opts) {
  const { W, H, url, cool, hot, y = -3.3, size = 5.2, spin = 0.5, yaw = 0.6, pitch = 0.12 } = opts;
  const r = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, preserveDrawingBuffer: true });
  r.setPixelRatio(1); r.setSize(W, H, false); r.setClearColor(0x000000, 0);
  r.toneMapping = THREE.ACESFilmicToneMapping; r.toneMappingExposure = 1.15; r.outputColorSpace = THREE.SRGBColorSpace;
  const scene = new THREE.Scene(), cam = new THREE.PerspectiveCamera(28, W / H, 0.1, 200); cam.position.set(0, 0, 24);
  // studio environment: soft boxes above and at the sides, a coloured kicker in the cover's colours
  const env = new THREE.Scene(); env.background = new THREE.Color(0x05060a);
  const box = (w, h, col, k, x, y_, z, rx = 0, ry = 0) => { const m = new THREE.Mesh(new THREE.PlaneGeometry(w, h), new THREE.MeshBasicMaterial({ color: new THREE.Color(col).multiplyScalar(k), side: THREE.DoubleSide })); m.position.set(x, y_, z); m.rotation.set(rx, ry, 0); env.add(m); };
  box(18, 8, 0xffffff, 5, 0, 10, 0, Math.PI / 2); box(3, 12, 0xffffff, 4, -10, 2, 4, 0, Math.PI / 2.6); box(3, 12, 0xffffff, 3, 10, 2, 4, 0, -Math.PI / 2.6);
  box(12, 6, cool, 2.2, -8, -1, -8, 0, Math.PI / 3); box(12, 6, hot, 1.6, 8, 0, -8, 0, -Math.PI / 3); box(40, 40, 0x1a2230, 1, 0, 0, 20);
  scene.environment = new THREE.PMREMGenerator(r).fromScene(env, 0.02).texture;
  const gltf = await new GLTFLoader().loadAsync(url), model = gltf.scene, bb = new THREE.Box3().setFromObject(model), sz = bb.getSize(new THREE.Vector3()), c = bb.getCenter(new THREE.Vector3());
  if (opts.tint) model.traverse(o => { const m = o.material; if (!m || !m.color) return; const h = {}; m.color.getHSL(h);   // repaint the saturated, clear-coated parts (car paint) in the cover's colour
    if (h.s > 0.45 && (m.clearcoat > 0 || m.metalness > 0.3)) { const t = {}; new THREE.Color(opts.tint).getHSL(t); m.color.setHSL(t.h, Math.min(1, t.s * 1.1), Math.min(0.5, Math.max(0.28, h.l))); } });
  model.position.sub(c); const pivot = new THREE.Group(); pivot.add(model); pivot.scale.setScalar(size / Math.max(sz.x, sz.z, sz.y)); scene.add(pivot);
  const key = new THREE.DirectionalLight(0xffffff, 1.6); key.position.set(-6, 10, 12); scene.add(key);
  const rim = new THREE.DirectionalLight(new THREE.Color(hot), 1.4); rim.position.set(8, 3, -10); scene.add(rim);
  const flash = new THREE.PointLight(new THREE.Color(hot).lerp(new THREE.Color(0xffffff), 0.5), 0, 80, 1.2); flash.position.set(0, y + 5, 10); scene.add(flash);
  return function draw(t, hit, bn) {
    pivot.position.set(0, y + 0.1 * Math.sin(t * 0.9) + 0.22 * bn, 0);
    pivot.rotation.set(pitch + 0.03 * Math.sin(t * 0.6), yaw + t * spin, 0.02 * Math.sin(t * 0.8));
    const s = size / Math.max(sz.x, sz.z, sz.y) * (1 + 0.05 * bn); pivot.scale.setScalar(s);
    flash.intensity = 380 * hit * hit; key.intensity = 1.4 + 0.8 * hit;
    r.render(scene, cam);
  };
}
