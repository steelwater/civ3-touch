-- City name lists for the base mod

CITY_NAMES = {
    default = {
        "Alexandria", "Byzantium", "Carthage", "Damascus", "Eridu",
        "Florence", "Gao", "Heliopolis", "Isin", "Jerusalem",
        "Lagos", "Memphis", "Nara", "Olympia", "Persepolis",
        "Qarth", "Rome", "Susa", "Thebes", "Uruk",
        "Venice", "Waset", "Xian", "Yerevan", "Zara"
    },
}

-- Track which names have been used per civ
local used_names = {}

CityNames = {}

--- Returns the next unused city name for the given civilization.
--- Falls back to "default" if the civ has no name list.
--- Returns a generated name if all names are exhausted.
function CityNames.next(civ_name)
    civ_name = civ_name or "default"
    local names = CITY_NAMES[civ_name] or CITY_NAMES["default"]
    if not names then
        return "City"
    end

    if not used_names[civ_name] then
        used_names[civ_name] = {}
    end

    for _, name in ipairs(names) do
        if not used_names[civ_name][name] then
            used_names[civ_name][name] = true
            return name
        end
    end

    -- All names used, generate a numbered one
    local count = 0
    for _ in pairs(used_names[civ_name]) do
        count = count + 1
    end
    local fallback = "City " .. (count + 1)
    used_names[civ_name][fallback] = true
    return fallback
end
