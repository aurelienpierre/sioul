// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! OpenPGP keys from the terminal: `sioul pgp list|make|import|export|lookup|cards`.

use crate::Session;
use crate::accounts::prompt_password;
use clap::Subcommand;
use sioul_core::pgp;
use sioul_core::securitykey::{self, Purpose, Touch};
use sioul_sync::{keys, secret};
use std::path::PathBuf;

#[derive(Subcommand)]
pub(crate) enum PgpCommand {
    /// Your keys, then others'.
    List,
    /// Makes a key for one of your addresses; the keyring keeps its passphrase.
    Make { address: String },
    /// Imports the keys of a file; a secret key's passphrase is asked, then kept in the keyring.
    Import { file: PathBuf },
    /// Writes a public key, armored, to the terminal.
    Export { fingerprint: String },
    /// Looks up someone's key: their domain's Web Key Directory, then keys.openpgp.org.
    Lookup { address: String },
    /// What each security key plugged in says, without its PIN: its keys, their
    /// touch, the tries left. Nothing is signed or opened, no PIN is given.
    Cards,
}

pub(crate) fn run(s: &Session, command: PgpCommand) -> Result<(), String> {
    match command {
        PgpCommand::List => {
            for key in pgp::summaries() {
                let mine = if key.own { "*" } else { " " };
                println!("{mine} {} · {} · {}{}", key.fingerprint, crate::one_line(&key.names.join(", ")), key.created, if key.expires.is_empty() { String::new() } else { format!(" → {}", key.expires) });
            }
            Ok(())
        }
        PgpCommand::Make { address } => {
            let name = s.config.accounts.iter().find(|a| a.address.as_deref() == Some(address.as_str())).and_then(|a| a.name.clone()).unwrap_or_default();
            let passphrase = match secret::test_passphrase() {
                Some(fixed) => fixed,
                None => pgp::new_passphrase()?,
            };
            let made = pgp::generate(&name, &address, &passphrase)?;
            secret::save_pgp_passphrase(&made.fingerprint, &passphrase).map_err(|e| e.sentence(&s.tr, &address))?;
            println!("{}", made.fingerprint);
            Ok(())
        }
        PgpCommand::Import { file } => {
            let bytes = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            // Your keys before: one imported again with a mistyped passphrase stays yours.
            let had: Vec<String> = pgp::summaries().into_iter().filter(|k| k.own).map(|k| k.fingerprint).collect();
            for key in pgp::import(&bytes)? {
                if key.own {
                    let passphrase = prompt_password(s, &crate::one_line(&key.names.join(", ")))?;
                    if !pgp::opens(&key.fingerprint, &passphrase) {
                        if !had.contains(&key.fingerprint) {
                            let _ = pgp::remove(&key.fingerprint);
                        }
                        return Err(s.tr.text("pgp-wrong-passphrase", None));
                    }
                    secret::save_pgp_passphrase(&key.fingerprint, &passphrase).map_err(|e| e.sentence(&s.tr, &key.fingerprint))?;
                }
                println!("{} · {}", key.fingerprint, crate::one_line(&key.names.join(", ")));
            }
            Ok(())
        }
        PgpCommand::Export { fingerprint } => {
            print!("{}", pgp::export_public(&fingerprint).ok_or_else(|| s.tr.text("pgp-no-key", None))?);
            Ok(())
        }
        PgpCommand::Cards => {
            let cards = securitykey::cards(sioul_sync::securitykey::readers()).map_err(|e| e.sentence(&s.tr, Purpose::Read))?;
            let known = securitykey::known_cards();
            let touch = |touch: Touch| {
                let policy = match touch {
                    Touch::Off => "off",
                    Touch::On => "on",
                    Touch::Fixed => "fixed",
                    Touch::Cached => "cached",
                    Touch::CachedFixed => "cached-fixed",
                    Touch::Unknown => "unknown",
                };
                s.say("seckey-cli-touch", &[("policy", policy.to_string())])
            };
            for card in cards {
                // Known to Sioul, as `list` marks your keys.
                let mine = if known.iter().any(|k| k.ident.eq_ignore_ascii_case(&card.ident)) { "*" } else { " " };
                println!("{mine} {} · {} · {}", card.ident, card.label(&s.tr), crate::one_line(&card.holder));
                for (id, slot) in [("seckey-cli-signs", &card.sign), ("seckey-cli-decrypts", &card.decrypt)] {
                    if let Some(slot) = slot {
                        let date = securitykey::full_date(&s.tr, i64::from(slot.created));
                        println!("    {}", s.say(id, &[("fingerprint", slot.fingerprint.clone()), ("algorithm", slot.algorithm.clone()), ("date", date), ("touch", touch(slot.touch))]));
                    }
                }
                println!("    {}", s.say("seckey-tries-left", &[("n", card.pin_tries.to_string())]));
                if card.pin_each_signature {
                    println!("    {}", s.tr.text("seckey-pin-each", None));
                }
                println!("    {}", s.say("seckey-cli-codes", &[("reset", card.reset_tries.to_string()), ("admin", card.admin_tries.to_string())]));
                if !card.url.is_empty() {
                    println!("    {}", crate::one_line(&card.url));
                }
                println!("    {}", crate::one_line(&card.reader));
            }
            Ok(())
        }
        PgpCommand::Lookup { address } => {
            let found = keys::lookup(&address).map_err(|e| e.sentence(&s.tr, &address))?;
            if found.is_empty() {
                println!("{}", s.say("pgp-lookup-some", &[("missing", address)]));
            }
            for key in found {
                println!("{} · {}", key.fingerprint, crate::one_line(&key.names.join(", ")));
            }
            Ok(())
        }
    }
}
