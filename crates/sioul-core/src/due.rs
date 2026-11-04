// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What money is due or expected soon, for the Porch's Money section
//! (docs/porch.md, docs/accounting.md, "Movements"): one plain row per money
//! movement (`movements`), in day order.
//!
//! - **Expected**: what is planned or announced from today to `days` ahead,
//!   money in as visibly as money out.
//! - **Not final yet**: the card holds the bank still lists.
//! - **Not seen**: what was expected in the past month and did not pass on
//!   its account; said once on the Porch, where Dismiss keeps it said.
//!
//! Done movements and transfers between your own accounts are not listed:
//! nothing is due of them. The rows are facts in plain fields; the Porch
//! words them, and follows the hours by each row's area: work money is not
//! said outside work's hours (docs/areas.md), and a budget's title cannot tell it.

use crate::movements::{self, Movement, Place, State, Way};
use jiff::ToSpan;
use jiff::civil::Date;
use serde::Serialize;
use std::path::Path;

/// One movement due or expected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Coming {
    /// Its day: planned, announced, or the bank's for a hold.
    pub day: Date,
    /// Who: the payee or the payer.
    pub label: String,
    /// Signed: + in, − out. The amount said, a range's middle.
    pub amount_cents: i64,
    /// ISO 4217: "EUR".
    pub currency: String,
    /// Said "about": an estimate, a range, a card hold.
    pub about: bool,
    /// The budget it counts in, by its title (parts: each title, joined by ", "); none in none yet.
    pub budget: Option<String>,
    /// What it is for (`movements::Movement::area`): its budgets'; in none, its account's.
    pub area: crate::areas::Area,
    /// planned, announced, not-final or not-seen (`State::id`).
    pub state: State,
    /// Where it opens: `sioul:movement/<its steadiest key>` (`Movement::address`).
    pub address: String,
}

/// How far back what was not seen is still said.
const NOT_SEEN_DAYS: i64 = 31;

/// What is due or expected from `today` to `days` ahead, the holds and what
/// was not seen, from the notes folder the configuration names; none
/// without one.
pub fn coming(today: Date, days: u32) -> Vec<Coming> {
    let config = crate::config::Config::load(&crate::config::default_path()).unwrap_or_default();
    let Some(root) = config.notes_root_path() else { return Vec::new() };
    coming_in(&root, &crate::words::Words::of(&config), today, days)
}

/// The same, from the notes folder `root`, with the words `words`.
pub fn coming_in(root: &Path, words: &crate::words::Words, today: Date, days: u32) -> Vec<Coming> {
    let from = today.checked_sub(NOT_SEEN_DAYS.days()).unwrap_or(today);
    let until = today.checked_add(i64::from(days).days()).unwrap_or(today);
    let ledger = crate::budget::Ledger::load(root).unwrap_or_default();
    let all = movements::load(root, words, from, until, today);
    let title = |id: &str| ledger.budget_title(id).to_string();
    all.list.iter().filter(|m| listed(m, today, until)).map(|m| row(m, &title)).collect()
}

/// Whether `m` is due or expected (see the module's notes).
fn listed(m: &Movement, today: Date, until: Date) -> bool {
    if m.way == Way::Between {
        return false;
    }
    match m.state {
        State::Planned | State::Announced => m.day >= today && m.day <= until,
        State::NotFinal => true,
        State::NotSeen => m.day < today,
        State::Done => false,
    }
}

