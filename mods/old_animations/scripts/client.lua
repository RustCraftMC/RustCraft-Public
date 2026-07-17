-- RustCraft 1.7-style Planar BlockHit
--
-- 目标：
-- 1. 只在主手持剑且真正 blocking 时触发
-- 2. 普通方块、食物、其他物品右键不会触发
-- 3. 保留原版静态格挡姿势
-- 4. 关闭原版 swing，避免 BlockHit 时剑向镜头前推
-- 5. 仅在格挡平面内绕护手做大幅度旋转
-- 6. 增加明显 XY 扫动，接近 PVP 客户端视频里的大幅 BlockHit
-- 7. 不使用动态 Z 位移，不做 X/Y 三维旋转

local config = game.config.define({
    {
        key = "enabled",
        type = "boolean",
        label = "Enabled",
        default = true
    },

    {
        key = "blockhit_angle",
        type = "number",
        label = "BlockHit recoil",
        default = 14,
        min = 0,
        max = 35,
        step = 1
    },

    {
        key = "pivot_x",
        type = "number",
        label = "Guard pivot X",
        -- The generated-item plane maps texture pixels to [-0.5, 0.5].
        -- The blade and the brown grip meet near pixel (5, 10.5) on swords.
        default = -0.19,
        min = -1.0,
        max = 1.0,
        step = 0.01
    },
    {
        key = "pivot_y",
        type = "number",
        label = "Guard pivot Y",
        default = -0.16,
        min = -1.0,
        max = 1.0,
        step = 0.01
    },
    {
        key = "pivot_z",
        type = "number",
        label = "Guard pivot Z",
        default = 0.0,
        min = -1.0,
        max = 1.0,
        step = 0.01
    },

    {
        key = "sweep_x",
        type = "number",
        label = "Sweep X",
        default = -0.04,
        min = -1.5,
        max = 1.5,
        step = 0.01
    },
    {
        key = "sweep_y",
        type = "number",
        label = "Sweep Y",
        default = 0.02,
        min = -1.5,
        max = 1.5,
        step = 0.01
    },

    {
        key = "swing_duration",
        type = "number",
        label = "BlockHit duration",
        default = 6,
        min = 2,
        max = 20,
        step = 1
    },

    {
        key = "continuous_blockhit",
        type = "boolean",
        label = "Continuous BlockHit",
        default = false
    },

    {
        key = "overshoot",
        type = "number",
        label = "Overshoot",
        default = 1.0,
        min = 1.0,
        max = 1.5,
        step = 0.01
    },

    {
        key = "hud_indicator",
        type = "choice",
        label = "HUD indicator",
        default = "never",
        options = {
            { value = "always", label = "Always" },
            { value = "never", label = "Never" }
        }
    }
})

local swing = {
    current = 0.0,
    previous = 0.0,
    tick = 0,
    active = false,
    attack_was_down = false
}

local function clamp(value, minimum, maximum)
    value = tonumber(value) or minimum

    if value < minimum then
        return minimum
    end

    if value > maximum then
        return maximum
    end

    return value
end

local function clamp01(value)
    return clamp(value, 0.0, 1.0)
end

local function reset_swing()
    swing.current = 0.0
    swing.previous = 0.0
    swing.tick = 0
    swing.active = false
    swing.attack_was_down = false
end

local function begin_swing()
    swing.current = 0.0
    swing.previous = 0.0
    swing.tick = 0
    swing.active = true
end

local function player_is_actually_blocking()
    local action = game.player.action()

    if action == nil then
        return false
    end

    if action.blocking ~= true then
        return false
    end

    -- 某些实现可能暂时不提供 use_action。
    if action.use_action ~= nil
        and action.use_action ~= "block" then
        return false
    end

    return true
end

local function is_sword_block_event(event)
    if event.hand ~= "main_hand" then
        return false
    end

    if event.state == nil then
        return false
    end

    if event.state.blocking ~= true then
        return false
    end

    -- 有 item_type 时必须是 sword。
    if event.state.item_type ~= nil
        and event.state.item_type ~= "sword" then
        return false
    end

    -- 有 use_action 时必须是 block。
    if event.state.use_action ~= nil
        and event.state.use_action ~= "block" then
        return false
    end

    return true
end

local function smoothstep(value)
    value = clamp01(value)
    return value * value * (3.0 - 2.0 * value)
end

-- Continuous BlockHit curve. Both branches meet at the same peak with zero
-- slope, so interpolation remains smooth at the strike and at the return.
local function blockhit_curve(progress)
    progress = clamp01(progress)

    local overshoot =
        clamp(config.get("overshoot"), 1.0, 1.5)

    if progress < 0.32 then
        local t = smoothstep(progress / 0.32)

        return t * overshoot
    end

    local t = smoothstep((progress - 0.32) / 0.68)

    return (1.0 - t) * overshoot
