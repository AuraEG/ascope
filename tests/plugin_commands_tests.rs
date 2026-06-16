use ascope::plugin::commands::PluginCommand;

#[test]
fn test_command_payload_equality() {
    let cmd1 = PluginCommand::ExecShell {
        cmd: "ls".to_string(),
    };
    let cmd2 = PluginCommand::ExecShell {
        cmd: "ls".to_string(),
    };
    let cmd3 = PluginCommand::FocusPath {
        path: "/tmp".to_string(),
    };

    assert_eq!(cmd1, cmd2);
    assert_ne!(cmd1, cmd3);
}
