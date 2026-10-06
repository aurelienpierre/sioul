// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What a task or an event asks of you, and what it gives back (docs/tasks.md,
//! "What it asks, what it gives"; docs/capacity.md). Margins before and after
//! it: getting there, getting ready, coming back; kept free in the plan, never
//! counted as a pause. Rated by you from 0 to 10: what it costs, cognitively,
//! emotionally, in anxiety, and to the body and senses (physical effort,
//! standing, noise, crowds); and what it gives back to your well-being.
//! Written in the task's or the event's own file (`X-SIOUL-BEFORE`,
//! `X-SIOUL-AFTER`, `X-SIOUL-COST`, `X-SIOUL-GAIN`), so that it travels with
//! it and other programs keep it.
//!
//! How it went, said after the task, is kept apart (`X-SIOUL-FELT-COST`,
//! `X-SIOUL-FELT-GAIN`, each dated): ratings given after are more accurate
//! than forecasts, anxiety above all is over-predicted beforehand (Rachman
//! 1994; docs/research/capacity-budget.md, criteria 6–7), so the plan uses
//! them when they exist (`capacity`).

use jiff::civil::Date;
use serde::{Deserialize, Serialize};

pub const BEFORE: &str = "X-SIOUL-BEFORE";
pub const AFTER: &str = "X-SIOUL-AFTER";
pub const COST: &str = "X-SIOUL-COST";
pub const GAIN: &str = "X-SIOUL-GAIN";
/// The costs and the gain as felt, after the task; dated by `FELT_ON`.
pub const FELT_COST: &str = "X-SIOUL-FELT-COST";
pub const FELT_GAIN: &str = "X-SIOUL-FELT-GAIN";
/// The parameter that dates a felt rating: `;X-SIOUL-ON=20261006`.
pub const FELT_ON: &str = "X-SIOUL-ON";

/// Minutes kept before and after: getting there and back, getting ready.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Margins {
    #[serde(default)]
    pub before: u32,
    #[serde(default)]
    pub after: u32,
}

impl Margins {
    pub fn is_empty(self) -> bool {
        self.before == 0 && self.after == 0
    }
}

/// Minutes, as a property's value says them ("20"); at most a day.
pub fn minutes_of(value: &str) -> u32 {
    value.trim().parse::<u32>().unwrap_or(0).min(24 * 60)
}

/// How heavy something is, as one word: from its highest cost when any is
/// rated (`Demands::level`), else as you chose it (`X-SIOUL-ENERGY`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Light,
    Usual,
    Heavy,
    /// It gives back: a gain of 5 or more, no cost above 3 (a walk, music).
    Rest,
}

impl Level {
    /// The word as `X-SIOUL-ENERGY` holds it: "light", "heavy", "rest"; the usual, "".
    pub fn energy(self) -> &'static str {
        match self {
            Level::Light => "light",
            Level::Usual => "",
            Level::Heavy => "heavy",
            Level::Rest => "rest",
        }
    }

    /// As the pages say it: "light", "usual", "heavy", "rest".
    pub fn id(self) -> &'static str {
        match self {
            Level::Light => "light",
            Level::Usual => "usual",
            Level::Heavy => "heavy",
            Level::Rest => "rest",
        }
    }

    /// The word of `X-SIOUL-ENERGY` ("light", "heavy", "rest", "" for the usual).
    pub fn of_energy(word: &str) -> Level {
        match word {
            "light" => Level::Light,
            "heavy" => Level::Heavy,
            "rest" => Level::Rest,
            _ => Level::Usual,
        }
    }

    /// The rating an item counts at when it has none of its own: light 2,
    /// usual 4, heavy 7; what gives back, 2 (nothing is free, little is).
    /// A guess to tune (docs/capacity.md, "Load").
    pub fn rating(self) -> f32 {
        match self {
            Level::Light | Level::Rest => 2.0,
            Level::Usual => 4.0,
            Level::Heavy => 7.0,
        }
    }

    /// Heavy from this rating on; light up to `LIGHT_TOP`. Guesses to tune (docs/capacity.md).
    pub const HEAVY_FROM: u8 = 7;
    pub const LIGHT_TOP: u8 = 3;
    /// A gain from which something with light costs gives back.
    pub const GIVES_FROM: u8 = 5;

    /// The level of a highest cost (0 to 10) with a gain.
    pub fn of(highest: u8, gain: Option<u8>) -> Level {
        if highest <= Level::LIGHT_TOP && gain.is_some_and(|g| g >= Level::GIVES_FROM) {
            Level::Rest
        } else if highest <= Level::LIGHT_TOP {
            Level::Light
        } else if highest >= Level::HEAVY_FROM {
            Level::Heavy
        } else {
            Level::Usual
        }
    }
}

