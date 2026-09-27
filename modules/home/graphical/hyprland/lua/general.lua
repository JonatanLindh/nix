require("lib")

-- Monitors
local mon_left  = "desc:Technical Concepts Ltd 27R83U X2414000812"
local mon_right = "desc:Technical Concepts Ltd 27R83U X2414000123"

hl.monitor({
    output              = mon_left,
    mode                = "highres @ highrr",
    position            = "0x0",
    scale               = "auto",
    bitdepth            = 10,
    supports_wide_color = true,
    supports_hdr        = true,
})

hl.monitor({
    output              = mon_right,
    mode                = "highres @ highrr",
    position            = "auto",
    scale               = "auto",
    bitdepth            = 10,
    supports_wide_color = true,
    supports_hdr        = true,
})

hl.monitor({ output = "", mode = "preferred", position = "auto", scale = 1 })

hl.config({
    general = {
        gaps_in          = 4,
        gaps_out         = 5,
        border_size      = 2,
        col              = {
            active_border   = { colors = { "rgba(33ccffee)", "rgba(00ff99ee)" }, angle = 45 },
            inactive_border = "rgba(595959aa)",
        },
        resize_on_border = false,
        allow_tearing    = false,
        layout           = "dwindle",
    },
    decoration = {
        rounding         = 7,
        rounding_power   = 2,
        active_opacity   = 1.0,
        inactive_opacity = 1.0,

        shadow           = {
            enabled      = false,
            range        = 4,
            render_power = 3,
            color        = "rgba(1a1a1aee)",
        },

        blur             = {
            enabled = true,
            xray    = true,
            size    = 3,
            passes  = 1,
        },
    },
    animations = {
        enabled = true,
    },
    dwindle = {
        preserve_split = true,
    },
    master = {
        new_status = "master",
    },
    misc = {
        disable_hyprland_logo = true,
    },
    input = {
        kb_layout    = "se",
        kb_options   = "caps:escape_shifted_capslock",
        follow_mouse = 1,
        touchpad     = {
            natural_scroll       = true,
            disable_while_typing = true,
        },
    },
    xwayland = {
        force_zero_scaling     = true,
        create_abstract_socket = true,
    },
})

-- Animations
hl.curve("easeOutQuint", { type = "bezier", points = { { 0.23, 1 }, { 0.32, 1 } } })
hl.curve("easeInOutCubic", { type = "bezier", points = { { 0.65, 0.05 }, { 0.36, 1 } } })
hl.curve("linear", { type = "bezier", points = { { 0, 0 }, { 1, 1 } } })
hl.curve("almostLinear", { type = "bezier", points = { { 0.5, 0.5 }, { 0.75, 1 } } })
hl.curve("quick", { type = "bezier", points = { { 0.15, 0 }, { 0.1, 1 } } })

hl.animation({ leaf = "global", enabled = true, speed = 10, bezier = "default" })
hl.animation({ leaf = "border", enabled = true, speed = 5.39, bezier = "easeOutQuint" })
hl.animation({ leaf = "windows", enabled = true, speed = 4.79, bezier = "easeOutQuint" })
hl.animation({ leaf = "windowsIn", enabled = true, speed = 4.1, bezier = "easeOutQuint", style = "popin 87%" })
hl.animation({ leaf = "windowsOut", enabled = true, speed = 1.49, bezier = "linear", style = "popin 87%" })
hl.animation({ leaf = "fadeIn", enabled = true, speed = 1.73, bezier = "almostLinear" })
hl.animation({ leaf = "fadeOut", enabled = true, speed = 1.46, bezier = "almostLinear" })
hl.animation({ leaf = "fade", enabled = true, speed = 3.03, bezier = "quick" })
hl.animation({ leaf = "layers", enabled = true, speed = 3.81, bezier = "easeOutQuint" })
hl.animation({ leaf = "layersIn", enabled = true, speed = 4, bezier = "easeOutQuint", style = "fade" })
hl.animation({ leaf = "layersOut", enabled = true, speed = 1.5, bezier = "linear", style = "fade" })
hl.animation({ leaf = "fadeLayersIn", enabled = true, speed = 1.79, bezier = "almostLinear" })
hl.animation({ leaf = "fadeLayersOut", enabled = true, speed = 1.39, bezier = "almostLinear" })
hl.animation({ leaf = "workspaces", enabled = true, speed = 1.94, bezier = "almostLinear", style = "fade" })
hl.animation({ leaf = "workspacesIn", enabled = true, speed = 1.21, bezier = "almostLinear", style = "fade" })
hl.animation({ leaf = "workspacesOut", enabled = true, speed = 1.94, bezier = "almostLinear", style = "fade" })
hl.animation({ leaf = "zoomFactor", enabled = true, speed = 7, bezier = "quick" })

-- Workspace monitor assignments
hl.workspace_rule({ workspace = "1", monitor = mon_right, default = true, on_created_empty = LaunchOnce "zen-beta" })
hl.workspace_rule({ workspace = "2", monitor = mon_right })
hl.workspace_rule({ workspace = "3", monitor = mon_right })
hl.workspace_rule({ workspace = "4", monitor = mon_right })
hl.workspace_rule({ workspace = "5", monitor = mon_right })
hl.workspace_rule({ workspace = "6", monitor = mon_left, default = true })
hl.workspace_rule({ workspace = "7", monitor = mon_left })
hl.workspace_rule({ workspace = "8", monitor = mon_left })
hl.workspace_rule({ workspace = "9", monitor = mon_left, on_created_empty = LaunchOnce "spotify" })
hl.workspace_rule({ workspace = "10", monitor = mon_left, on_created_empty = LaunchOnce "vesktop" })

