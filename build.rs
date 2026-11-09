fn main() {
    // Each platform gets the widget style that looks native there.
    let style = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("macos") => "cupertino",
        _ => "fluent",
    };
    let config = slint_build::CompilerConfiguration::new().with_style(style.into());
    slint_build::compile_with_config("ui/app.slint", config).expect("the UI compiles");

    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("build/icon.ico");
        resource.compile().expect("the Windows icon embeds");
    }
}