/// The four costs and the gain, each 0 to 10, as you rate them; none unsaid.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Demands {
    #[serde(default)]
    pub cognitive: Option<u8>,
    #[serde(default)]
    pub emotional: Option<u8>,
    #[serde(default)]
    pub anxiety: Option<u8>,
    /// Body and senses: physical effort, standing, sensory load, noise, crowds.
    #[serde(default)]
    pub body: Option<u8>,
    #[serde(default)]
    pub gain: Option<u8>,
}

/// The costs' names as written, in the order of `Demands::costs`.
pub const COST_NAMES: [&str; 4] = ["COGNITIVE", "EMOTIONAL", "ANXIETY", "BODY"];

impl Demands {
    /// The four costs, in order: cognitive, emotional, anxiety, body.
    pub fn costs(&self) -> [Option<u8>; 4] {
        [self.cognitive, self.emotional, self.anxiety, self.body]
    }

    /// The cost at `index` of `costs`, changed.
    pub fn set_cost(&mut self, index: usize, value: Option<u8>) {
        let value = value.map(|v| v.min(10));
        match index {
            0 => self.cognitive = value,
            1 => self.emotional = value,
            2 => self.anxiety = value,
            _ => self.body = value,
        }
    }

    /// Whether any cost is rated.
    pub fn has_cost(&self) -> bool {
        self.costs().iter().any(Option::is_some)
    }

    /// Nothing said at all.
    pub fn is_empty(&self) -> bool {
        !self.has_cost() && self.gain.is_none()
    }

    /// The highest cost rated, or None.
    pub fn highest(&self) -> Option<u8> {
        self.costs().into_iter().flatten().max()
    }

    /// How heavy it is, from every cost rated: its highest. A task is as
    /// heavy as its worst side (docs/research/capacity-budget.md, criterion 9).
    /// None when no cost is rated: then the word chosen stands.
    pub fn level(&self) -> Option<Level> {
        self.highest().map(|h| Level::of(h, self.gain))
    }

    /// Each value 10 at most.
    pub fn bounded(&self) -> Demands {
        let b = |v: Option<u8>| v.map(|v| v.min(10));
        Demands { cognitive: b(self.cognitive), emotional: b(self.emotional), anxiety: b(self.anxiety), body: b(self.body), gain: b(self.gain) }
    }

    /// The costs as written: "COGNITIVE=3;EMOTIONAL=5;ANXIETY=7;BODY=2", the rated ones only.
    pub fn cost_value(&self) -> String {
        COST_NAMES.iter().zip(self.costs()).filter_map(|(name, value)| value.map(|v| format!("{name}={}", v.min(10)))).collect::<Vec<_>>().join(";")
    }

    /// The costs read from what `cost_value` writes; names in any case, any order.
    pub fn read_cost(&mut self, value: &str) {
        for part in value.split(';') {
            let Some((name, number)) = part.split_once('=') else { continue };
            let number = number.trim().parse::<u8>().ok().map(|n| n.min(10));
            if let Some(index) = COST_NAMES.iter().position(|n| name.trim().eq_ignore_ascii_case(n)) {
                self.set_cost(index, number);
            }
        }
    }

    /// The gain read from its property's value.
    pub fn read_gain(&mut self, value: &str) {
        self.gain = value.trim().parse::<u8>().ok().map(|n| n.min(10));
    }
}

/// A rating said after the task, on a day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Felt {
    /// The day it was said; None when its line had no date (another program's).
    pub on: Option<Date>,
    pub demands: Demands,
}

/// "20261006" → the date; anything else, None.
pub fn felt_date(value: &str) -> Option<Date> {
    let value = value.trim();
    if value.len() != 8 {
        return None;
    }
    Date::strptime("%Y%m%d", value).ok()
}

