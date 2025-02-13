use wasm_bindgen::prelude::wasm_bindgen;
use web_sys::js_sys;

#[wasm_bindgen(module = "/invoke.js")]
extern "C" {
    pub fn addNode(node_type: String) -> js_sys::Promise;
    pub fn getNodeInfo(node_id: String) -> js_sys::Promise;
    pub fn setInverted(node_id: String, inverted: bool) -> js_sys::Promise;
    pub fn setName(node_id: String, name: String) -> js_sys::Promise;
}
