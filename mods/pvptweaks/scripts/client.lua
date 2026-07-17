local config = game.config.define({
    { key = "hurtcam_enabled", type = "boolean", label = "Hurt Camera", default = true },
    { key = "fovchange_enabled", type = "boolean", label = "FOV Changes", default = true },
})

local function apply()
    game.client.set_hurtcam_override(config.get("hurtcam_enabled"))
    game.client.set_fovchange_override(config.get("fovchange_enabled"))
end

game.events.on("client.tick", function(_)
    apply()
end)

apply()
