-- Base building definitions

Building.define({
    id = "palace",
    name = "Palace",
    cost = 200,
    maintenance = 0,
    requires = {},
    can_produce = function(ctx)
        ctx.blocked = true
        ctx.reason = "cannot build Palace"
        return ctx
    end,
})

Building.define({
    id = "granary",
    name = "Granary",
    cost = 60,
    maintenance = 1,
    requires = {"pottery"},
})

-- Auto-add Palace when founding the player's first city
register_hook("on_city_founded", function(ctx)
    local cities = City.cities_for_player(ctx.player_id)
    if #cities == 1 then
        City.add_building(ctx.city_id, "palace")
    end
    return ctx
end, 50)
