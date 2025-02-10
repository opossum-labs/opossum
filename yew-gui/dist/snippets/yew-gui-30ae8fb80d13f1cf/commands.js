const invoke = window.__TAURI__.invoke

export async function addNode(node_type) {
    return await invoke('add_node', { node_type: node_type })
        .then(response => {
            return response;
        })
        .catch(error => {
            console.error("Fehler:", error);
            throw error;
        });
}

export async function printBeepBoop(node_type) {
    return invoke('print_beep_boop');
}