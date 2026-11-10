// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The command line's help in the session's language (docs/i18n.md, "The
//! command line"). Clap writes each command's and each option's help from
//! the code's comments, in English. Before the arguments are read, each text
//! this language has a message for takes its place: `cli-<command path>` for
//! a command (`cli-remind`, `cli-mail-send`; the program itself `cli`), and
//! `cli-<command path>--<argument>` for an argument, its `_` written `-`
//! (`cli-remind--watch`, `cli-event--all-day`, `cli--language` for a global
//! one), in `locales/<language>/sioul.ftl`.
//! What clap writes itself follows: "Usage:", "Commands:", and the `-h`,
//! `-V` and `help` it adds; "Arguments" and "Options" read the same in
//! French. A text without a message keeps its English; English has none of
//! these messages, the comments being its words. The help's structure (its
//! sections, their order, the columns) is clap's own, unchanged.

use clap::Command;
use sioul_core::i18n::Translator;

/// The language the help is written in, from the arguments as given, before
/// clap reads them: `--language X` (or `--language=X`) and `--config X`,
/// anywhere before `--`, as their global options are; the last one wins.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Early {
    pub language: Option<String>,
    pub config: Option<std::path::PathBuf>,
}

impl Early {
    pub(crate) fn of(args: impl IntoIterator<Item = std::ffi::OsString>) -> Early {
        let mut early = Early::default();
        let mut args = args.into_iter().skip(1).map(|a| a.to_string_lossy().into_owned());
        while let Some(arg) = args.next() {
            if arg == "--" {
                break;
            }
            for (name, slot) in [("--language", 0), ("--config", 1)] {
                let value = match arg.strip_prefix(name) {
                    Some("") => args.next(),
                    Some(rest) if rest.starts_with('=') => Some(rest[1..].to_string()),
                    _ => continue,
                };
                if let Some(value) = value {
                    match slot {
                        0 => early.language = Some(value),
                        _ => early.config = Some(value.into()),
                    }
                }
            }
        }
        early
    }
}

/// `command` with its help in `tr`'s language where it has the words.
pub(crate) fn translated(command: Command, tr: &Translator) -> Command {
    let mut command = words(command, "cli", tr);
    command.build();
    if let (Some(usage), Some(commands)) = (tr.own("cli-heading-usage"), tr.own("cli-heading-commands")) {
        // Clap keeps a heading for the program's life: one small string, once.
        let commands: &'static str = Box::leak(commands.into_boxed_str());
        clap_s_own(&mut command, tr, &usage, commands);
    }
    command
}

/// The texts the code gives, command by command: its about, its arguments'.
fn words(command: Command, key: &str, tr: &Translator) -> Command {
    let mut command = command;
    if let Some(about) = tr.own(key) {
        command = command.about(about).long_about(None::<&str>);
    }
    let ids: Vec<String> = command.get_arguments().map(|a| a.get_id().as_str().to_string()).collect();
    for id in ids {
        if let Some(help) = tr.own(&format!("{key}--{}", id.replace('_', "-"))) {
            command = command.mut_arg(id, |arg| arg.help(help).long_help(None::<&str>));
        }
    }
    let names: Vec<String> = command.get_subcommands().map(|c| c.get_name().to_string()).collect();
    for name in names {
        let below = format!("{key}-{name}");
        command = command.mut_subcommand(name, |sub| words(sub, &below, tr));
    }
    command
}

