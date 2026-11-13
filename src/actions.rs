use gpui_kit::{App, KeyBinding, actions};

actions!(
    pi_gpui,
    [
        NewSession,
        StopAgent,
        ToggleSidebar,
        ToggleRightPanel,
        ToggleTerminal,
        OpenFind,
        OpenSettings,
        ToggleTheme,
        FocusComposer,
        SendPrompt,
        SwitchToChat,
        SwitchToTasks,
        SwitchToScheduled,
        SwitchToArtifacts,
        QuitApp,
    ]
);

pub fn register_keybindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-n", NewSession, None),
        KeyBinding::new("ctrl-alt-n", NewSession, None),
        KeyBinding::new("escape", StopAgent, Some("ChatPanel")),
        KeyBinding::new("cmd-b", ToggleSidebar, None),
        KeyBinding::new("cmd-\\", ToggleRightPanel, None),
        KeyBinding::new("ctrl-`", ToggleTerminal, None),
        KeyBinding::new("cmd-f", OpenFind, None),
        KeyBinding::new("cmd-,", OpenSettings, None),
        KeyBinding::new("cmd-enter", SendPrompt, Some("Composer")),
        KeyBinding::new("cmd-q", QuitApp, None),
    ]);
}
