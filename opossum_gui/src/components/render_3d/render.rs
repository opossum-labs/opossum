use dioxus::prelude::*;

// const RENDER_FUNCS: Asset = asset!(".\\src\\components\\render_3d\\render.js");
// const THREE_MOD_JS: Asset = asset!("./assets/three_mod.js");

#[component]
pub fn ThreeJSComponent() -> Element {
    // Dynamische ID für das Canvas, das wir später für Three.js verwenden
    let canvas_id = "threejs_canvas";
    use_future(move || {
        let canvas_id = canvas_id.to_owned();
        async move {
            let script = format!(
                r#"
                function createRender(){{
                    if (typeof THREE !== 'undefined' && typeof ORBIT !== 'undefined' && document.getElementById("{canvas_id}") != null){{
                                            clearInterval(interval);  // Stoppe das Polling, sobald der Plot erstellt wurde

                        const scene = new THREE.Scene();
                        const canvas = document.getElementById("{canvas_id}");
                        const camera = new THREE.PerspectiveCamera( 75, canvas.innerWidth / canvas.innerHeight, 0.1, 1000 );
                        
                        const renderer = new THREE.WebGLRenderer({{canvas}});
                        renderer.setSize( canvas.innerWidth, canvas.innerHeight );
                        renderer.setAnimationLoop( animate );
                        // document.body.appendChild( renderer.domElement );
                        
                        const geometry = new THREE.BoxGeometry( 1, 1, 1 );
                        const material = new THREE.MeshBasicMaterial( {{ color: 0x00ff00 }} );
                        const cube = new THREE.Mesh( geometry, material );
                        scene.add( cube );
                        
                        camera.position.z = 5;
                        const controls = new ORBIT.OrbitControls( camera, renderer.domElement );
                        controls.target.set( 0, 1, 0 );

                        controls.update();
                        
                        function animate() {{                
                            cube.rotation.x += 0.01;
                            cube.rotation.y += 0.01;
                            controls.update();
                            renderer.render( scene, camera );                        
                        }}

                        
                        window.addEventListener("resize", () => {{
        const canvas = renderer.domElement;
        const width = canvas.clientWidth;
        const height = canvas.clientHeight;
        
        renderer.setSize(width, height, false);
        camera.aspect = width / height;
        camera.updateProjectionMatrix();
                                    controls.update();
    
                    }});
                    }}
                }}
                const interval = setInterval(createRender, 100);  // Alle 100ms nach dem Element suchen
                "#,
            );

            document::eval(&script);
        }
    });

    rsx! {
        div { class: "canvas-container",
            canvas { id: canvas_id }
        }
    }
}
