-- Turn lifecycle hooks for the base mod.
-- Resets unit state at the start of each player's turn.

register_hook("on_turn_start_unit", function(ctx)
    local uid = ctx.unit_id
    -- Reset movement to max
    local max_move = Unit.get(uid, "max_movement")
    Unit.set(uid, "movement", max_move)
    -- Reset has_moved flag
    Unit.set(uid, "has_moved", false)
    -- Reset skipped flag
    Unit.set(uid, "skipped", false)
end, 100)
