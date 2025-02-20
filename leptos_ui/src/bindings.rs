use wasm_bindgen::{prelude::wasm_bindgen, JsValue};
use web_sys::js_sys;

#[wasm_bindgen(module = "/invoke.js")]
unsafe extern "C" {
    pub unsafe fn addNode(node_type: String) -> js_sys::Promise;
    pub unsafe fn getNodeInfo(node_id: String) -> js_sys::Promise;
    pub unsafe fn setInverted(node_id: String, inverted: bool) -> js_sys::Promise;
    pub unsafe fn setName(node_id: String, name: String) -> js_sys::Promise;
    pub unsafe fn setLidt(node_id: String, lidt: f64) -> js_sys::Promise;
    pub unsafe fn connectNodes(
        node_id_1: String,
        port_1: String,
        node_id_2: String,
        port_2: String,
        distance: f64,
    ) -> js_sys::Promise;
}
