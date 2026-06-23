AuraScope v0.6.0 "Unified Search Modal & Scrolling Edition" introduces a telescope-style unified search overlay dialog. Features include fuzzy file finder, asynchronous live grep content search, search match highlighting/centering in code previews, insert vs normal modes, custom border colors, colored file icons, and a custom sliding viewport with a stateful scrollbar.

### Summary of Changes

* **Fuzzy File Finder**: Dynamic query search of all scanned workspace files utilizing the `nucleo` fuzzy matching engine.
* **Asynchronous Live Grep**: High-performance, background-threaded content search using `ripgrep` with instant process cancellation on new keystrokes.
* **Match Preview Integration**: Matches from live grep automatically highlight and center in the `bat` code preview pane on the dashboard.
* **Navigation Modes**: Added dynamic modes toggling between `Insert` (interactive editing) and `Normal` (list traversal using `j`/`k` or arrow keys).
* **Modal Styling & Polish**: Implemented custom border coloring (Gold for Fuzzy, Blue for Live Grep), file-specific colored icons in search results, and cursor visibility management.
* **Viewport Scrolling & Scrollbar**: Replaced jumpy stateful list scrolling with a custom manual sliding-window viewport and integrated a stateful `Scrollbar` on the right side of the results.
* **Lua Search API**: Exposed the synchronous `ascope.search` API to Lua plugins, returning formatted JSON search results.
* **Test Verification**: Added comprehensive test coverage for fuzzy file finding, async ripgrep, cursor movement, editing, and the Lua search API.

### Commit History and Roles

* `8c9bcf0` - `Merge task 14: Expose Search API to Lua Plugin Engine`
  * **Role**: Merge Lua search API and overlay scrollbar / viewport fixes.
* `5c8bcb7` - `feat(search): implement search overlay navigation modes, stateful scrolling, and lua search API`
  * **Role**: Implement scrolling viewport, scrollbar widget, navigation modes, and Lua engine search exposition.
* `91fafe2` - `Merge task 13: Wire Search Matches to bat Line Highlight`
  * **Role**: Merge preview match line centering and highlighting.
* `48e8814` - `feat(preview): wire search overlay matches to preview line highlighting and centering`
  * **Role**: Implement `bat` line highlighting options mapping and scroll centering.
* `945fc16` - `Merge task 12: Implement Async Ripgrep Engine`
  * **Role**: Merge background ripgrep search engine.
* `c61dc6f` - `feat(search): implement cancelable async ripgrep background search engine`
  * **Role**: Implement worker thread, process handles tracking, and crossbeam message passing.
* `c307c55` - `Merge task 11: Implement Fuzzy Finding for File Navigation`
  * **Role**: Merge `nucleo` library fuzzy matcher logic.
* `78f79ed` - `feat(search): implement fuzzy matching over workspace files in search modal`
  * **Role**: Integrate `nucleo` files matcher with `AppState`.
* `fcb100e` - `Merge task 10: Create Floating Modal UI Layout`
  * **Role**: Merge initial modal rendering widget.
* `bc24f84` - `feat(search): draw floating overlay prompt box`
  * **Role**: Create initial popup window bounds and input bar layout.
