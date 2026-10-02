-- hooks.lua — lightweight hook / event system for FreeC3 mods.
--
-- Usage from Lua:
--   register_hook("on_unit_moved", function(ctx) ... end, 100)
--   local result = fire_hook("on_unit_moved", { unit_id = 42 })

-- Internal registry: event_name -> sorted list of {handler, priority}
__hooks = {}

--- Register a handler for an event name.
--- @param event_name string
--- @param handler function(ctx) -> ctx|nil
--- @param priority number|nil  default 100; lower fires first
function register_hook(event_name, handler, priority)
    priority = priority or 100
    if not __hooks[event_name] then
        __hooks[event_name] = {}
    end
    local list = __hooks[event_name]
    -- Insert in sorted order (ascending priority).
    local entry = { handler = handler, priority = priority }
    local inserted = false
    for i, existing in ipairs(list) do
        if priority < existing.priority then
            table.insert(list, i, entry)
            inserted = true
            break
        end
    end
    if not inserted then
        list[#list + 1] = entry
    end
end

--- Fire all handlers registered for an event name.
--- @param event_name string
--- @param context table  mutable context passed to each handler
--- @return table  final context after all handlers have run
function fire_hook(event_name, context)
    context = context or {}
    local list = __hooks[event_name]
    if not list then
        return context
    end
    for _, entry in ipairs(list) do
        local result = entry.handler(context)
        if result ~= nil then
            context = result
        end
        if context.__cancelled then
            break
        end
    end
    return context
end
