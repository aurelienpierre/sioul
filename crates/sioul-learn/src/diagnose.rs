// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What the header features say of each account's mail (`sioul spam
//! features`): each feature's mean over the corpus, by account and label,
//! and how often it is known. A feature whose mean differs from one account
//! to another more than from ham to spam tells the classifier where mail was
//! collected rather than what it is: it learns each account's share of spam
//! instead of spam. Optionally, only the mail of one sender's domain: what
//! the features carry of a bank's or a provider's real messages. Numbers
//! only, never a message.

use crate::labels::{self, Copy, Label};
use crate::spamcore::{N, NAMES};
use crate::{Dirs, LearnError, corpus, train};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

/// One account's mail of one label: how many, each feature's mean over
/// those where it is known, and how many those are.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Group {
    pub account: String,
    pub label: Label,
    pub messages: u64,
    /// By feature, in `NAMES` order; none when no message knows it.
    pub means: Vec<Option<f64>>,
    pub known: Vec<u64>,
}

/// The feature names, in the order of `Group::means`.
pub fn names() -> &'static [&'static str] {
    &NAMES
}

/// Each account's and label's feature means, over the copies a training
/// learns from; only the mail of `domain` (the sender's registrable domain,
/// or one under it) when given.
pub fn feature_means(dirs: &Dirs, trusted: &BTreeMap<String, Vec<String>>, domain: Option<&str>) -> Result<Vec<Group>, LearnError> {
    let mut copies = Vec::new();
    corpus::read_all(dirs, |record| copies.push(Copy::of(&record)))?;
    let (labeled, _) = labels::decide(copies, &labels::read_log(dirs), &labels::read_moved(dirs), &labels::read_flagged(dirs));
    let chosen: HashMap<corpus::Place, Label> = labeled.into_iter().map(|l| (l.place, l.label)).collect();
    let domain = domain.map(|d| d.trim().trim_start_matches('@').to_ascii_lowercase());
    // By account and label (spam: true), in order.
    let mut sums: BTreeMap<(String, bool), (u64, [f64; N], [u64; N])> = BTreeMap::new();
    corpus::read_all(dirs, |record| {
        let Some(&label) = chosen.get(&record.place()) else { return };
        if let Some(domain) = &domain {
            let from = labels::from_address(&record.header_bytes()).and_then(|a| a.rsplit_once('@').map(|(_, d)| d.to_string())).unwrap_or_default();
            if !(from == *domain || from.ends_with(&format!(".{domain}"))) {
                return;
            }
        }
        let (_, features) = train::read_message(&record, trusted);
        let group = sums.entry((record.account.clone(), label == Label::Spam)).or_insert((0, [0.0; N], [0; N]));
        group.0 += 1;
        for (h, x) in features.iter().enumerate() {
            if !x.is_nan() {
                group.1[h] += f64::from(*x);
                group.2[h] += 1;
            }
        }
    })?;
    Ok(sums
        .into_iter()
        .map(|((account, spam), (messages, sum, known))| Group {
            account,
            label: if spam { Label::Spam } else { Label::Ham },
            messages,
            means: sum.iter().zip(&known).map(|(s, &k)| (k > 0).then(|| s / k as f64)).collect(),
            known: known.to_vec(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic;

    #[test]
    fn means_by_account_and_label() {
        let root = std::env::temp_dir().join(format!("sioul-learn-diagnose-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dirs = Dirs::under(&root);
        let mail = synthetic::mailbox(2, 60, 40);
        let records: Vec<_> = mail.iter().enumerate().map(|(i, m)| synthetic::record_of(m, 1, i as u32 + 1)).collect();
        corpus::store(&dirs, &records).unwrap();
        let groups = feature_means(&dirs, &BTreeMap::new(), None).unwrap();
        assert_eq!(groups.iter().map(|g| g.messages).sum::<u64>(), 100);
        assert!(groups.iter().all(|g| g.means.len() == N && g.known.len() == N));
        let list = NAMES.iter().position(|n| *n == "list_unsubscribe").unwrap();
        assert!(groups.iter().all(|g| g.means[list].is_some_and(|m| (0.0..=1.0).contains(&m))), "{groups:?}");
        // One sender's domain alone: the invented spam's.
        let lottery = feature_means(&dirs, &BTreeMap::new(), Some("lottery.test")).unwrap();
        assert!(!lottery.is_empty() && lottery.iter().all(|g| g.label == Label::Spam), "{lottery:?}");
        let _ = std::fs::remove_dir_all(root);
    }
}
