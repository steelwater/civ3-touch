-- City production rules for the base mod

-- on_can_produce: global validation hook for city production
-- Fires after per-unit-type can_produce callbacks.
-- Context: city_id, city_name, x, y, population, player_id, is_coastal,
--          unit_type_name, cost, blocked, reason
register_hook("on_can_produce", function(ctx)
    -- Placeholder: always allow for now (tech requirements not yet implemented)
    return ctx
end, 50)

-- on_city_process_production: handles shield accumulation and completion
register_hook("on_city_process_production", function(ctx)
    if not ctx.producing then
        return ctx
    end

    ctx.shield_stockpile = ctx.shield_stockpile + ctx.shields_per_turn

    if ctx.shield_stockpile >= ctx.production_cost then
        ctx.production_complete = true
        ctx.overflow = ctx.shield_stockpile - ctx.production_cost
    end

    return ctx
end, 50)
