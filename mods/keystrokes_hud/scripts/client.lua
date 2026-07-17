local config = game.config.define({
    { key = "enabled", type = "boolean", label = "Enabled", default = true },
    { key = "anchor", type = "choice", label = "Anchor", default = "top_left",
      options = {
          { value = "top_left", label = "Top left" },
          { value = "top_right", label = "Top right" },
          { value = "bottom_left", label = "Bottom left" },
          { value = "bottom_right", label = "Bottom right" }
      }
    },
    { key = "offset_x", type = "number", label = "Offset X", default = 24, min = 0, max = 4096, step = 1 },
    { key = "offset_y", type = "number", label = "Offset Y", default = 24, min = 0, max = 4096, step = 1 },
    { key = "scale", type = "number", label = "Scale", default = 1.2, min = 0.5, max = 3.0, step = 0.05 },
    { key = "key_size", type = "number", label = "Key size", default = 24, min = 12, max = 48, step = 1 },
    { key = "key_gap", type = "number", label = "Key gap", default = 5, min = 0, max = 20, step = 1 },
    { key = "row_gap", type = "number", label = "Row gap", default = 4, min = 0, max = 20, step = 1 },
    { key = "text_scale", type = "number", label = "Text scale", default = 1.0, min = 0.75, max = 1.5, step = 0.05 },
    { key = "theme", type = "choice", label = "Theme", default = "classic",
      options = {
          { value = "classic", label = "Classic" },
          { value = "dark", label = "Dark" },
          { value = "light", label = "Light" }
      }
    },
    { key = "box_alpha", type = "number", label = "Box opacity", default = 0.58, min = 0.0, max = 1.0, step = 0.01 },
    { key = "active_alpha", type = "number", label = "Active opacity", default = 0.90, min = 0.0, max = 1.0, step = 0.01 },
    { key = "text_alpha", type = "number", label = "Text opacity", default = 1.00, min = 0.0, max = 1.0, step = 0.01 },
    { key = "show_mouse", type = "boolean", label = "Show mouse buttons", default = true },
    { key = "show_sneak", type = "boolean", label = "Show sneak", default = true },
    { key = "show_space", type = "boolean", label = "Show space", default = true },
    { key = "show_cps", type = "boolean", label = "Show CPS", default = true },
    { key = "show_fps", type = "boolean", label = "Show FPS", default = true },
    { key = "show_ping", type = "boolean", label = "Show ping", default = true },
    { key = "cps_window", type = "number", label = "CPS window (seconds)", default = 1.0, min = 0.25, max = 2.0, step = 0.05 }
})

local THEMES = {
    classic = {
        border = { r = 0.11, g = 0.14, b = 0.20 },
        inactive = { r = 0.35, g = 0.43, b = 0.58 },
        active = { r = 0.53, g = 0.63, b = 0.85 },
        text = { r = 1.00, g = 1.00, b = 1.00 }
    },
    dark = {
        border = { r = 0.05, g = 0.05, b = 0.06 },
        inactive = { r = 0.18, g = 0.20, b = 0.25 },
        active = { r = 0.20, g = 0.54, b = 0.62 },
        text = { r = 0.98, g = 0.98, b = 0.98 }
    },
    light = {
        border = { r = 0.80, g = 0.82, b = 0.86 },
        inactive = { r = 0.92, g = 0.94, b = 0.97 },
        active = { r = 0.64, g = 0.75, b = 0.96 },
        text = { r = 0.10, g = 0.12, b = 0.15 }
    }
}

local click_times = {
    attack = {},
    use = {}
}
local current_time = 0.0

local function clamp(v, min_v, max_v)
    if v < min_v then
        return min_v
    end
    if v > max_v then
        return max_v
    end
    return v
end

local function round(v)
    return math.floor((v or 0) + 0.5)
end

local function remove_old_clicks(list, now, window)
    local cutoff = now - window
    local i = 1
    while i <= #list do
        if list[i] < cutoff then
            table.remove(list, i)
        else
            i = i + 1
        end
    end
end

local function count_clicks(list, now, window)
    remove_old_clicks(list, now, window)
    return #list
end

