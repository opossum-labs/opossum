// three.js viewer for the optoscene actix example.
//
// It decodes OSCN frames (see the protocol layout in the README) and applies
// the client semantics: full scene, replace layer, upsert nodes, update
// transforms, remove nodes. GPU resources are disposed on every removal so
// memory stays flat across updates.

import * as THREE from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { RoomEnvironment } from "three/addons/environments/RoomEnvironment.js";

const renderer = new THREE.WebGLRenderer({ antialias: true });
renderer.setPixelRatio(window.devicePixelRatio);
renderer.setSize(window.innerWidth, window.innerHeight);
document.body.appendChild(renderer.domElement);

const scene = new THREE.Scene();
scene.background = new THREE.Color(0x101014);

// An environment map, required for glass with transmission to look right.
const pmrem = new THREE.PMREMGenerator(renderer);
scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;

const camera = new THREE.PerspectiveCamera(
  45,
  window.innerWidth / window.innerHeight,
  0.001,
  100,
);
camera.position.set(0.12, 0.09, 0.22);

const controls = new OrbitControls(camera, renderer.domElement);
controls.target.set(0, 0, 0);
controls.update();

// The scene root carries the optoscene root rotation; one group per layer hangs
// under it, and `nodeMap` tracks every content object by its uid.
const root = new THREE.Group();
scene.add(root);
const layerGroups = {
  optics: new THREE.Group(),
  rays: new THREE.Group(),
  aux: new THREE.Group(),
};
for (const group of Object.values(layerGroups)) {
  root.add(group);
}
const nodeMap = new Map();

const loader = new GLTFLoader();

/** Decodes an OSCN frame into its header and payload ArrayBuffer. */
function decodeFrame(buffer) {
  const view = new DataView(buffer);
  const magic = String.fromCharCode(
    view.getUint8(0),
    view.getUint8(1),
    view.getUint8(2),
    view.getUint8(3),
  );
  if (magic !== "OSCN") {
    throw new Error("bad frame magic");
  }
  const version = view.getUint8(4);
  if (version !== 1) {
    throw new Error("unsupported protocol version " + version);
  }
  const headerLength = view.getUint32(5, true);
  const headerBytes = new Uint8Array(buffer, 9, headerLength);
  const header = JSON.parse(new TextDecoder().decode(headerBytes));
  const payload = buffer.slice(9 + headerLength);
  return { header, payload };
}

/** Parses a GLB payload into a gltf result. */
function parseGlb(payload) {
  return new Promise((resolve, reject) => {
    loader.parse(payload, "", resolve, reject);
  });
}

/** Frees the geometry and materials of an object subtree. */
function dispose(object) {
  object.traverse((child) => {
    if (child.geometry) {
      child.geometry.dispose();
    }
    const material = child.material;
    if (Array.isArray(material)) {
      material.forEach((m) => m.dispose());
    } else if (material) {
      material.dispose();
    }
  });
}

/** Removes an object from its parent and disposes it. */
function removeObject(object) {
  if (object.parent) {
    object.parent.remove(object);
  }
  dispose(object);
}

/** Empties a layer group, disposing its content and forgetting its uids. */
function clearLayer(name) {
  const group = layerGroups[name];
  for (const child of [...group.children]) {
    group.remove(child);
    dispose(child);
    if (child.userData && child.userData.uid) {
      nodeMap.delete(child.userData.uid);
    }
  }
}

/** Applies the loaded root node's rotation to the viewer root. */
function applyRootRotation(gltf) {
  const rootNode = gltf.scene.getObjectByName("optoscene_root");
  if (rootNode) {
    root.quaternion.copy(rootNode.quaternion);
  }
}

/** The content objects of a loaded gltf: those carrying a `uid` in userData. */
function contentObjects(gltf) {
  const objects = [];
  gltf.scene.traverse((object) => {
    if (object.userData && object.userData.uid) {
      objects.push(object);
    }
  });
  return objects;
}

/** Inserts a content object into its layer group, replacing any same uid. */
function upsertObject(object) {
  const uid = object.userData.uid;
  const layer = object.userData.layer || "aux";
  const existing = nodeMap.get(uid);
  if (existing) {
    removeObject(existing);
  }
  (layerGroups[layer] || layerGroups.aux).add(object);
  nodeMap.set(uid, object);
}

async function handleMessage(buffer) {
  const { header, payload } = decodeFrame(buffer);
  console.log("message", header.type, header);

  switch (header.type) {
    case "full_scene": {
      for (const name of Object.keys(layerGroups)) {
        clearLayer(name);
      }
      const gltf = await parseGlb(payload);
      applyRootRotation(gltf);
      for (const object of contentObjects(gltf)) {
        upsertObject(object);
      }
      break;
    }
    case "replace_layer": {
      clearLayer(header.layer);
      const gltf = await parseGlb(payload);
      for (const object of contentObjects(gltf)) {
        if ((object.userData.layer || "aux") === header.layer) {
          upsertObject(object);
        }
      }
      break;
    }
    case "upsert_nodes": {
      const gltf = await parseGlb(payload);
      for (const object of contentObjects(gltf)) {
        upsertObject(object);
      }
      break;
    }
    case "update_transforms": {
      for (const entry of header.nodes) {
        const object = nodeMap.get(entry.uid);
        if (object) {
          object.position.set(...entry.translation);
          object.quaternion.set(...entry.rotation);
        }
      }
      break;
    }
    case "remove_nodes": {
      for (const uid of header.uids) {
        const object = nodeMap.get(uid);
        if (object) {
          removeObject(object);
          nodeMap.delete(uid);
        }
      }
      break;
    }
    default:
      console.warn("unknown message type", header.type);
  }
}

function connect() {
  const url = `ws://${location.host}/ws`;
  const socket = new WebSocket(url);
  socket.binaryType = "arraybuffer";
  socket.onmessage = (event) => {
    handleMessage(event.data).catch((error) => console.error(error));
  };
  socket.onclose = () => {
    console.log("socket closed, reconnecting in 1s");
    setTimeout(connect, 1000);
  };
  socket.onerror = () => socket.close();
}
connect();

window.addEventListener("resize", () => {
  camera.aspect = window.innerWidth / window.innerHeight;
  camera.updateProjectionMatrix();
  renderer.setSize(window.innerWidth, window.innerHeight);
});

// Exposed so the geometry count can be watched during acceptance testing.
window.viewerRenderer = renderer;

function animate() {
  requestAnimationFrame(animate);
  controls.update();
  renderer.render(scene, camera);
}
animate();
