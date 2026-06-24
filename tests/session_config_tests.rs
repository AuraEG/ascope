use ascope::config::session::parse_session_config;
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_parse_session_config() {
    let dir = tempdir().unwrap();
    let mut config_file = File::create(dir.path().join(".ascope.toml")).unwrap();
    write!(
        config_file,
        r#"
        [[commands]]
        name = "Run Local DB"
        cmd = "docker-compose up -d"
        "#
    )
    .unwrap();

    let cfg = parse_session_config(dir.path());
    assert_eq!(cfg.commands.len(), 1);
    assert_eq!(cfg.commands[0].name, "Run Local DB");
    assert_eq!(cfg.commands[0].cmd, "docker-compose up -d");
}
