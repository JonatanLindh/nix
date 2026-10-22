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

    // The drag shelf; shows on one monitor at a time (see Shelf.screenName).
    Variants {
        model: Quickshell.screens

        ShelfWindow {}
    }

    // Screenshot / screen recording overlay (see Screenshot.qml).
    Variants {
        model: Quickshell.screens
        ScreenshotOverlay {}
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

    // The colour picker runs straight away (no panel) and puts its slot in the bar.
    GlobalShortcut {
        appid: "quickshell"
        name: "picker"
        onPressed: ColorPicker.pick()
    }

    // The shelf, on the focused monitor.
    GlobalShortcut {
        appid: "quickshell"
        name: "shelf"
        onPressed: Shelf.toggle()
    }

    // Print opens the capture overlay; while recording, it stops the recording instead.
    GlobalShortcut {
        appid: "quickshell"
        name: "screenshot"
        onPressed: Screenshot.open("selection")
    }
    GlobalShortcut {
        appid: "quickshell"
        name: "screenshot-window"
        onPressed: Screenshot.open("window")
    }
    GlobalShortcut {
        appid: "quickshell"
        name: "screenshot-screen"
        onPressed: Screenshot.open("screen")
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