/// The felt ratings of a file's lines, as (name, date parameter, value):
/// one per date, the costs and the gain of a date together; undated ones
/// first, as the oldest, then by date.
pub fn felt_of(lines: &[(String, Option<Date>, String)]) -> Vec<Felt> {
    let mut out: Vec<Felt> = Vec::new();
    for (name, on, value) in lines {
        let index = match out.iter().position(|f| f.on == *on && on.is_some()) {
            Some(i) => i,
            None => {
                out.push(Felt { on: *on, demands: Demands::default() });
                out.len() - 1
            }
        };
        if name.eq_ignore_ascii_case(FELT_COST) {
            out[index].demands.read_cost(value);
        } else if name.eq_ignore_ascii_case(FELT_GAIN) {
            out[index].demands.read_gain(value);
        }
    }
    out.sort_by_key(|f| f.on);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn costs_written_read_and_weighed() {
        let rated = Demands { cognitive: Some(2), emotional: Some(2), anxiety: Some(9), body: Some(4), gain: Some(1) };
        assert_eq!(rated.cost_value(), "COGNITIVE=2;EMOTIONAL=2;ANXIETY=9;BODY=4");
        let mut read = Demands::default();
        read.read_cost("anxiety=9; cognitive=2 ;EMOTIONAL=2;body=4;OTHER=4");
        read.read_gain(" 1 ");
        assert_eq!(read, rated);
        let mut big = Demands::default();
        big.read_cost("COGNITIVE=40;BODY=12");
        assert_eq!((big.cognitive, big.body), (Some(10), Some(10)), "never past 10");
        assert_eq!(minutes_of(" 20 "), 20);
        assert_eq!(minutes_of("3000"), 1440);
        // Older files without BODY read as before; a body alone is written alone.
        let mut old = Demands::default();
        old.read_cost("COGNITIVE=3;EMOTIONAL=5;ANXIETY=7");
        assert_eq!(old.body, None);
        assert_eq!(Demands { body: Some(6), ..Demands::default() }.cost_value(), "BODY=6");
        // JSON without "body" (an older form) reads it as unsaid.
        let json: Demands = serde_json::from_str(r#"{"cognitive":3,"gain":5}"#).unwrap();
        assert_eq!((json.cognitive, json.body, json.gain), (Some(3), None, Some(5)));
    }

    #[test]
    fn heaviness_is_the_highest_cost() {
        let d = |c: Option<u8>, e: Option<u8>, a: Option<u8>, b: Option<u8>, g: Option<u8>| Demands { cognitive: c, emotional: e, anxiety: a, body: b, gain: g };
        // Thresholds: ≤3 light, 4–6 usual, ≥7 heavy.
        assert_eq!(d(Some(3), None, None, None, None).level(), Some(Level::Light));
        assert_eq!(d(Some(4), None, None, None, None).level(), Some(Level::Usual));
        assert_eq!(d(Some(6), None, None, None, None).level(), Some(Level::Usual));
        assert_eq!(d(Some(7), None, None, None, None).level(), Some(Level::Heavy));
        assert_eq!(d(Some(0), None, None, None, None).level(), Some(Level::Light));
        // Every cost counts, body too: one heavy side makes it heavy.
        assert_eq!(d(Some(1), Some(1), Some(1), Some(8), None).level(), Some(Level::Heavy));
        assert_eq!(d(Some(2), Some(2), Some(9), None, Some(8)).level(), Some(Level::Heavy), "a gain never lightens a heavy cost");
        // Gives back: a gain of 5 or more, no cost above 3.
        assert_eq!(d(Some(3), None, None, Some(2), Some(5)).level(), Some(Level::Rest));
        assert_eq!(d(Some(3), None, None, None, Some(4)).level(), Some(Level::Light));
        assert_eq!(d(Some(4), None, None, None, Some(9)).level(), Some(Level::Usual));
        // No cost rated: the word stands.
        assert_eq!(d(None, None, None, None, Some(9)).level(), None);
        assert_eq!(Demands::default().level(), None);
        assert_eq!((Level::Usual.energy(), Level::Rest.energy(), Level::of_energy("heavy"), Level::of_energy("")), ("", "rest", Level::Heavy, Level::Usual));
    }

    #[test]
    fn felt_ratings_by_date() {
        let on = felt_date;
        let lines = vec![
            (FELT_COST.to_string(), on("20261006"), "ANXIETY=3;BODY=1".to_string()),
            (FELT_GAIN.to_string(), on("20261006"), "6".to_string()),
            (FELT_COST.to_string(), on("20260929"), "ANXIETY=6".to_string()),
            (FELT_GAIN.to_string(), None, "2".to_string()),
        ];
        let felt = felt_of(&lines);
        assert_eq!(felt.len(), 3);
        assert_eq!(felt[0], Felt { on: None, demands: Demands { gain: Some(2), ..Demands::default() } }, "undated first, as oldest");
        assert_eq!(felt[1].demands.anxiety, Some(6));
        assert_eq!(felt[2], Felt { on: on("20261006"), demands: Demands { anxiety: Some(3), body: Some(1), gain: Some(6), ..Demands::default() } });
        assert_eq!(felt_date("2026-10-06"), None);
    }
}
