-- Movement cost calculation for the base mod.
-- Registers a hook that sets movement cost based on terrain type and unit category.
--
-- Movement costs use integer-thirds: 1 movement point = 3 internal units.
-- Road = 1 (1/3 of a movement point), Grassland = 3, Hill/Forest = 6.

register_hook("on_calculate_movement_cost", function(ctx)
    local terrain = ctx.terrain
    local road_level = ctx.road_level or 0
    local category = ctx.unit_category or "melee"

    -- Ice is impassable for all unit types
    if terrain == "ice" then
        ctx.cost = -1
        ctx.blocked = true
        return ctx
    end

    -- Naval units: can only move on ocean/coast
    if category == "naval" then
        if terrain == "ocean" or terrain == "coast" then
            if road_level > 0 then
                ctx.cost = 1
            else
                ctx.cost = 3
            end
            ctx.blocked = false
        else
            ctx.cost = -1
            ctx.blocked = true
        end
        return ctx
    end

    -- Land/civilian units: cannot enter ocean or coast
    if terrain == "ocean" or terrain == "coast" then
        ctx.cost = -1
        ctx.blocked = true
        return ctx
    end

    -- Roads override terrain cost (1/3 of a movement point)
    if road_level > 0 then
        ctx.cost = 1
        ctx.blocked = false
        return ctx
    end

    if terrain == "mountain" then
        ctx.cost = -1
        ctx.blocked = true
        return ctx
    end

    -- Check for ignore_terrain_cost trait (scout-type units)
    if ctx.unit_id then
        local traits = UnitType.get_traits(ctx.unit_id)
        if traits then
            for _, trait_name in ipairs(traits) do
                if trait_name == "ignore_terrain_cost" then
                    ctx.cost = 3  -- flat 1 movement point regardless of terrain
                    ctx.blocked = false
                    return ctx
                end
            end
        end
    end

    -- Terrain/vegetation costs (integer-thirds: 3 = 1 movement point)
    local vegetation = ctx.vegetation or "none"
    if terrain == "hill" or vegetation == "forest" or vegetation == "jungle" then
        ctx.cost = 6
    elseif terrain == "grassland" or terrain == "plains" or terrain == "desert"
        or terrain == "tundra" then
        ctx.cost = 3
    else
        ctx.cost = 3
    end

    ctx.blocked = false
    return ctx
end, 100)
