AuraScope v0.7.0 "Command Palette & On-Demand Folder Analysis Edition" introduces a telescope-style command palette supporting project auto-detection and custom command execution (`!command`), lazy loading of directories on-demand to eliminate large directory startup lag, an on-demand Deep Directory Storage Analysis popup (`Ctrl+k` / `Shift+k`), and a caching Folder Analysis dashboard widget in the right pane.

### Summary of Changes

* **Command Palette**: Auto-detects build and execution scripts (Cargo, CMake, npm, Docker, Make, Go, Java Gradle/Maven, Python) and supports typing `!` for custom shell commands.
* **On-Demand Directory Expansion**: Speeds up start time on massive workspaces by lazy loading subdirectory contents only when they are expanded in the tree view.
* **Deep Directory Storage Analysis**: Separate background worker thread scanner that calculates recursive files count, sizes, and access times, presenting them in a beautiful pop-up modal breakdown.
* **Folder Analysis Dashboard**: Features immediate file and folder counts, top files by size, and file type extension frequency distributions.
* **Performance Caching**: Employs an interior mutability `RefCell` cache in AppState to guarantee render times remain completely lag-free.
* **Coverage & Verification**: Implements 19 new target unit/integration tests for command palettes, project detectors, and lazy analysis states.

### Commit History and Roles

* `7767cde` - `test: add unit tests for Folder Analysis dashboard calculation`
  * **Role**: Verify Folder Analysis summaries and file extension counts.
* `9ef51e0` - `feat: implement lightweight folder analysis dashboard in the right pane with caching`
  * **Role**: Implement RefCell caching and right-pane dashboard rendering.
* `f5a1236` - `feat: render size details popup UI, map Ctrl+k and Shift+k, add on-demand analysis tests`
  * **Role**: Render loading spinners, subdirectory share tables, and add test suite.
* `bbb9928` - `feat: wire state and Ctrl+k trigger for size details popup`
  * **Role**: Map normal mode KeyEvents to popup trigger.
* `7a8b515` - `feat: implement lazy loading of directories on-demand`
  * **Role**: Disable upfront recursive scans and load folders only on expand.
* `a26c97d` - `feat: command palette custom shell execution and improved build/package tool support`
  * **Role**: Implement custom shell command raw execution.
* `b749a76` - `Merge Task 4: Command Palette UI Modal`
  * **Role**: Merge Command Palette UI widget.
* `2a80bea` - `feat(palette): draw Command Palette modal UI with matching and scrollbar`
  * **Role**: Implement command palette input focus and scrollbar.
* `db95cb6` - `Merge Task 3: Command Palette State Management`
  * **Role**: Merge command palette state matches.
* `444d463` - `feat(palette): add command palette state and update matching logic`
  * **Role**: Implement matching logic for commands list.
* `a214b7d` - `Merge Task 2: Custom Configuration Parsing`
  * **Role**: Merge custom `.ascope.toml` command parser.
* `3347889` - `feat(palette): parse custom commands from .ascope.toml`
  * **Role**: Parse customized user scripts.
* `cf3f632` - `Merge Task 1: Command Detection Engine`
  * **Role**: Merge project scripts builder.
* `f30158c` - `feat(palette): implement project workspace manifest detector`
  * **Role**: Detect build manifests and package scripts.
