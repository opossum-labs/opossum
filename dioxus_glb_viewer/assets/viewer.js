/**
 * dioxus_glb_viewer — three.js viewer module
 *
 * No importmap needed: three.js addons use relative imports, and this module
 * receives the absolute base URL of the three.js directory as `threeBase`.
 *
 * Loaded via `await import(viewerUrl)` from the boot script.
 * The browser caches the module so multiple GlbViewer instances share the same module namespace.
 *
 * Exports: createViewer(canvasId, options, send, threeBase) -> Viewer object
 */

// ─── Entry point ─────────────────────────────────────────────────────────────

/**
 * Creates a viewer instance for a canvas.
 *
 * @param {string}   canvasId  - DOM ID of the <canvas> element (polled until available).
 * @param {object}   options   - ViewerOptions object from Rust (snake_case fields).
 * @param {function} send      - Callback for sending events to Rust: send({event, ...}).
 * @param {string}   threeBase - Absolute URL of the three.js asset directory.
 * @returns {Promise<object>}  - Viewer object with `.apply(op)` and `.dispose()`.
 */
export async function createViewer(canvasId, options, send, threeBase) {
    const THREE          = await import(threeBase + '/three.module.js');
    const { OrbitControls } = await import(threeBase + '/examples/jsm/controls/OrbitControls.js');
    const { GLTFLoader }    = await import(threeBase + '/examples/jsm/loaders/GLTFLoader.js');
    const canvas = await waitForElement(canvasId, 10_000);
    if (!canvas) {
        throw new Error(`Canvas #${canvasId} not found after 10 s`);
    }

    // ── Check for WebGL context ─────────────────────────────────────────────
    if (!canvas.getContext('webgl2') && !canvas.getContext('webgl')) {
        throw new Error('WebGL not available (VM, RDP, or outdated graphics driver?)');
    }

    // ── Renderer ────────────────────────────────────────────────────────────
    const renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
    renderer.setPixelRatio(Math.min(typeof window !== 'undefined' ? (window.devicePixelRatio || 1) : 1, 2));

    // ── Scene, camera, controls ─────────────────────────────────────────────
    const scene    = new THREE.Scene();
    scene.background = new THREE.Color(options.background);

    const camera = new THREE.PerspectiveCamera(options.fov_degrees, 1, 0.01, 10_000);
    camera.position.set(...options.initial_camera);

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.target.set(...options.initial_target);
    controls.enableDamping = false;
    controls.update();

    // ── Lights ──────────────────────────────────────────────────────────────
    const ambient = new THREE.AmbientLight(0xffffff, options.ambient_intensity);
    const sun     = new THREE.DirectionalLight(0xffffff, options.directional_intensity);
    sun.position.set(1, 2, 1.5);
    scene.add(ambient, sun);

    // ── Grid ────────────────────────────────────────────────────────────────
    let grid = null;
    if (options.grid) {
        grid = new THREE.GridHelper(20, 20);
        scene.add(grid);
    }

    // ── GLTFLoader ──────────────────────────────────────────────────────────
    const loader = new GLTFLoader();

    // ── Picking infrastructure ───────────────────────────────────────────────
    const raycaster = new THREE.Raycaster();

    // ── Instance state ──────────────────────────────────────────────────────
    /** id -> { root: THREE.Group, helper: THREE.BoxHelper|null } */
    const objects      = new Map();
    /** id -> generation counter (prevents stale load callbacks) */
    const generations  = new Map();
    /** transport key -> array of Base64 chunk strings */
    const pendingBytes = new Map();

    let hasAutoFitted = false;
    let needsResize   = true;
    let disposed      = false;

    // AbortController for all DOM listeners (ac.abort() removes them all at once)
    const ac = new AbortController();

    // ── Resize via ResizeObserver (NOT window.resize) ───────────────────────
    // window.resize does not fire when a splitter narrows the panel.
    const ro = new ResizeObserver(() => { needsResize = true; });
    ro.observe(canvas);

    function resize() {
        const w = Math.max(1, Math.floor(canvas.clientWidth));
        const h = Math.max(1, Math.floor(canvas.clientHeight));
        // false = no inline CSS: three would otherwise set width/height attributes which
        // would re-trigger ResizeObserver → feedback loop.
        renderer.setSize(w, h, false);
        camera.aspect = w / h;
        camera.updateProjectionMatrix();
        needsResize = false;
    }

    // ── RAF loop ─────────────────────────────────────────────────────────────
    renderer.setAnimationLoop(() => {
        if (disposed) return;
        // Self-shutdown: canvas detached from DOM (unmount without a Destroy op).
        if (!canvas.isConnected) { dispose(); return; }
        if (needsResize) resize();
        controls.update(); // needed for enableDamping
        renderer.render(scene, camera);
    });

    // ── Picking ───────────────────────────────────────────────────────────────
    let pointerDown = null;

    canvas.addEventListener('pointerdown', (e) => {
        if (e.button !== 0) return;
        pointerDown = { x: e.clientX, y: e.clientY, id: e.pointerId };
    }, { signal: ac.signal });

    canvas.addEventListener('pointercancel', () => { pointerDown = null; }, { signal: ac.signal });

    canvas.addEventListener('pointerup', (e) => {
        const start = pointerDown;
        pointerDown = null;
        if (!start || e.pointerId !== start.id || e.button !== 0) return;
        // Distance threshold 4 px: orbit drags are not a pick.
        // Time threshold intentionally NOT used — a slow deliberate click is a click.
        if (Math.hypot(e.clientX - start.x, e.clientY - start.y) > 4) return;

        // NDC from CSS box (not canvas.width): at devicePixelRatio != 1 the values would otherwise be wrong.
        const rect = canvas.getBoundingClientRect();
        const ndc  = new THREE.Vector2(
             ((e.clientX - rect.left) / rect.width)  * 2 - 1,
            -((e.clientY - rect.top)  / rect.height) * 2 + 1,
        );
        raycaster.setFromCamera(ndc, camera);

        // Only raycast against model roots (NOT scene.children, because GridHelper is also
        // raycastable and would produce hits without a glbId).
        const roots = [];
        for (const entry of objects.values()) {
            if (entry.root.visible) roots.push(entry.root);
        }
        const hit = raycaster.intersectObjects(roots, true)[0];

        if (hit) {
            send({
                event:     'pick',
                id:        findGlbId(hit.object),
                mesh_name: hit.object.name || null,
                point:     [hit.point.x, hit.point.y, hit.point.z],
                shift:     e.shiftKey,
                ctrl:      e.ctrlKey || e.metaKey,
                alt:       e.altKey,
            });
        } else {
            send({ event: 'pick', id: null, mesh_name: null, point: null,
                   shift: e.shiftKey, ctrl: e.ctrlKey || e.metaKey, alt: e.altKey });
        }
    }, { signal: ac.signal });

    // ── Model operations ─────────────────────────────────────────────────────

    /** Walks up from a mesh to the Group with userData.glbId. */
    function findGlbId(obj) {
        for (let o = obj; o; o = o.parent) {
            if (o.userData && typeof o.userData.glbId === 'string') return o.userData.glbId;
        }
        return null;
    }

    function applyTransform(root, t) {
        root.position.set(...t.position);
        root.rotation.set(...t.rotation_euler_xyz); // Euler XYZ, radians
        root.scale.set(...t.scale);
    }

    function setSelected(id, selected) {
        const entry = objects.get(id);
        if (!entry) return;
        if (selected) {
            if (!entry.helper) {
                const h = new THREE.BoxHelper(entry.root, new THREE.Color(currentOptions.selection_color));
                scene.add(h);
                entry.helper = h;
            }
        } else {
            if (entry.helper) {
                scene.remove(entry.helper);
                entry.helper.geometry.dispose();
                entry.helper.material.dispose();
                entry.helper = null;
            }
        }
    }

    /** Recursively disposes all GPU resources of a scene group. */
    function disposeSceneGraph(root) {
        root.traverse((o) => {
            if (o.geometry) o.geometry.dispose();
            const mats = Array.isArray(o.material) ? o.material : (o.material ? [o.material] : []);
            for (const m of mats) {
                // Dispose all texture properties of the material (map, normalMap, aoMap, …)
                for (const k of Object.keys(m)) {
                    const v = m[k];
                    if (v && typeof v === 'object' && v.isTexture) v.dispose();
                }
                m.dispose();
            }
        });
    }

    function removeObject(id) {
        const entry = objects.get(id);
        if (!entry) return;
        if (entry.helper) {
            scene.remove(entry.helper);
            entry.helper.geometry.dispose();
            entry.helper.material.dispose();
        }
        scene.remove(entry.root);
        disposeSceneGraph(entry.root);
        objects.delete(id);
    }

    /** Frames all visible models. MOVES THE CAMERA. */
    function fitView() {
        const box = new THREE.Box3();
        for (const entry of objects.values()) {
            if (entry.root.visible) box.expandByObject(entry.root);
        }
        if (box.isEmpty()) return;
        const center = box.getCenter(new THREE.Vector3());
        const size   = box.getSize(new THREE.Vector3());
        const maxDim = Math.max(size.x, size.y, size.z);
        const fov    = camera.fov * (Math.PI / 180);
        const dist   = (maxDim / 2) / Math.tan(fov / 2) * 1.5;
        const dir    = camera.position.clone().sub(controls.target).normalize();
        camera.position.copy(center.clone().add(dir.multiplyScalar(dist)));
        controls.target.copy(center);
        controls.update();
    }

    function maybeAutoFit() {
        if (!currentOptions.fit_on_first_load || hasAutoFitted) return;
        hasAutoFitted = true;
        fitView();
    }

    // ── Base64 → ArrayBuffer ─────────────────────────────────────────────────
    function base64ToArrayBuffer(b64) {
        if (typeof Uint8Array.fromBase64 === 'function') {
            return Uint8Array.fromBase64(b64).buffer;
        }
        const bin = atob(b64);
        const out = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
        return out.buffer;
    }

    // ── Load logic with generation counter ────────────────────────────────────
    function load(op) {
        const gen = (generations.get(op.id) || 0) + 1;
        generations.set(op.id, gen);
        removeObject(op.id);

        const onLoad = (gltf) => {
            if (generations.get(op.id) !== gen) {
                // A newer request has overtaken this load — release GPU resources.
                disposeSceneGraph(gltf.scene);
                return;
            }
            const root = gltf.scene;
            root.userData.glbId = op.id;
            applyTransform(root, op.transform);
            root.visible = op.visible;
            scene.add(root);
            objects.set(op.id, { root, helper: null });
            if (op.selected) setSelected(op.id, true);
            send({ event: 'loaded', id: op.id });
            maybeAutoFit();
        };

        const onError = (err) => {
            if (generations.get(op.id) !== gen) return;
            send({ event: 'load_error', id: op.id,
                   message: String((err && err.message) || err) });
        };

        if (op.source.kind === 'url') {
            loader.load(op.source.url, onLoad, undefined, onError);
        } else {
            // Bytes path: assemble chunks and parse
            const chunks = pendingBytes.get(op.source.key) || [];
            pendingBytes.delete(op.source.key);
            try {
                const b64    = chunks.join('');
                const buffer = base64ToArrayBuffer(b64);
                // path = '' because .glb is self-contained (no external references)
                loader.parse(buffer, '', onLoad, onError);
            } catch (e) {
                onError(e);
            }
        }
    }

    // ── Environment ───────────────────────────────────────────────────────────
    // Transmissive glass (KHR_materials_transmission) renders what is behind the surface, so with
    // nothing around the scene there is nothing to refract and the material comes out black. The
    // map is built only when one is asked for: a viewer that never wants it pays neither the
    // import nor the render-to-cubemap.
    let environmentKind = 'none';
    let environmentMap  = null;

    async function applyEnvironment(kind) {
        if (kind === environmentKind) return;
        environmentKind = kind;

        if (environmentMap) { environmentMap.dispose(); environmentMap = null; }
        if (kind !== 'room') { scene.environment = null; return; }

        const { RoomEnvironment } =
            await import(threeBase + '/examples/jsm/environments/RoomEnvironment.js');
        const pmrem = new THREE.PMREMGenerator(renderer);
        const room  = new RoomEnvironment();
        const map   = pmrem.fromScene(room).texture;
        // Generator and room were scaffolding; only the cube texture outlives this call.
        pmrem.dispose();
        room.dispose();

        // The import was awaited, so the viewer may have been torn down in the meantime - and a
        // second call may have overtaken this one. Either way this map is no longer the wanted one.
        if (disposed || environmentKind !== kind) { map.dispose(); return; }
        environmentMap = map;
        scene.environment = map;
    }

    // ── Options ───────────────────────────────────────────────────────────────
    let currentOptions = options;
    void applyEnvironment(options.environment);

    function applyOptions(opts) {
        currentOptions = opts;
        scene.background = new THREE.Color(opts.background);
        ambient.intensity = opts.ambient_intensity;
        sun.intensity     = opts.directional_intensity;
        void applyEnvironment(opts.environment);
        // Grid: add/remove based on flag
        if (opts.grid && !grid) {
            grid = new THREE.GridHelper(20, 20);
            scene.add(grid);
        } else if (!opts.grid && grid) {
            scene.remove(grid);
            grid.geometry.dispose();
            grid.material.dispose();
            grid = null;
        }
        // Selection colour: update existing helpers
        for (const entry of objects.values()) {
            if (entry.helper) {
                entry.helper.material.color.set(opts.selection_color);
            }
        }
    }

    // ── Op dispatcher ─────────────────────────────────────────────────────────
    function apply(op) {
        switch (op.op) {
            case 'add':
                load(op);
                break;
            case 'reload':
                load(op);
                break;
            case 'remove':
                removeObject(op.id);
                break;
            case 'set_transform': {
                const entry = objects.get(op.id);
                if (entry) {
                    applyTransform(entry.root, op.transform);
                    if (entry.helper) entry.helper.update();
                }
                break;
            }
            case 'set_visible': {
                const entry = objects.get(op.id);
                if (entry) entry.root.visible = op.visible;
                break;
            }
            case 'set_selected':
                setSelected(op.id, op.selected);
                break;
            case 'set_options':
                applyOptions(op.options);
                break;
            case 'fit_view':
                fitView();
                break;
            case 'reset_camera':
                camera.position.set(...currentOptions.initial_camera);
                controls.target.set(...currentOptions.initial_target);
                controls.update();
                break;
            case 'clear':
                for (const id of [...objects.keys()]) removeObject(id);
                break;
            case 'bytes_begin': {
                // Byte stream start: create empty chunk array
                pendingBytes.set(op.key, new Array(op.chunks));
                break;
            }
            case 'bytes_chunk': {
                const arr = pendingBytes.get(op.key);
                if (arr) arr[op.seq] = op.b64;
                break;
            }
            default:
                console.warn('[dxglb] Unknown op:', op.op);
        }
    }

    // ── Full resource release ─────────────────────────────────────────────────
    function dispose() {
        if (disposed) return;
        disposed = true;
        renderer.setAnimationLoop(null);  // stop RAF loop
        ro.disconnect();
        ac.abort();                       // remove all listeners at once
        controls.dispose();
        for (const id of [...objects.keys()]) removeObject(id);
        if (grid) { scene.remove(grid); grid.geometry.dispose(); grid.material.dispose(); grid = null; }
        if (environmentMap) { environmentMap.dispose(); environmentMap = null; }
        scene.environment = null;
        scene.clear();
        renderer.dispose();
        // Explicitly release the WebGL context (browsers cap at ~16 simultaneous contexts)
        if (typeof renderer.forceContextLoss === 'function') renderer.forceContextLoss();
    }

    return { apply, dispose };
}

// ─── Helper functions ─────────────────────────────────────────────────────────

/**
 * Polls until the canvas element is available.
 * Uses setTimeout instead of requestAnimationFrame because rAF does not fire in hidden or
 * minimised windows — a viewer in a collapsed panel would otherwise hang forever.
 *
 * @param {string} id         - DOM ID of the target element.
 * @param {number} timeoutMs  - Maximum wait time in milliseconds.
 * @returns {Promise<Element|null>}
 */
function waitForElement(id, timeoutMs) {
    return new Promise((resolve) => {
        const el = document.getElementById(id);
        if (el) { resolve(el); return; }

        const interval = setInterval(() => {
            const found = document.getElementById(id);
            if (found) { clearInterval(interval); clearTimeout(timer); resolve(found); }
        }, 16);

        const timer = setTimeout(() => {
            clearInterval(interval);
            resolve(null);
        }, timeoutMs);
    });
}
