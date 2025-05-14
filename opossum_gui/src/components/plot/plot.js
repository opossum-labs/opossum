// plotly_integration.js
(function(global) {
    // Erstelle ein Objekt namens `plotlyNamespace` im globalen Namensraum
    global.opossumPlots = global.opossumPlots || {};

    // Funktion zum Erstellen eines Plots
    global.opossumPlots.createPlot = function(plotId, plotData) {
        const plotElement = document.getElementById(plotId);
        const layout = {
            title: {
                text: plotData.layout.title
            },
            showlegend: plotData.layout.legend
        };

        // Plotly Plot erstellen
        Plotly.newPlot(plotElement, [{
            x: plotData.x,
            y: plotData.y,
            type: plotData.type
        }], layout, {
            displayModeBar: true,
            displaylogo: false,
            modeBarButtonsToRemove: ["select2d", "lasso2d", "autoScale2d", "zoomIn2d", "zoomOut2d"]
        });
    };

})(window);