fn row(m: &Movement, title: &dyn Fn(&str) -> String) -> Coming {
    let budget = match &m.place {
        Place::Transfer { .. } | Place::Nowhere | Place::Unplaced => None,
        place => Some(place.budgets().iter().map(|b| title(b)).collect::<Vec<_>>().join(", ")),
    };
    Coming { day: m.day, label: m.label.clone(), amount_cents: m.sum.amount().cents(), currency: m.currency.clone(), about: m.sum.about(), budget, area: m.area, state: m.state, address: m.address() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(text: &str) -> Date {
        text.parse().unwrap()
    }

    /// A notes folder of its own, with a budget file, a bank's movements and
    /// a store of holds: what is expected over two weeks, the hold, what was
    /// not seen, in day order; nothing done, no transfer.
    #[test]
    fn what_is_coming_over_two_weeks() {
        let root = std::env::temp_dir().join(format!("sioul-due-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join(crate::budget::LEDGER),
            "[[budget]]\nid = 'home'\ntitle = 'Home'\nperiod = 'month'\n\
             [[budget]]\nid = 'work'\ntitle = 'Work'\nperiod = 'month'\narea = 'work'\n\
             [[preset]]\nid = 'rent'\nbudget = 'home'\nlabel = 'Rent'\namount = -600\nevery = 'month'\nday = 1\n\
             [[preset]]\nid = 'edf'\nbudget = 'home'\nlabel = 'EDF'\namount = -52\nevery = 'month'\nday = 12\nabout = true\n\
             [[preset]]\nid = 'insurance'\nbudget = 'home'\nlabel = 'Accident insurance'\namount = -15\nevery = 'month'\nday = 20\n\
             [[preset]]\nid = 'food'\nbudget = 'home'\nlabel = 'Food'\namount = -300\nevery = 'month'\nday = 15\nestimate = true\n\
             [[line]]\nid = 'inv'\nbudget = 'work'\ndate = 2026-11-10\namount = 480\nlabel = 'Client A'\nplanned = true\nlinks = ['sioul:invoice/2026-002']\n\
             [[bank_account]]\nid = 'main'\ntitle = 'Main'\nfills = ['home', 'work']\nexports = ['0123456A']\n",
        )
        .unwrap();
        // The rent passed on 1 November; the insurance did not pass on 20 October; a card hold on the 3rd.
        std::fs::write(
            root.join(crate::bank::MANIFEST),
            "[[account]]\nid = '0123456A'\nbalance = 900.0\nas_of = 2026-11-03\n\
             [[movement]]\naccount = '0123456A'\ndate = 2026-10-01\namount = -600.0\nlabel = 'PRLV SEPA RENT AGENCY'\nid = 'A1'\n\
             [[movement]]\naccount = '0123456A'\ndate = 2026-10-13\namount = -51.2\nlabel = 'PRLV SEPA EDF'\nid = 'A2'\n\
             [[movement]]\naccount = '0123456A'\ndate = 2026-11-01\namount = -600.0\nlabel = 'PRLV SEPA RENT AGENCY'\nid = 'A3'\n\
             [[movement]]\naccount = '0123456A'\ndate = 2026-11-02\namount = -4.5\nlabel = 'CB BAKERY'\nid = 'A4'\n",
        )
        .unwrap();
        std::fs::write(root.join(movements::STORE), "format = 1\n[[hold]]\naccount = '0123456A'\nid = 'H1'\ndate = 2026-11-03\namount = -60.0\nlabel = 'CB STATION EXEMPLE'\n").unwrap();
        let today = day("2026-11-04");
        let rows = coming_in(&root, &crate::words::Words::builtin(), today, 14);
        let seen: Vec<(String, &str, i64, bool, Option<&str>, crate::areas::Area, &str)> = rows.iter().map(|c| (c.day.to_string(), c.label.as_str(), c.amount_cents, c.about, c.budget.as_deref(), c.area, c.state.id())).collect();
        let (admin, work) = (crate::areas::Area::ADMIN, crate::areas::Area::WORK);
        assert_eq!(
            seen,
            vec![
                ("2026-10-20".to_string(), "Accident insurance", -1500, false, Some("Home"), admin, "not-seen"),
                ("2026-11-03".to_string(), "CB STATION EXEMPLE", -6000, true, Some("Home"), admin, "not-final"),
                // Client A pays into Work: said in work's hours only, whatever its budget's title.
                ("2026-11-10".to_string(), "Client A", 48000, false, Some("Work"), work, "announced"),
                ("2026-11-12".to_string(), "EDF", -5200, true, Some("Home"), admin, "planned"),
            ],
            "{rows:#?}"
        );
        // Two days more reach the next insurance.
        let longer = coming_in(&root, &crate::words::Words::builtin(), today, 16);
        assert_eq!(longer.last().map(|c| (c.day.to_string(), c.state)), Some(("2026-11-20".to_string(), State::Planned)));
        // Each opens its movement, by an address any of its sources also find.
        assert_eq!(rows[2].address, crate::links::movement_uri("line:inv"));
        assert!(rows.iter().all(|c| c.currency == "EUR"));
        // Without a notes folder's files: nothing, and no panic.
        assert!(coming_in(&root.join("nowhere"), &crate::words::Words::builtin(), today, 14).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
