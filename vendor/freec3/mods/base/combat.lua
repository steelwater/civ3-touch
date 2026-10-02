-- combat.lua — Combat resolution for the base mod.
-- Implements Civ3-style combat rounds and defense bonuses.

-- Terrain defense bonus (priority 40, before resolution)
register_hook("on_pre_combat", function(ctx)
    local terrain = ctx.defender_terrain
    local vegetation = ctx.defender_vegetation or "none"

    local bonus = 0
    -- Hill terrain bonus (50%)
    if terrain == "hill" then
        bonus = bonus + 0.5
    end
    -- Vegetation bonus (25%) — stacks with hill
    if vegetation == "forest" or vegetation == "jungle" then
        bonus = bonus + 0.25
    end

    if bonus > 0 then
        ctx.defender_strength = ctx.defender_strength * (1 + bonus)
    end

    return ctx
end, 40)

-- City defense bonus (priority 42, after terrain but before fortification)
register_hook("on_pre_combat", function(ctx)
    if ctx.defender_in_city then
        ctx.defender_strength = ctx.defender_strength * 1.5
    end
    return ctx
end, 42)

-- Fortification defense bonus (priority 45, before resolution)
register_hook("on_pre_combat", function(ctx)
    if ctx.defender_fortified then
        ctx.defender_strength = ctx.defender_strength * 1.25
    end
    return ctx
end, 45)

-- Combat resolution (priority 50)
-- Civ3-style: each round, attacker has (atk / (atk + def)) chance to deal 1 HP.
-- Otherwise defender deals 1 HP to attacker. Continues until one reaches 0 HP.
register_hook("on_resolve_combat", function(ctx)
    local atk_str = ctx.attacker_strength
    local def_str = ctx.defender_strength
    local atk_hp = ctx.attacker_hp
    local def_hp = ctx.defender_hp
    local rounds = {}
    local round = 0

    while atk_hp > 0 and def_hp > 0 do
        round = round + 1
        -- Attacker wins round if roll <= threshold (out of 1000)
        local threshold = math.floor((atk_str / (atk_str + def_str)) * 1000)
        local roll = Engine.random(1, 1000)

        if roll <= threshold then
            def_hp = def_hp - 1
        else
            atk_hp = atk_hp - 1
        end

        rounds[#rounds + 1] = { attacker_hp = atk_hp, defender_hp = def_hp }
    end

    ctx.attacker_hp = atk_hp
    ctx.defender_hp = def_hp
    ctx.rounds = rounds
    return ctx
end, 50)
