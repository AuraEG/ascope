# ==========================================================================
# File    : ascope.sh
# Project : AuraScope
# Layer   : Shell
# Purpose : Bash/Zsh wrapper that captures the final AuraScope directory and
#           applies it to the parent shell session.
#
# Author  : Ahmed Ashour
# Created : 2026-06-13
# ==========================================================================

ascope_warp() {
    local tmp_file
    tmp_file=$(mktemp -t ascope.XXXXXX 2>/dev/null || mktemp)

    ascope --export-target "$tmp_file" "$@"

    if [ -f "$tmp_file" ]; then
        local target_dir
        target_dir=$(cat "$tmp_file")
        rm -f "$tmp_file"
        if [ -d "$target_dir" ]; then
            cd "$target_dir" || return
        fi
    fi
}

alias asc=ascope_warp