-- Smart gaps
hl.workspace_rule({ workspace = "w[tv1]", gaps_out = 0, gaps_in = 0 })
hl.workspace_rule({ workspace = "f[1]", gaps_out = 0, gaps_in = 0 })

-- Window rules
hl.window_rule({
    name        = "no-gaps-wtv1",
    match       = { float = false, workspace = "w[tv1]" },
    border_size = 0,
    rounding    = 0,
})

hl.window_rule({
    name        = "no-gaps-f1",
    match       = { float = false, workspace = "f[1]" },
    border_size = 0,
    rounding    = 0,
})

hl.window_rule({
    name           = "suppress-maximize-events",
    match          = { class = ".*" },
    suppress_event = "maximize",
})

hl.window_rule({
    name     = "fix-xwayland-drags",
    match    = {
        class      = "^$",
        title      = "^$",
        xwayland   = true,
        float      = true,
        fullscreen = false,
        pin        = false,
    },
    no_focus = true,
})

-- Layer rules
hl.layer_rule({
    name  = "waybar-blur",
    match = { namespace = "waybar" },
    blur  = true,
})

-- Input devices
hl.device({
    name        = "epic-mouse-v1",
    sensitivity = -0.5,
})

hl.device({
    name = "wacom-intuos-bt-m-pen",
    output = mon_right
})

-- Gestures
hl.gesture({
    fingers   = 3,
    direction = "horizontal",
    action    = "workspace",
})

-- Keybindings
hl.bind("SUPER + Q", hl.dsp.exec_cmd(Launch "ghostty"))
hl.bind("SUPER + C", hl.dsp.window.close())
hl.bind("SUPER + E", hl.dsp.exec_cmd(Launch "nautilus"))
hl.bind("SUPER + P", hl.dsp.window.float({ action = "toggle" }))
hl.bind(
    "SUPER + SUPER_L",
    hl.dsp.exec_cmd(LaunchOnce("tofi-drun", "--drun-launch=true", "pkill")),
    { release = true }
)
hl.bind("SUPER + J", hl.dsp.group.toggle())
hl.bind("SUPER + F", hl.dsp.window.fullscreen(0))
hl.bind("SUPER + L", function()
    local ws = hl.get_active_workspace()
    local next_layout = ws.tiled_layout == "dwindle" and "scrolling" or "dwindle"
    hl.workspace_rule({ workspace = ws.name, layout = next_layout })
    Debug("Layout (" .. ws.name .. "): " .. next_layout)
end)
hl.bind("SUPER + M", hl.dsp.exec_cmd(Launch "wleave"))

hl.bind("SUPER + left", hl.dsp.focus({ direction = "left" }))
hl.bind("SUPER + right", hl.dsp.focus({ direction = "right" }))
hl.bind("SUPER + up", hl.dsp.focus({ direction = "up" }))
hl.bind("SUPER + down", hl.dsp.focus({ direction = "down" }))

for i = 1, 10 do
    local key = i % 10
    hl.bind("SUPER + " .. key, hl.dsp.focus({ workspace = i }))
    hl.bind("SUPER + SHIFT + " .. key, hl.dsp.window.move({ workspace = i }))
end

hl.bind("SUPER + S", hl.dsp.workspace.toggle_special("magic"))
hl.bind("SUPER + SHIFT + S", hl.dsp.window.move({ workspace = "special:magic" }))

hl.bind("SUPER + mouse_down", hl.dsp.focus({ workspace = "e-1" }))
hl.bind("SUPER + mouse_up", hl.dsp.focus({ workspace = "e+1" }))

hl.bind("SUPER + mouse:272", hl.dsp.window.drag(), { mouse = true })
hl.bind("SUPER + mouse:273", hl.dsp.window.resize(), { mouse = true })

hl.bind("XF86AudioRaiseVolume", hl.dsp.exec_cmd(Launch "wpctl set-volume -l 1 @DEFAULT_AUDIO_SINK@ 5%+"),
    { locked = true, repeating = true })
hl.bind("XF86AudioLowerVolume", hl.dsp.exec_cmd(Launch "wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-"),
    { locked = true, repeating = true })
hl.bind("XF86AudioMute", hl.dsp.exec_cmd(Launch "wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle"),
    { locked = true, repeating = true })
hl.bind("XF86AudioMicMute", hl.dsp.exec_cmd(Launch "wpctl set-mute @DEFAULT_AUDIO_SOURCE@ toggle"),
    { locked = true, repeating = true })
hl.bind("XF86MonBrightnessUp", hl.dsp.exec_cmd(Launch "brightnessctl -e4 -n2 set 5%+"),
    { locked = true, repeating = true })
hl.bind("XF86MonBrightnessDown", hl.dsp.exec_cmd(Launch "brightnessctl -e4 -n2 set 5%-"),
    { locked = true, repeating = true })

hl.bind("XF86AudioNext", hl.dsp.exec_cmd(Launch "playerctl next"), { locked = true })
hl.bind("XF86AudioPause", hl.dsp.exec_cmd(Launch "playerctl play-pause"), { locked = true })
hl.bind("XF86AudioPlay", hl.dsp.exec_cmd(Launch "playerctl play-pause"), { locked = true })
hl.bind("XF86AudioPrev", hl.dsp.exec_cmd(Launch "playerctl previous"), { locked = true })

hl.bind("SUPER + PRINT", hl.dsp.exec_cmd(Launch "hyprshot -m window"))
hl.bind("PRINT", hl.dsp.exec_cmd(Launch "hyprshot -m output"))
hl.bind("SUPER + SHIFT + PRINT", hl.dsp.exec_cmd(Launch "hyprshot -m region"))
