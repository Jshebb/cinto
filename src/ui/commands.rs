pub(super) const COMMAND_TIPS: [(&str, &str); 14] = [
    ("/settings", "open API settings"),
    ("/prompt", "show Harmony prompt"),
    ("/tools", "list agent tools"),
    ("/todos", "show current todo list"),
    ("/git", "show git changes"),
    ("/changes", "show git changes"),
    ("/stage", "stage paths"),
    ("/unstage", "unstage paths"),
    ("/commit", "commit staged changes"),
    ("/diff", "show workspace diff"),
    ("/checkpoint", "save a patch checkpoint"),
    ("/checkpoints", "list patch checkpoints"),
    ("/clear", "clear chat"),
    ("/quit", "exit"),
];

pub(super) fn slash_command_tips(prefix: &str, width: u16) -> String {
    let matches = COMMAND_TIPS
        .iter()
        .filter(|(command, _)| command.starts_with(prefix))
        .collect::<Vec<_>>();

    if matches.is_empty() {
        return "no command matches".to_string();
    }

    let full = matches
        .iter()
        .map(|(command, tip)| format!("{command} {tip}"))
        .collect::<Vec<_>>()
        .join("   ");
    if full.chars().count() <= width as usize {
        return full;
    }

    let compact = matches
        .iter()
        .map(|(command, _)| *command)
        .collect::<Vec<_>>()
        .join(" ");
    if compact.chars().count() <= width as usize {
        compact
    } else {
        compact.chars().take(width as usize).collect()
    }
}
