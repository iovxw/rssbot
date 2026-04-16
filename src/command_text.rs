#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParsedCommand<'a> {
    pub(crate) command: &'a str,
    pub(crate) mention: Option<&'a str>,
    pub(crate) args: &'a str,
}

pub(crate) fn parse_command_like_tbot(text: &str) -> Option<ParsedCommand<'_>> {
    if !text.starts_with('/') {
        return None;
    }

    let token = text.split_whitespace().next()?;
    let command = token.strip_prefix('/')?;
    if command.is_empty() {
        return None;
    }

    let mut parts = command.split('@');
    let command = parts.next().unwrap();
    let mention = parts.next();
    let args = text[token.len()..].trim_start_matches(|ch: char| ch.is_whitespace());

    Some(ParsedCommand {
        command,
        mention,
        args,
    })
}

pub(crate) fn parse_matching_command<'a>(
    text: &'a str,
    expected_command: &str,
    bot_username: Option<&str>,
) -> Option<&'a str> {
    let parsed = parse_command_like_tbot(text)?;
    let expected_command = expected_command
        .strip_prefix('/')
        .unwrap_or(expected_command);

    if parsed.command != expected_command {
        return None;
    }

    match parsed.mention {
        None => Some(parsed.args),
        Some(mention) if bot_username == Some(mention) => Some(parsed.args),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_command_like_tbot, parse_matching_command, ParsedCommand};

    #[test]
    fn parses_command_and_trimmed_args() {
        assert_eq!(
            parse_command_like_tbot("/sub   https://example.com/rss.xml  title"),
            Some(ParsedCommand {
                command: "sub",
                mention: None,
                args: "https://example.com/rss.xml  title",
            })
        );
    }

    #[test]
    fn parses_command_mention() {
        assert_eq!(
            parse_command_like_tbot("/sub@rssbot"),
            Some(ParsedCommand {
                command: "sub",
                mention: Some("rssbot"),
                args: "",
            })
        );
    }

    #[test]
    fn matches_command_for_current_bot() {
        assert_eq!(
            parse_matching_command("/sub@rssbot https://example.com/rss.xml", "sub", Some("rssbot")),
            Some("https://example.com/rss.xml")
        );
        assert_eq!(
            parse_matching_command("/sub https://example.com/rss.xml", "/sub", Some("rssbot")),
            Some("https://example.com/rss.xml")
        );
    }

    #[test]
    fn ignores_commands_for_other_bots() {
        assert_eq!(
            parse_matching_command("/sub@otherbot https://example.com/rss.xml", "sub", Some("rssbot")),
            None
        );
    }

    #[test]
    fn rejects_non_commands_and_other_command_names() {
        assert_eq!(parse_matching_command("sub https://example.com/rss.xml", "sub", Some("rssbot")), None);
        assert_eq!(parse_matching_command("/rss https://example.com/rss.xml", "sub", Some("rssbot")), None);
    }
}