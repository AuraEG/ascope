local zoxide = {}

function zoxide.query()
    ascope.exec_shell("zoxide", {"query", "--list", "--score"}, function(stdout, stderr, exit_code)
        if exit_code ~= 0 then
            ascope.notify("Zoxide query failed", "error")
            return
        end
        ascope.notify("Zoxide query returned " .. #stdout .. " bytes", "info")
    end)
end

return zoxide
