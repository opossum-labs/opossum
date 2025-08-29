// This build script is necessary in order to embed an application icon into the windows executable.
// Unfortunately this does not work with the standard dioxus bundler...

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/favicon.ico");

        // This is good practice for modern Windows applications
        res.set_manifest(
            r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
<trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
        <requestedPrivileges>
            <requestedExecutionLevel level="asInvoker" uiAccess="false" />
        </requestedPrivileges>
    </security>
</trustInfo>
</assembly>
"#,
        );
        if let Err(e) = res.compile() {
            eprintln!("Failed to compile Windows resources: {}", e);
            std::process::exit(1);
        }
    }
}
