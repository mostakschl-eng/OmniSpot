fn main() {
    println!("cargo:rerun-if-changed=assets/app-switcher.ico");
    println!("cargo:rerun-if-changed=app.rc");
    embed_resource::compile("app.rc", embed_resource::NONE)
        .manifest_required()
        .expect("embed the OmniSpot Windows icon");

    let config = slint_build::CompilerConfiguration::new()
        .with_edition((1, 19))
        .with_style("fluent".into())
        .embed_resources(slint_build::EmbedResourcesKind::EmbedFiles);
    slint_build::compile_with_config("ui/settings.slint", config)
        .expect("compile the OmniSpot Settings and About windows");
}