/// What clap writes itself, once it is built: the usage's and the commands'
/// headings, its `-h`, `-V` and `help`, in every command, those under `help` too.
fn clap_s_own(command: &mut Command, tr: &Translator, usage: &str, commands: &'static str) {
    let mut owned = std::mem::take(command);
    // As clap's own template, its "Usage:" written in words (clap writes it in English).
    let all_args = owned.get_arguments().any(|a| !a.is_hide_set()) || owned.has_subcommands();
    let template = if all_args { format!("{{before-help}}{{about-with-newline}}\n{usage} {{usage}}\n\n{{all-args}}{{after-help}}") } else { format!("{{before-help}}{{about-with-newline}}\n{usage} {{usage}}{{after-help}}") };
    owned = owned.help_template(template).subcommand_help_heading(commands);
    let has = |command: &Command, id: &str| command.get_arguments().any(|a| a.get_id() == id);
    if has(&owned, "help") {
        let long = owned.get_arguments().find(|a| a.get_id() == "help").is_some_and(|a| a.get_long_help().is_some());
        let (short, long_words) = if long { (tr.own("cli-help-more"), tr.own("cli-help-summary")) } else { (tr.own("cli-help"), None) };
        owned = owned.mut_arg("help", |arg| {
            let arg = match short {
                Some(short) => arg.help(short),
                None => arg,
            };
            match long_words {
                Some(long) => arg.long_help(long),
                None => arg,
            }
        });
    }
    if has(&owned, "version")
        && let Some(version) = tr.own("cli-version")
    {
        owned = owned.mut_arg("version", |arg| arg.help(version));
    }
    *command = owned;
    let help_about = tr.own("cli-help-command");
    let help_for = tr.own("cli-help-command--subcommand");
    for sub in command.get_subcommands_mut() {
        if sub.get_name() == "help" && sub.get_about().is_some_and(|about| about.to_string() == "Print this message or the help of the given subcommand(s)") {
            let mut owned = std::mem::take(sub);
            if let Some(about) = &help_about {
                owned = owned.about(about.clone());
            }
            if let Some(help) = &help_for
                && owned.get_arguments().any(|a| a.get_id() == "subcommand")
            {
                owned = owned.mut_arg("subcommand", |arg| arg.help(help.clone()));
            }
            *sub = owned;
        }
        clap_s_own(sub, tr, usage, commands);
    }
}