end

game.events.on("client.tick", function(event)
    if not config.get("enabled") then
        reset_swing()
        return
    end

    if not event.playing then
        reset_swing()
        return
    end

    swing.previous = swing.current

    local blocking = player_is_actually_blocking()
    local attack_down =
        game.input.is_down("attack") == true

    if not blocking then
        reset_swing()
        return
    end

    -- 左键按下边沿启动一次 BlockHit。
    if attack_down and not swing.attack_was_down then
        begin_swing()
    end

    -- 可选：左右键持续按住时循环。
    if config.get("continuous_blockhit")
        and attack_down
        and not swing.active then
        begin_swing()
    end

    if swing.active then
        local duration = math.floor(
            clamp(config.get("swing_duration"), 2, 20)
        )

        swing.tick = swing.tick + 1
        swing.current =
            clamp01(swing.tick / duration)

        if swing.tick >= duration then
            -- Keep phase 1.0 after completion. blockhit_curve(1) is already
            -- zero, while resetting the phase here would interpolate backward
            -- through the entire animation and then snap on the next tick.
            swing.tick = duration
            swing.active = false
        end
    end

    swing.attack_was_down = attack_down
end)

game.events.on("animation.first_person.calculate", {
    priority = 500,

    callback = function(event)
        if not config.get("enabled") then
            return
        end

        if not is_sword_block_event(event) then
            return
        end

        local delta =
            swing.current - swing.previous

        -- 结束时避免把 1 -> 0 插值成错误回绕。
        if delta < -0.5 then
            delta = 0.0
        end

        local partial_tick =
            clamp01(event.state.partial_tick or 0.0)

        local interpolated =
            swing.previous + delta * partial_tick

        event:set_swing_progress(
            clamp01(interpolated)
        )

        event:set_swinging(
            swing.active
        )

        if event.vanilla then
            -- 保留基础持剑、装备和静态格挡姿势。
            event.vanilla:set_base_enabled(true)
            event.vanilla:set_equip_enabled(true)
            event.vanilla:set_use_enabled(true)
            event.vanilla:set_block_enabled(true)

            -- The vanilla swing chain supplies the short forward push and
            -- diagonal recoil seen in 1.7 BlockHit.
            event.vanilla:set_swing_enabled(true)
        end
    end
})

game.events.on("animation.first_person.transform", {
    priority = -10000,

    callback = function(event)
        if not config.get("enabled") then
            return
        end

        if not is_sword_block_event(event) then
            return
        end

        local progress =
            blockhit_curve(event.state.swing_progress)

        if progress <= 0.0001 then
            return
        end

        local pivot_x =
            tonumber(config.get("pivot_x")) or 0.02

        local pivot_y =
            tonumber(config.get("pivot_y")) or -0.42

        local pivot_z =
            tonumber(config.get("pivot_z")) or 0.0

        -- Older config files store this as a positive recoil amount. Normalize
        -- both old positive values and the short-lived negative format.
        local angle_strength = math.abs(
            tonumber(config.get("blockhit_angle")) or 14
        )
        -- Positive Z rotation follows the original 1.7 BlockHit direction.
        local angle = clamp(angle_strength, 0.0, 35.0)
            * progress

        local sweep_x =
            (tonumber(config.get("sweep_x")) or -0.32)
            * progress

        local sweep_y =
            (tonumber(config.get("sweep_y")) or 0.40)
            * progress

        -- 围绕护手点做平面内旋转：
        --
        -- M' = M * T(P) * Rz(angle) * T(-P)
        --
        -- 只旋转 Z 轴，不使用 rotate_x / rotate_y。
        event.transform:local_translate(
            pivot_x,
            pivot_y,
            pivot_z
        )

        event.transform:local_rotate_z(angle)

        event.transform:local_translate(
            -pivot_x,
            -pivot_y,
            -pivot_z
        )

        -- Apply the sweep in the pre-rotation parent space. local_translate
        -- would rotate the Y offset with the blade and could send the sword
        -- below the viewport at larger recoil angles.
        event.transform:translate(
            sweep_x,
            sweep_y,
            0.0
        )
    end
})

game.events.on("render.hud.after", function(event)
    if not config.get("enabled") then
        return
    end

    if config.get("hud_indicator") ~= "always" then
        return
    end

    event.draw:text({
        text = "1.7 BlockHit Enabled",
        x = 8,
        y = 8,
        scale = 1.0,
        color = {
            r = 1.0,
            g = 1.0,
            b = 1.0,
            a = 0.85
        }
    })
end)

function on_load()
    reset_swing()
    game.log.info("1.7 planar BlockHit loaded")
end

function on_unload()
    reset_swing()
    game.log.info("1.7 planar BlockHit unloaded")
end
