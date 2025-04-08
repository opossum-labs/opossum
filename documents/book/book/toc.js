// Populate the sidebar
//
// This is a script, and not included directly in the page, to control the total size of the book.
// The TOC contains an entry for each page, so if each page includes a copy of the TOC,
// the total size of the page becomes O(n**2).
class MDBookSidebarScrollbox extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        this.innerHTML = '<ol class="chapter"><li class="chapter-item expanded "><a href="intro.html"><strong aria-hidden="true">1.</strong> Introduction</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="goals.html"><strong aria-hidden="true">1.1.</strong> Goals</a></li><li class="chapter-item expanded "><a href="plan.html"><strong aria-hidden="true">1.2.</strong> Project plan</a></li></ol></li><li class="chapter-item expanded "><a href="existing_software.html"><strong aria-hidden="true">2.</strong> Existing Software</a></li><li class="chapter-item expanded "><a href="optical_model.html"><strong aria-hidden="true">3.</strong> Modeling optical systems</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="nodes.html"><strong aria-hidden="true">3.1.</strong> Nodes</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="nodes/source.html"><strong aria-hidden="true">3.1.1.</strong> Source</a></li><li class="chapter-item expanded "><a href="nodes/dummy.html"><strong aria-hidden="true">3.1.2.</strong> Dummy</a></li><li class="chapter-item expanded "><a href="nodes/beam_splitter.html"><strong aria-hidden="true">3.1.3.</strong> Beam splitter</a></li><li class="chapter-item expanded "><a href="nodes/energy_meter.html"><strong aria-hidden="true">3.1.4.</strong> Energy meter</a></li><li class="chapter-item expanded "><a href="nodes/node_group.html"><strong aria-hidden="true">3.1.5.</strong> Node group</a></li><li class="chapter-item expanded "><a href="nodes/ideal_filter.html"><strong aria-hidden="true">3.1.6.</strong> Ideal filter</a></li><li class="chapter-item expanded "><a href="nodes/reflective_grating.html"><strong aria-hidden="true">3.1.7.</strong> Reflective grating</a></li><li class="chapter-item expanded "><a href="nodes/reference_node.html"><strong aria-hidden="true">3.1.8.</strong> Reference node</a></li><li class="chapter-item expanded "><a href="nodes/spherical_lens.html"><strong aria-hidden="true">3.1.9.</strong> Spherical lens</a></li><li class="chapter-item expanded "><a href="nodes/cylindric_lens.html"><strong aria-hidden="true">3.1.10.</strong> Cylindric lens</a></li><li class="chapter-item expanded "><a href="nodes/spectrometer.html"><strong aria-hidden="true">3.1.11.</strong> Spectrometer</a></li><li class="chapter-item expanded "><a href="nodes/spot_diagram.html"><strong aria-hidden="true">3.1.12.</strong> Spot diagram</a></li><li class="chapter-item expanded "><a href="nodes/wavefront_monitor.html"><strong aria-hidden="true">3.1.13.</strong> Wavefront monitor</a></li><li class="chapter-item expanded "><a href="nodes/paraxial_surface.html"><strong aria-hidden="true">3.1.14.</strong> Paraxial surface</a></li><li class="chapter-item expanded "><a href="nodes/fluence_detector.html"><strong aria-hidden="true">3.1.15.</strong> Fluence detector</a></li><li class="chapter-item expanded "><a href="nodes/wedge.html"><strong aria-hidden="true">3.1.16.</strong> Wedge</a></li><li class="chapter-item expanded "><a href="nodes/mirror.html"><strong aria-hidden="true">3.1.17.</strong> Mirror</a></li><li class="chapter-item expanded "><a href="nodes/parabolic_mirror.html"><strong aria-hidden="true">3.1.18.</strong> Parabolic mirror</a></li></ol></li><li class="chapter-item expanded "><a href="edges.html"><strong aria-hidden="true">3.2.</strong> Edges</a></li><li class="chapter-item expanded "><a href="materials.html"><strong aria-hidden="true">3.3.</strong> Materials</a></li><li class="chapter-item expanded "><a href="optical_components_effects.html"><strong aria-hidden="true">3.4.</strong> Physical processes, Subsystems and Components</a></li></ol></li><li class="chapter-item expanded "><a href="model_analysis.html"><strong aria-hidden="true">4.</strong> Model analysis</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="analyzers.html"><strong aria-hidden="true">4.1.</strong> Analyzers</a></li><li class="chapter-item expanded "><a href="interfacing.html"><strong aria-hidden="true">4.2.</strong> Interfacing with external code</a></li></ol></li><li class="chapter-item expanded "><a href="architecture.html"><strong aria-hidden="true">5.</strong> Software architecture</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="opticscenery.html"><strong aria-hidden="true">5.1.</strong> OpticScenery</a></li><li class="chapter-item expanded "><a href="opticnode.html"><strong aria-hidden="true">5.2.</strong> OpticNode</a></li><li class="chapter-item expanded "><a href="opticanalyzer.html"><strong aria-hidden="true">5.3.</strong> OpticAnalyzer</a></li></ol></li><li class="chapter-item expanded "><a href="usage.html"><strong aria-hidden="true">6.</strong> Usage</a></li><li class="chapter-item expanded "><a href="open_questions.html"><strong aria-hidden="true">7.</strong> Open questions</a></li><li class="chapter-item expanded "><a href="resources.html"><strong aria-hidden="true">8.</strong> Useful resources</a></li></ol>';
        // Set the current, active page, and reveal it if it's hidden
        let current_page = document.location.href.toString().split("#")[0];
        if (current_page.endsWith("/")) {
            current_page += "index.html";
        }
        var links = Array.prototype.slice.call(this.querySelectorAll("a"));
        var l = links.length;
        for (var i = 0; i < l; ++i) {
            var link = links[i];
            var href = link.getAttribute("href");
            if (href && !href.startsWith("#") && !/^(?:[a-z+]+:)?\/\//.test(href)) {
                link.href = path_to_root + href;
            }
            // The "index" page is supposed to alias the first chapter in the book.
            if (link.href === current_page || (i === 0 && path_to_root === "" && current_page.endsWith("/index.html"))) {
                link.classList.add("active");
                var parent = link.parentElement;
                if (parent && parent.classList.contains("chapter-item")) {
                    parent.classList.add("expanded");
                }
                while (parent) {
                    if (parent.tagName === "LI" && parent.previousElementSibling) {
                        if (parent.previousElementSibling.classList.contains("chapter-item")) {
                            parent.previousElementSibling.classList.add("expanded");
                        }
                    }
                    parent = parent.parentElement;
                }
            }
        }
        // Track and set sidebar scroll position
        this.addEventListener('click', function(e) {
            if (e.target.tagName === 'A') {
                sessionStorage.setItem('sidebar-scroll', this.scrollTop);
            }
        }, { passive: true });
        var sidebarScrollTop = sessionStorage.getItem('sidebar-scroll');
        sessionStorage.removeItem('sidebar-scroll');
        if (sidebarScrollTop) {
            // preserve sidebar scroll position when navigating via links within sidebar
            this.scrollTop = sidebarScrollTop;
        } else {
            // scroll sidebar to current active section when navigating via "next/previous chapter" buttons
            var activeSection = document.querySelector('#sidebar .active');
            if (activeSection) {
                activeSection.scrollIntoView({ block: 'center' });
            }
        }
        // Toggle buttons
        var sidebarAnchorToggles = document.querySelectorAll('#sidebar a.toggle');
        function toggleSection(ev) {
            ev.currentTarget.parentElement.classList.toggle('expanded');
        }
        Array.from(sidebarAnchorToggles).forEach(function (el) {
            el.addEventListener('click', toggleSection);
        });
    }
}
window.customElements.define("mdbook-sidebar-scrollbox", MDBookSidebarScrollbox);
