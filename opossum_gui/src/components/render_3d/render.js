import * as THREE from "three"

export function animate(renderer, cube){
    requestAnimationFrame(function() {animate(renderer, cube)});
    cube.rotation.x += 0.01;
    cube.rotation.y += 0.01;
    renderer.render(scene, camera);
}
export function initThreeJS(canvas_id){
    const canvas = document.getElementById(canvas_id);
    if (!canvas) {
        console.error('Canvas nicht gefunden!');
        return;
    }

    // Scene und Kamera erstellen
    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(75, window.innerWidth / window.innerHeight, 0.1, 1000);
    const renderer = new THREE.WebGLRenderer({ canvas: canvas });
    renderer.setSize(window.innerWidth, window.innerHeight);

    // Einfache Geometrie und Material erstellen
    const geometry = new THREE.BoxGeometry();
    const material = new THREE.MeshBasicMaterial({ color: 0x00ff00 });
    const cube = new THREE.Mesh(geometry, material);
    scene.add(cube);

    // Kamera-Position
    camera.position.z = 5;

    // Animationsloop zum Drehen des Würfels
    
    animate(renderer, cube);
}

// (function(global) {
//     // Erstelle ein Objekt namens `plotlyNamespace` im globalen Namensraum
//     global.opossumRender = global.opossumRender || {};

//     global.opossumRender.animate = function(renderer, cube) {
//         requestAnimationFrame(function() {global.opossumRender.animate(renderer, cube)});
//         cube.rotation.x += 0.01;
//         cube.rotation.y += 0.01;
//         renderer.render(scene, camera);
//     }

//     global.opossumRender.initThreeJS = function(canvas_id) {
//         const canvas = document.getElementById(canvas_id);
//         if (!canvas) {
//             console.error('Canvas nicht gefunden!');
//             return;
//         }

//         // Scene und Kamera erstellen
//         const scene = new THREE.Scene();
//         const camera = new THREE.PerspectiveCamera(75, window.innerWidth / window.innerHeight, 0.1, 1000);
//         const renderer = new THREE.WebGLRenderer({ canvas: canvas });
//         renderer.setSize(window.innerWidth, window.innerHeight);

//         // Einfache Geometrie und Material erstellen
//         const geometry = new THREE.BoxGeometry();
//         const material = new THREE.MeshBasicMaterial({ color: 0x00ff00 });
//         const cube = new THREE.Mesh(geometry, material);
//         scene.add(cube);

//         // Kamera-Position
//         camera.position.z = 5;

//         // Animationsloop zum Drehen des Würfels
        
//         global.opossumRender.animate(renderer, cube);
//     };
// })(window);

