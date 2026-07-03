ascope_fzf = {}

function ascope_fzf.pick(title, items, on_select)
    ascope.open_modal({
        title = title,
        items = items,
        on_select = on_select
    })
end

return ascope_fzf
