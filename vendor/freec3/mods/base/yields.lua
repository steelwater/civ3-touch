-- Tile yield definitions for the base mod

TILE_YIELDS = {
    grassland = {food = 2, shields = 1, commerce = 0},
    plains    = {food = 1, shields = 1, commerce = 1},
    desert    = {food = 0, shields = 1, commerce = 0},
    tundra    = {food = 1, shields = 0, commerce = 0},
    hill      = {food = 1, shields = 2, commerce = 0},
    coast     = {food = 1, shields = 0, commerce = 2},
    ocean     = {food = 1, shields = 0, commerce = 2},
    mountain  = {food = 0, shields = 1, commerce = 0},
    ice       = {food = 0, shields = 0, commerce = 0},
}

-- Vegetation overlays override base terrain yields
VEGETATION_YIELDS = {
    forest = {food = 1, shields = 2, commerce = 0},
    jungle = {food = 1, shields = 0, commerce = 0},
}

-- on_calculate_tile_yield: returns {food, shields, commerce} for a tile
register_hook("on_calculate_tile_yield", function(ctx)
    local vegetation = ctx.vegetation or "none"

    -- Vegetation overrides base terrain yields
    local veg_yields = VEGETATION_YIELDS[vegetation]
    if veg_yields then
        ctx.food = veg_yields.food
        ctx.shields = veg_yields.shields
        ctx.commerce = veg_yields.commerce
    else
        local yields = TILE_YIELDS[ctx.terrain]
        if yields then
            ctx.food = yields.food
            ctx.shields = yields.shields
            ctx.commerce = yields.commerce
        else
            ctx.food = 0
            ctx.shields = 0
            ctx.commerce = 0
        end
    end

    -- Road bonus: +1 commerce
    if ctx.road_level and ctx.road_level > 0 then
        ctx.commerce = ctx.commerce + 1
    end

    -- Improvement bonuses
    if ctx.improvement == 1 then       -- Mine
        ctx.shields = ctx.shields + 1
    elseif ctx.improvement == 2 then   -- Irrigation
        ctx.food = ctx.food + 1
    end

    return ctx
end, 50)
