// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What a task or an event asks of you, and what it gives back (docs/tasks.md,
//! "What it asks, what it gives"). Margins before and after it: getting there,
//! getting ready, coming back; kept free in the plan, never counted as a pause.
//! Rated by you from 0 to 10: what it costs, cognitively, emotionally, and in
//! anxiety; and what it gives back to your well-being. Written in the task's
//! or the event's own file (`X-SIOUL-BEFORE`, `X-SIOUL-AFTER`, `X-SIOUL-COST`,
//! `X-SIOUL-GAIN`), so that it travels with it and other programs keep it.

use serde::{Deserialize, Serialize};

pub const BEFORE: &str = "X-SIOUL-BEFORE";
pub const AFTER: &str = "X-SIOUL-AFTER";
pub const COST: &str = "X-SIOUL-COST";
pub const GAIN: &str = "X-SIOUL-GAIN";

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

/// The three costs and the gain, each 0 to 10, as you rate them; none unsaid.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Demands {
    #[serde(default)]
    pub cognitive: Option<u8>,
    #[serde(default)]
    pub emotional: Option<u8>,
    #[serde(default)]
    pub anxiety: Option<u8>,
    #[serde(default)]
    pub gain: Option<u8>,
}

impl Demands {
    /// Whether any cost is rated.
    pub fn has_cost(&self) -> bool {
        self.cognitive.is_some() || self.emotional.is_some() || self.anxiety.is_some()
    }

    /// The costs as written: "COGNITIVE=3;EMOTIONAL=5;ANXIETY=7", the rated ones only.
    pub fn cost_value(&self) -> String {
        [("COGNITIVE", self.cognitive), ("EMOTIONAL", self.emotional), ("ANXIETY", self.anxiety)]
            .iter()
            .filter_map(|(name, value)| value.map(|v| format!("{name}={v}")))
            .collect::<Vec<_>>()
            .join(";")
    }

    /// The costs read from what `cost_value` writes; names in any case, any order.
    pub fn read_cost(&mut self, value: &str) {
        for part in value.split(';') {
            let Some((name, number)) = part.split_once('=') else { continue };
            let number = number.trim().parse::<u8>().ok().map(|n| n.min(10));
            match name.trim().to_ascii_uppercase().as_str() {
                "COGNITIVE" => self.cognitive = number,
                "EMOTIONAL" => self.emotional = number,
                "ANXIETY" => self.anxiety = number,
                _ => {}
            }
        }
    }

    /// The gain read from its property's value.
    pub fn read_gain(&mut self, value: &str) {
        self.gain = value.trim().parse::<u8>().ok().map(|n| n.min(10));
    }

    /// One difficulty from 0 to 10, from the costs rated: their mean, the
    /// highest counted twice, so that one high cost (a call that frightens)
    /// is never drowned by two low ones. None when no cost is rated.
    pub fn difficulty(&self) -> Option<f32> {
        let rated: Vec<f32> = [self.cognitive, self.emotional, self.anxiety].into_iter().flatten().map(f32::from).collect();
        let highest = rated.iter().copied().fold(None, |m: Option<f32>, v| Some(m.map_or(v, |m| m.max(v))))?;
        Some((rated.iter().sum::<f32>() + highest) / (rated.len() as f32 + 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn costs_written_read_and_weighed() {
        let rated = Demands { cognitive: Some(2), emotional: Some(2), anxiety: Some(9), gain: Some(1) };
        assert_eq!(rated.cost_value(), "COGNITIVE=2;EMOTIONAL=2;ANXIETY=9");
        let mut read = Demands::default();
        read.read_cost("anxiety=9; cognitive=2 ;EMOTIONAL=2;OTHER=4");
        read.read_gain(" 1 ");
        assert_eq!(read, rated);
        // One high cost is not drowned: (2 + 2 + 9 + 9) / 4.
        assert_eq!(rated.difficulty(), Some(5.5));
        assert_eq!(Demands { cognitive: Some(5), emotional: Some(5), anxiety: Some(5), gain: None }.difficulty(), Some(5.0));
        assert_eq!(Demands { anxiety: Some(8), ..Demands::default() }.difficulty(), Some(8.0), "one rated: itself");
        assert_eq!(Demands { gain: Some(7), ..Demands::default() }.difficulty(), None, "a gain is no cost");
        let mut big = Demands::default();
        big.read_cost("COGNITIVE=40");
        assert_eq!(big.cognitive, Some(10), "never past 10");
        assert_eq!(minutes_of(" 20 "), 20);
        assert_eq!(minutes_of("3000"), 1440);
    }
}
