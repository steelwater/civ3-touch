-- actions.lua — Action definitions for the base mod

-- Improvement type constants (must match yields.lua)
IMPROVEMENT_MINE = 1
IMPROVEMENT_IRRIGATION = 2

Action.define({
    id = "build_city",
    name = "Build City",
    hotkey = "B",
    turns = 0,
    consumes_unit = true,
    animation_name = "BUILD",
    icon = {7, 1},
    valid_conditions = function(ctx)
        -- Block on ocean, coast, mountain
        if ctx.terrain == "ocean" or ctx.terrain == "coast" or ctx.terrain == "mountain" then
            ctx.blocked = true
            ctx.reason = "cannot found a city on " .. ctx.terrain
            return ctx
        end
        -- Block within 2 tiles of existing city (Chebyshev distance)
        local cities = City.all_positions()
        local map_width = ctx.map_width
        local wrap_x = ctx.wrap_x
        for _, pos in ipairs(cities) do
            local dx = math.abs(ctx.x - pos.x)
            if wrap_x and map_width then
                dx = math.min(dx, map_width - dx)
            end
            local dy = math.abs(ctx.y - pos.y)
            if math.max(dx, dy) <= 2 then
                ctx.blocked = true
                ctx.reason = "too close to an existing city"
                return ctx
            end
        end
        return ctx
    end,
    on_complete = function(ctx)
        local name = CityNames.next(ctx.player_id)
        City.create(name, ctx.player_id, ctx.x, ctx.y)
    end,
})

Action.define({
    id = "build_road",
    name = "Build Road",
    hotkey = "R",
    turns = 2,
    consumes_unit = false,
    animation_name = "ROAD",
    icon = {0, 2},
    valid_conditions = function(ctx)
        if ctx.terrain == "ocean" or ctx.terrain == "coast" or ctx.terrain == "mountain" then
            ctx.blocked = true
            ctx.reason = "cannot build a road on " .. ctx.terrain
            return ctx
        end
        local road = Tile.get(ctx.x, ctx.y, "road_level")
        if road and road > 0 then
            ctx.blocked = true
            ctx.reason = "road already exists"
            return ctx
        end
        return ctx
    end,
    on_complete = function(ctx)
        Tile.set(ctx.x, ctx.y, "road_level", 1)
    end,
})

Action.define({
    id = "build_mine",
    name = "Build Mine",
    hotkey = "M",
    turns = 3,
    consumes_unit = false,
    animation_name = "MINE",
    icon = {3, 2},
    valid_conditions = function(ctx)
        if ctx.terrain ~= "hill" then
            ctx.blocked = true
            ctx.reason = "mines can only be built on hills"
            return ctx
        end
        local imp = Tile.get(ctx.x, ctx.y, "improvement")
        if imp ~= nil then
            ctx.blocked = true
            ctx.reason = "improvement already exists"
            return ctx
        end
        return ctx
    end,
    on_complete = function(ctx)
        Tile.set(ctx.x, ctx.y, "improvement", IMPROVEMENT_MINE)
    end,
})

Action.define({
    id = "build_irrigation",
    name = "Build Irrigation",
    hotkey = "I",
    turns = 3,
    consumes_unit = false,
    animation_name = "IRRIGATE",
    icon = {4, 2},
    valid_conditions = function(ctx)
        if ctx.terrain ~= "grassland" and ctx.terrain ~= "plains" and ctx.terrain ~= "desert" then
            ctx.blocked = true
            ctx.reason = "irrigation can only be built on grassland, plains, or desert"
            return ctx
        end
        local vegetation = ctx.vegetation or "none"
        if vegetation ~= "none" then
            ctx.blocked = true
            ctx.reason = "must clear vegetation before irrigating"
            return ctx
        end
        local imp = Tile.get(ctx.x, ctx.y, "improvement")
        if imp ~= nil then
            ctx.blocked = true
            ctx.reason = "improvement already exists"
            return ctx
        end
        return ctx
    end,
    on_complete = function(ctx)
        Tile.set(ctx.x, ctx.y, "improvement", IMPROVEMENT_IRRIGATION)
    end,
})
