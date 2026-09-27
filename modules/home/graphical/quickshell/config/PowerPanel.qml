pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell

Panel {
    id: root

    name: "pwr"
    title: "pwr"

    function run(command) {
        Quickshell.execDetached(command);
        close();
    }

    actions: BarText {
        visible: Power.hasBattery
        text: Power.charging ? "charging" : Power.onAc ? "on ac" : "discharging"
        color: Theme.label
    }

    Meter {
        visible: Power.hasBattery
        label: "battery"
        value: Power.percent / 100
        valueText: Power.percent + "%" + (Power.timeLeft ? "  " + Power.timeLeft : "")
        interactive: false
        fill: Power.low ? Theme.warn : Theme.text
    }

    Section {
        visible: Power.profilesAvailable
        text: "profile"
    }

    Repeater {
        model: Power.profilesAvailable ? Power.profiles : []

        PanelRow {
            required property int modelData
            text: Power.shortName(modelData)
            detail: Power.longName(modelData)
            selected: Power.profile === modelData
            onClicked: Power.setProfile(modelData)
        }
    }

    Section {
        text: "caf"

        Toggle {
            checked: Caf.enabled
            onToggled: Caf.enabled = !Caf.enabled
        }
    }

    Note {
        text: "keep the screen on and skip auto-lock"
    }

    footer: [
        Action {
            text: "lock"
            onClicked: root.run(["loginctl", "lock-session"])
        },
        Action {
            text: "suspend"
            onClicked: root.run(["systemctl", "suspend"])
        },
        Item {
            Layout.fillWidth: true
        },
        Action {
            // The full session menu (super+m).
            text: "session →"
            onClicked: root.bar.panel = "session"
        }
    ]
}