/// Every text the help shows that the code gives, with its message's name:
/// what a language translates (the tests ask French for each).
#[cfg(test)]
pub(crate) fn keys(command: &Command, key: &str, out: &mut Vec<(String, String)>) {
    if let Some(about) = command.get_about() {
        out.push((key.to_string(), about.to_string()));
    }
    for arg in command.get_arguments() {
        if let Some(help) = arg.get_help() {
            out.push((format!("{key}--{}", arg.get_id().as_str().replace('_', "-")), help.to_string()));
        }
    }
    for sub in command.get_subcommands() {
        keys(sub, &format!("{key}-{}", sub.get_name()), out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// The language comes from the arguments as given, wherever the global
    /// options stand, before `--`.
    #[test]
    fn the_help_s_language_comes_from_the_arguments_as_given() {
        let of = |args: &[&str]| Early::of(args.iter().map(std::ffi::OsString::from));
        assert_eq!(of(&["sioul", "remind", "--help"]), Early::default());
        assert_eq!(of(&["sioul", "--language", "fr", "remind", "--help"]).language.as_deref(), Some("fr"));
        assert_eq!(of(&["sioul", "remind", "--language=fr", "--config", "/x/c.toml"]), Early { language: Some("fr".into()), config: Some("/x/c.toml".into()) });
        assert_eq!(of(&["sioul", "notes", "--", "--language", "fr"]), Early::default(), "after --, words");
        assert_eq!(of(&["sioul", "--languages", "fr"]), Early::default(), "another option");
    }

    /// TODO §4 (10 November 2026): every text of the help has its French,
    /// and every French message names a text the help has (none left over
    /// from a command gone).
    #[test]
    fn every_help_text_has_its_french() {
        let mut command = crate::Cli::command();
        command.build();
        let mut found = Vec::new();
        keys(&crate::Cli::command(), "cli", &mut found);
        let fr = Translator::new("fr");
        let missing: Vec<String> = found.iter().filter(|(key, _)| fr.own(key).is_none()).map(|(key, english)| format!("{key} = {english}")).collect();
        assert!(missing.is_empty(), "{} without French:\n{}", missing.len(), missing.join("\n"));
        // The help's own section of the French file (other `cli-` messages, "[Y/n]", are the command line's sentences).
        let source = include_str!("../../sioul-core/locales/fr/sioul.ftl");
        let section = source.split("\n## L’aide de la ligne de commande").nth(1).expect("the help's section").split("\n## ").next().unwrap_or("");
        let clap_s = ["cli-heading-usage", "cli-heading-commands", "cli-help", "cli-help-more", "cli-help-summary", "cli-version", "cli-help-command", "cli-help-command--subcommand"];
        let stale: Vec<&str> = section.lines().filter_map(|line| line.split_once(" = ").map(|(key, _)| key.trim())).filter(|key| !clap_s.contains(key) && !found.iter().any(|(k, _)| k == key)).collect();
        assert!(stale.is_empty(), "French for no text of the help: {stale:?}");
        // English has none: the comments are its words.
        let en = Translator::new("en");
        assert!(found.iter().all(|(key, _)| en.own(key).is_none()));
    }

    /// The help in French keeps clap's structure: the same sections in the
    /// same order, the same commands and options, each line in its column;
    /// in English it is clap's own, word for word.
    #[test]
    fn the_help_in_french_keeps_its_structure() {
        let render = |language: &str, path: &[&str], long: bool| {
            let mut command = translated(crate::Cli::command(), &Translator::new(language));
            let mut at = &mut command;
            for name in path {
                at = at.find_subcommand_mut(name).unwrap();
            }
            let text = if long { at.render_long_help() } else { at.render_help() };
            text.to_string()
        };
        let english = |path: &[&str]| {
            let mut command = crate::Cli::command();
            command.build();
            let mut at = &mut command;
            for name in path {
                at = at.find_subcommand_mut(name).unwrap();
            }
            at.render_help().to_string()
        };
        for path in [&[][..], &["remind"], &["mail"], &["mail", "send"], &["spam"], &["tasks"], &["movement"]] {
            assert_eq!(render("en", path, false), english(path), "{path:?}: English is clap's own");
            let (en, fr) = (render("en", path, false), render("fr", path, false));
            // Its shape: the sections in their order, each entry (a command, an
            // argument, an option) in its order; a help too long for its line
            // may go under its entry in one language and beside it in the other.
            let shape = |text: &str| {
                let sections = text.lines().filter(|line| !line.starts_with(' ') && line.ends_with(':')).count();
                let entries: Vec<String> = text
                    .lines()
                    .filter(|line| line.len() - line.trim_start().len() <= 6 && line.starts_with("  "))
                    .filter_map(|line| line.split_whitespace().next())
                    .filter(|word| word.starts_with(['-', '<', '[']) || word.chars().all(|c| c.is_ascii_lowercase() || c == '-'))
                    .map(str::to_string)
                    .collect();
                (sections, entries)
            };
            assert_eq!(shape(&en), shape(&fr), "{path:?}:\n{en}\n{fr}");
            assert!(shape(&fr).1.len() >= 2, "{fr}");
            assert!(fr.contains("Utilisation") && !fr.contains("Usage:") && !fr.contains("Print help"), "{fr}");
            // A command with commands of its own: their heading in words.
            if en.contains("\nCommands:") {
                assert!(fr.contains("\nCommandes:") && !fr.contains("Commands:"), "{fr}");
            }
        }
        let fr = render("fr", &["remind"], false);
        assert!(fr.contains("--watch") && fr.contains("--session") && fr.contains("Reste ouvert"), "{fr}");
        // The long help too, and `help` under a command.
        let long = render("fr", &[], true);
        assert!(long.contains("Affiche l’aide") && !long.contains("Print "), "{long}");
        let help = render("fr", &["help"], false);
        assert!(!help.contains("Print this message"), "{help}");
    }
}
