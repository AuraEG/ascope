ascope.notify("Loading tmux plugin...", "info")

local tmux_env = os.getenv("TMUX")
if tmux_env and tmux_env ~= "" then
    ascope.notify("Tmux detected ✓", "info")
else
    ascope.notify("Tmux environment not detected", "warn")
end
