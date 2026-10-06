//@ pragma UseQApplication
//@ pragma IconTheme Adwaita
import qs
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io

ShellRoot {
    id: shell

    Variants {
        id: bars

        model: Quickshell.screens

        Bar {
            required property var modelData
            screen: modelData
        }
    }

    // Opens/closes a panel on the focused monitor.
    function toggle(name) {
        const monitor = Hyprland.focusedMonitor?.name;
        const bar = bars.instances.find(b => b.screen.name === monitor) ?? bars.instances[0];
        bar?.toggle(name);
    }

    // Keybinds: Hyprland global shortcuts ("quickshell:<panel>") go straight to this process,
    // no `qs ipc` process per keypress. Bound in hyprland/lua/general.lua.
    // qmllint disable unresolved-type import required
    Instantiator {
        model: ["launcher", "clip", "op", "notif", "session", "cal"]

        GlobalShortcut {
            required property string modelData

            appid: "quickshell"
            name: modelData
            onPressed: shell.toggle(modelData)
            // The launcher is a release bind (tap SUPER), and Hyprland's global dispatcher
            // forwards a release bind as a "released" event, so react to that too.
            onReleased: if (modelData === "launcher") shell.toggle(modelData)
        }
    }

    // qmllint enable unresolved-type import required

    // `qs ipc call bar toggle vol` does the same from scripts.
    IpcHandler {
        target: "bar"

        function toggle(name: string): void {
            shell.toggle(name);
        }

        function dnd(): void {
            Notifs.dnd = !Notifs.dnd;
        }
    }
}
