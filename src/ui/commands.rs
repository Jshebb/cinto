pub(super) const COMMAND_TIPS: [(&str, &str); 6] = [
    ("/settings", "open API settings"),
    ("/prompt", "show Harmony prompt"),
    ("/tools", "list agent tools"),
    ("/todos", "show current todo list"),
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
