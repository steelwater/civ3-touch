-- Zone of Control rules for base mod
-- Military units project ZoC; civilian units do not.
-- All land units are affected by ZoC.

-- Which units project ZoC
register_hook("on_projects_zoc", function(ctx)
    local ut_id = Unit.get(ctx.unit_id, "type")
    local category = UnitType.get(ut_id, "category")
    if category == "civilian" then
        ctx.projects_zoc = false
    else
        ctx.projects_zoc = true
    end
    return ctx
end)

-- Which units are affected by ZoC
register_hook("on_affected_by_zoc", function(ctx)
    -- All land units are affected by ZoC for now
    -- Naval/air units would be excluded here in the future
    ctx.affected_by_zoc = true
    return ctx
end)
