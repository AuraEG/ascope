use ascope::plugin::manifest::PluginManifest;

#[test]
fn test_manifest_parsing() {
    let toml_str = r#"
        name = "test-plugin"
        version = "1.0.0"
        author = "Ahmed"
        main = "init.lua"
    "#;
    let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
    assert_eq!(manifest.name, "test-plugin");
    assert_eq!(manifest.main, "init.lua");
}