local function push_click(kind)
    local list = click_times[kind]
    if list then
        list[#list + 1] = current_time
    end
end

local function format_number(value, suffix)
    return string.format("%d%s", round(value), suffix or "")
end

local function get_theme()
    return THEMES[config.get("theme")] or THEMES.classic
end

local function get_window_size()
    local client = game.client.snapshot() or {}
    local window = client.window or {}
    local width = tonumber(window.framebuffer_width or window.width) or 0
    local height = tonumber(window.framebuffer_height or window.height) or 0
    return width, height, client
end

local function build_overlay_size()
    local scale = clamp(tonumber(config.get("scale")) or 1.0, 0.5, 3.0)
    local key = math.max(1.0, (tonumber(config.get("key_size")) or 20) * scale)
    local gap = math.max(0.0, (tonumber(config.get("key_gap")) or 4) * scale)
    local row_gap = math.max(0.0, (tonumber(config.get("row_gap")) or 3) * scale)
    local mouse_width = math.max(key * 1.8, 2.0 * key + gap)
    local row2_width = key * 3.0 + gap * 2.0
    local stat_width = math.max(row2_width, key * 4.5)
    local overlay_width = stat_width

    if config.get("show_mouse") then
        overlay_width = math.max(overlay_width, mouse_width * 2.0 + gap)
    end

    local rows = 2 -- W + A/S/D
    if config.get("show_mouse") then rows = rows + 1 end
    if config.get("show_sneak") then rows = rows + 1 end
    if config.get("show_space") then rows = rows + 1 end
    if config.get("show_cps") then rows = rows + 1 end
    if config.get("show_fps") then rows = rows + 1 end
    if config.get("show_ping") then rows = rows + 1 end

    local overlay_height = key * rows + row_gap * math.max(0, rows - 1)
    return {
        width = overlay_width,
        height = overlay_height,
        key = key,
        gap = gap,
        row_gap = row_gap,
        mouse_width = mouse_width,
        row2_width = row2_width,
        stat_width = stat_width
    }
end

local function resolve_origin(layout, win_width, win_height)
    local anchor = config.get("anchor")
    local ox = tonumber(config.get("offset_x")) or 0
    local oy = tonumber(config.get("offset_y")) or 0

    if anchor == "top_right" then
        return math.max(0, win_width - ox - layout.width), oy
    elseif anchor == "bottom_left" then
        return ox, math.max(0, win_height - oy - layout.height)
    elseif anchor == "bottom_right" then
        return math.max(0, win_width - ox - layout.width), math.max(0, win_height - oy - layout.height)
    end

    return ox, oy
end

local function color_with_alpha(color, alpha)
    return { r = color.r, g = color.g, b = color.b, a = alpha }
end

local function centered_text_x(x, width, text, scale)
    local estimate = (#text or 0) * 6.2 * scale
    return x + (width - estimate) * 0.5
end

local function centered_text_y(y, height, scale)
    return y + (height - 9.5 * scale) * 0.5 + 0.5
end

local function draw_box(event, theme, x, y, width, height, text, active, scale)
    local text_scale_factor = clamp(tonumber(config.get("text_scale")) or 1.0, 0.75, 1.5)
    local text_scale = math.max(0.9, scale * 0.90 * text_scale_factor)
    local border = color_with_alpha(theme.border, 0.95)
    local fill = color_with_alpha(active and theme.active or theme.inactive, active and (tonumber(config.get("active_alpha")) or 0.9) or (tonumber(config.get("box_alpha")) or 0.58))
    local text_color = color_with_alpha(theme.text, tonumber(config.get("text_alpha")) or 1.0)
    local inset = math.max(1, math.floor(scale + 1.0))

    event.draw:rect({
        x = x,
        y = y,
        width = width,
        height = height,
        color = border
    })
    event.draw:rect({
        x = x + inset,
        y = y + inset,
        width = math.max(0.0, width - inset * 2.0),
        height = math.max(0.0, height - inset * 2.0),
        color = fill
    })
    event.draw:text({
        text = text,
        x = centered_text_x(x, width, text, scale),
        y = centered_text_y(y, height, scale),
        scale = text_scale,
        color = text_color
    })
end

local function input_down(action)
    return game.input.is_down(action)
end

local function tick_to_cps(count, window)
    window = math.max(0.25, tonumber(window) or 1.0)
    return round(count / window)
end

local function purge_clicks(now)
    local window = math.max(0.25, tonumber(config.get("cps_window")) or 1.0)
    remove_old_clicks(click_times.attack, now, window)
    remove_old_clicks(click_times.use, now, window)
end

local function reset_click_history()
    click_times.attack = {}
    click_times.use = {}
    current_time = 0.0
end

game.events.on("client.tick", function(event)
    if not config.get("enabled") then
        return
    end

    local dt = tonumber(event.delta_time) or 0.0
    if event.playing then
        current_time = current_time + dt
        purge_clicks(current_time)
    else
        reset_click_history()
    end
end)

game.events.on("input.action", function(event)
    if not config.get("enabled") then
        return
    end

    if event.edge ~= "pressed" or event["repeat"] then
        return
    end

    if event.action == "attack" then
        push_click("attack")
    elseif event.action == "use" then
        push_click("use")
    end
end)

game.events.on("render.hud.after", function(event)
    if not config.get("enabled") then
        return
    end

    local theme = get_theme()
    local layout = build_overlay_size()
    local win_width, win_height, client = get_window_size()
    if win_width <= 0 or win_height <= 0 then
        return
    end

    local base_x, base_y = resolve_origin(layout, win_width, win_height)
    local scale = clamp(tonumber(config.get("scale")) or 1.0, 0.5, 3.0)
    local key = layout.key
    local gap = layout.gap
    local row_gap = layout.row_gap
    local row_x = base_x + (layout.width - layout.row2_width) * 0.5
    local row_y = base_y
    local stat_width = layout.stat_width
    local mouse_width = layout.mouse_width

    -- W key
    draw_box(event, theme, row_x + key + gap, row_y, key, key, "W", input_down("forward"), scale)
    row_y = row_y + key + row_gap

    -- A / S / D row
    draw_box(event, theme, row_x, row_y, key, key, "A", input_down("strafe_left"), scale)
    draw_box(event, theme, row_x + key + gap, row_y, key, key, "S", input_down("backward"), scale)
    draw_box(event, theme, row_x + (key + gap) * 2.0, row_y, key, key, "D", input_down("strafe_right"), scale)
    row_y = row_y + key + row_gap

    -- Mouse buttons
    if config.get("show_mouse") then
        local mouse_row_x = base_x + (layout.width - (mouse_width * 2.0 + gap)) * 0.5
        draw_box(event, theme, mouse_row_x, row_y, mouse_width, key, "LMB", input_down("attack"), scale)
        draw_box(event, theme, mouse_row_x + mouse_width + gap, row_y, mouse_width, key, "RMB", input_down("use"), scale)
        row_y = row_y + key + row_gap
    end

    -- Sneak row
    if config.get("show_sneak") then
        draw_box(event, theme, base_x, row_y, layout.width, key, "Sneak", input_down("sneak"), scale)
        row_y = row_y + key + row_gap
    end

    -- Space row
    if config.get("show_space") then
        draw_box(event, theme, base_x, row_y, layout.width, key, "Space", input_down("jump"), scale)
        row_y = row_y + key + row_gap
    end

    -- CPS row
    if config.get("show_cps") then
        local window = math.max(0.25, tonumber(config.get("cps_window")) or 1.0)
        local cps = tick_to_cps(count_clicks(click_times.attack, current_time, window) + count_clicks(click_times.use, current_time, window), window)
        draw_box(event, theme, base_x, row_y, stat_width, key, string.format("%d CPS", cps), cps > 0, scale)
        row_y = row_y + key + row_gap
    end

    -- FPS row
    if config.get("show_fps") then
        local fps = round(tonumber(client.fps) or 0)
        draw_box(event, theme, base_x, row_y, stat_width, key, string.format("%d FPS", fps), fps > 0, scale)
        row_y = row_y + key + row_gap
    end

    -- Ping row
    if config.get("show_ping") then
        local ping = nil
        if client.connection and client.connection.latency_ms ~= nil then
            ping = tonumber(client.connection.latency_ms)
        end
        local text = ping and string.format("%dms", round(ping)) or "--ms"
        draw_box(event, theme, base_x, row_y, stat_width, key, text, ping ~= nil and ping >= 0, scale)
    end
end)

function on_load()
    game.log.info("Keystrokes HUD loaded: configurable keystrokes + CPS overlay")
end

