-- City growth rules for the base mod

-- Food consumption: 2 food per population per turn (Civ3 standard)
FOOD_PER_POP = 2

-- Growth threshold: food_needed = 10 + 2 * population
-- TODO: Refine this formula later (actual Civ3 varies by difficulty level)
function food_needed_for_growth(population)
    return 10 + 2 * population
end

-- on_city_process_food: handles food accumulation, growth, and starvation
register_hook("on_city_process_food", function(ctx)
    local net_food = ctx.food_per_turn - (ctx.population * FOOD_PER_POP)
    ctx.food_stockpile = ctx.food_stockpile + net_food
    ctx.net_food = net_food

    local needed = food_needed_for_growth(ctx.population)
    if ctx.food_stockpile >= needed then
        -- Grow
        ctx.population = ctx.population + 1
        ctx.food_stockpile = ctx.food_stockpile - needed
        ctx.grew = true
    elseif ctx.food_stockpile < 0 and ctx.population > 1 then
        -- Starve
        ctx.population = ctx.population - 1
        ctx.food_stockpile = 0
        ctx.starved = true
    elseif ctx.food_stockpile < 0 then
        -- Pop 1 cannot starve below 1
        ctx.food_stockpile = 0
    end

    return ctx
end, 50)

-- Granary effect: preserve 50% food on city growth
register_hook("on_city_process_food", function(ctx)
    if ctx.grew and City.has_building(ctx.city_id, "granary") then
        local needed = food_needed_for_growth(ctx.population - 1)
        ctx.food_stockpile = math.floor(needed / 2)
    end
    return ctx
end, 60)
