// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sioul's core.
//!
//! Everything the interface shows is decided here, so that the command line,
//! the Qt interface and AI agents all see the same Porch. The interface holds
//! no logic of its own (docs/architecture.md).

pub mod accounts;
pub mod agenda;
pub mod areas;
pub mod bank;
pub mod budget;
pub mod capabilities;
pub mod capture;
pub mod card;
pub mod cases;
pub mod codes;
pub mod compose;
pub mod config;
pub mod contracts;
pub mod dayview;
pub mod contacts;
pub mod folders;
pub mod github;
pub mod headers;
pub mod health;
pub mod i18n;
pub mod invoice;
pub mod lines;
pub mod letters;
pub mod links;
pub mod lookalike;
pub mod mailindex;
pub mod maildir;
pub mod money;
pub mod notes;
pub mod payments;
pub mod pgp;
pub mod plan;
pub mod places;
pub mod porch;
pub mod presets;
pub mod papers;
pub mod quiet;
pub mod routines;
pub mod reminders;
pub mod project;
pub mod reading;
pub mod rules;
pub mod settings;
pub mod shield;
pub mod sounds;
pub mod sites;
pub mod state;
pub mod taskview;
pub mod tasks;
pub mod text;
pub mod threads;
pub mod timelog;
pub mod timereport;
pub mod today;
pub mod trust;
pub mod vdir;
pub mod view;
pub mod weather;
pub mod wearable;
pub mod window;
