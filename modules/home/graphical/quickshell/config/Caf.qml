pragma Singleton
import Quickshell

// Shared across all bars (one per screen).
Singleton {
    // Idle inhibit; each bar holds an IdleInhibitor bound to this.
    property bool enabled: false
}
