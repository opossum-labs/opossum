use dioxus::prelude::*;
use serde::Serialize;

const PLOT_FUNCS: Asset = asset!(".\\src\\components\\plot\\plot.js");

#[derive(Serialize)]
struct PlotLayout {
    title: &'static str,
    legend: bool,
}
impl PlotLayout {
    pub fn new() -> Self {
        Self {
            title: "nice title",
            legend: true,
        }
    }
}

#[derive(Serialize)]
struct PlotData {
    x: Vec<f64>,
    y: Vec<f64>,
    r#type: &'static str, // `type` ist ein reserviertes Wort in Rust -> `r#type`
    layout: PlotLayout,
}

#[component]
pub fn PlotComponent() -> Element {
    let plot_id = "plot_div";

    // Beispielhafte Daten für den Plot
    let plot_data = PlotData {
        x: vec![1.0, 2.0, 3.0],
        y: vec![8.0, 6.0, 3.0],
        r#type: "scatter",
        layout: PlotLayout::new(),
    };

    use_future(move || {
        let plot_id = plot_id.to_owned();
        let plot_data_json = serde_json::to_string(&plot_data).unwrap();
        async move {
            let script = format!(
                r#"
            function createPlot(){{
                if (typeof opossumPlots !== 'undefined' && typeof Plotly !== 'undefined'){{
                    const plotData = {}
                    opossumPlots.createPlot('{}', plotData)
                    clearInterval(interval);  // Stoppe das Polling, sobald der Plot erstellt wurde
                }}
            }}
            const interval = setInterval(createPlot, 100);  // Alle 100ms nach dem Element suchen
            "#,
                plot_data_json, plot_id
            );

            document::eval(&script);
        }
    });
    rsx! {
        document::Script { src: PLOT_FUNCS }
        div { class: "plottt", id: plot_id }
    }
}
