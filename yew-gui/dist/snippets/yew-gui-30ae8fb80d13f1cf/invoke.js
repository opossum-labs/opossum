const invoke = window.__TAURI__.core.invoke

export async function addNode(node_type) {
    return await invoke('add_node', { nodeType: node_type })
        .then(response => {
            return response;
        })
        .catch(error => {
            console.error("Error:", error);
            throw error;
        });
}

export async function getNodeInfo(node_id) {
    return await invoke('get_node_info', { nodeId: node_id })
        .then(response => {
            return response;
        })
        .catch(error => {
            console.error("Error:", error);
            throw error;
        });
}

export async function setInverted(node_id, inverted) {
    return await invoke('set_inverted', { nodeId: node_id, inverted: inverted })
        .then(response => {
            return response;
        })
        .catch(error => {
            console.error("Error:", error);
            throw error;
        });
}

export async function setName(node_id, name) {
    return await invoke('set_name', { nodeId: node_id, name: name })
        .then(response => {
            return response;
        })
        .catch(error => {
            console.error("Error:", error);
            throw error;
        });
}
