-- units.lua — Base game unit type definitions

UnitType.define({
    name = "warrior",
    attack = 1,
    defense = 1,
    movement = 1,
    max_hp = 3,
    cost = 10,
    category = "melee",
    art_ini = "Art/Units/warrior/Warrior.INI",
})

UnitType.define({
    name = "spearman",
    attack = 1,
    defense = 2,
    movement = 1,
    max_hp = 3,
    cost = 20,
    category = "melee",
    art_ini = "Art/Units/Spearman/Spearman.INI",
    can_produce = function(ctx)
        if not Tech.has_researched(ctx.player_id, "bronze_working") then
            ctx.blocked = true
            ctx.reason = "requires Bronze Working"
        end
        return ctx
    end,
})

UnitType.define({
    name = "swordsman",
    attack = 3,
    defense = 2,
    movement = 1,
    max_hp = 3,
    cost = 30,
    category = "melee",
    art_ini = "Art/Units/Swordsman/Swordsman.INI",
    -- TODO: requires Iron Working tech + iron strategic resource
})

UnitType.define({
    name = "scout",
    attack = 1,
    defense = 1,
    movement = 2,
    max_hp = 2,
    cost = 15,
    category = "melee",
    traits = {"ignore_terrain_cost"},
    art_ini = "Art/Units/Scout/Scout.INI",
})

UnitType.define({
    name = "galley",
    attack = 1,
    defense = 1,
    movement = 3,
    max_hp = 2,
    cost = 30,
    category = "naval",
    art_ini = "Art/Units/Galley/Galley.INI",
    can_produce = function(ctx)
        if not ctx.is_coastal then
            ctx.blocked = true
            ctx.reason = "requires a coastal city"
        end
        return ctx
    end,
})

UnitType.define({
    name = "settler",
    attack = 0,
    defense = 0,
    movement = 1,
    max_hp = 1,
    cost = 30,
    category = "civilian",
    actions = {"build_city"},
    art_ini = "Art/Units/Settler/settler.INI",
    can_produce = function(ctx)
        if ctx.population < 3 then
            ctx.blocked = true
            ctx.reason = "city needs population 3 or more to build a settler"
        end
        return ctx
    end,
})

UnitType.define({
    name = "worker",
    attack = 0,
    defense = 0,
    movement = 1,
    max_hp = 1,
    cost = 15,
    category = "civilian",
    actions = {"build_road", "build_mine", "build_irrigation"},
    art_ini = "Art/Units/Worker/Worker.INI",
})
