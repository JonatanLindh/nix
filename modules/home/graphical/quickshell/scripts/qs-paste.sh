# Paste the clipboard into the focused window. Wayland has no "paste this" request, so this
# sends the app's own paste shortcut through Hyprland: Ctrl+Shift+V for terminals, Ctrl+V
# for everything else. Give focus a moment to come back from the window that just closed.
sleep 0.15
class=$(hyprctl -j activewindow | jq -r '.class // ""')
case "$class" in
    *ghostty* | *kitty* | *[Aa]lacritty* | *foot* | *wezterm* | *konsole* | *[Tt]erminal*) mods="CTRL SHIFT" ;;
    *) mods="CTRL" ;;
esac
hyprctl dispatch "hl.dsp.send_shortcut({ mods = '$mods', key = 'V' })" >/dev/null
