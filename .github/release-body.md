AuraScope v0.9.0 "Dev Environment (Tmux + SSH + Docker)" introduces complete integration plugins for developer workflows, bringing native Tmux session control, SSH connection management, and Docker container/image exploration directly to the TUI.

### Summary of Changes

* **Tmux Session splits & Navigation**: Automatically detects active Tmux sessions, splits panes vertically/horizontally, sends workspace paths to active panes, and synchronizes clipboard buffers.
* **SSH Config Parser & Host Picker**: Reads ssh configurations, prompts host connections in remote shell modes, mounts remote paths dynamically via SSHFS, and cleans up mount points on shutdown.
* **Docker Container & Image Explorer**: Asynchronously monitors docker daemon, displays active container metrics/statuses, copies container file structures for TUI browsing, inspects/runs/pulls local or remote images, and parses Compose configuration files.
* **Modal Back-Navigation & Keybinding Enhancements**: Maps `Esc` to return to parent picker tab positions, intercepts `D` keypresses to trigger deletion confirmation popups, and populates action helper menus in the footer.

### Commit History and Roles

* `9ee861a` - `feat(docker): implement docker compose status widget and dialog back navigation`
  * **Role**: Enable compose widget rendering and dialog Esc back navigation.
* `a6dde74` - `feat(docker): execute commands in the active TUI folder context`
  * **Role**: Set workspace directories for background process executions.
* `4236e32` - `feat(docker): implement image pull action and fallbacks`
  * **Role**: Enable background remote image pulling.
* `e1474d3` - `feat(docker): implement container filesystem browser`
  * **Role**: Copy container files locally via docker cp.
* `ccab08f` - `feat(docker): implement docker container picker modal & fallbacks`
  * **Role**: List running docker containers and compose statuses.
* `532b340` - `feat(docker): implement docker explorer plugin & integration tests`
  * **Role**: Build docker plugin skeleton and loader tests.
* `3f1b65f` - `feat: complete ssh remote connection plugin and shell integration`
  * **Role**: Complete remote terminal drop execution.
* `d526518` - `feat: implement ssh active mount manager, dashboard widget, and unmount action`
  * **Role**: Manage SSHFS mount lifetimes on startup and exit.
* `edbbffb` - `feat: implement sshfs mounting and navigation to remote directory`
  * **Role**: Mount SSH host filesystems to local folder paths.
* `efda694` - `feat: implement ssh connection modes picker and shell tmux integration`
  * **Role**: Launch interactive remote shells inside tmux or ssh sessions.
* `58205c0` - `feat: implement ssh config parser and host picker modal`
  * **Role**: Parse ssh configs and display host pickers.
* `b20019a` - `feat: implement tmux integration with smart splitting and clipboard buffer sharing`
  * **Role**: Synchronize system clipboard buffers with tmux buffers.
* `ced8308` - `feat: implement Task 2.4 Tmux Session Picker`
  * **Role**: Show tmux sessions and attach active terminal.
* `19a7edd` - `feat: implement Task 2.3 Tmux Pane List Picker`
  * **Role**: Send folder path selections to other tmux panes.
* `e59081a` - `feat: implement Task 2.2 Tmux Split Actions`
  * **Role**: Run tmux splits in horizontal/vertical orientations.
* `fee3c66` - `feat: implement Task 2.1 Tmux Environment Detection`
  * **Role**: Detect active Tmux shell sessions on startup.
