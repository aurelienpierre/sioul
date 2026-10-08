// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The weather at a place you chose, for a small applet in the status line:
//! now, the next four hours one by one, then mornings, afternoons, evenings
//! and nights; and for the phone's card on the home screen (`card`, `days`):
//! the hour under way, the next four, the next four parts of the day, then
//! the next seven days. From Open-Meteo (no key; data CC BY 4.0, credited
//! where shown: <https://open-meteo.com/en/licence>), in monochrome icons.

use crate::i18n::Translator;
use jiff::civil::Date;
use jiff::{Timestamp, Zoned};
use serde::{Deserialize, Serialize};

/// One hour of forecast.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hour {
    /// Its start, Unix seconds.
    pub at: i64,
    pub temperature: f32,
    /// The chance of rain, in percent; none when the model does not say.
    pub rain: Option<u8>,
    /// WMO weather code.
    pub code: u8,
    pub day: bool,
}

/// One day of forecast, at the place.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Day {
    /// The day, as the place counts its days.
    pub date: Date,
    /// WMO weather code, the day's most marked.
    pub code: u8,
    pub low: f32,
    pub high: f32,
    /// The highest chance of rain in the day, in percent; none when the model does not say.
    pub rain: Option<u8>,
}

/// The forecast kept: the hours from now on, the days from the place's
/// today on, and when it was fetched.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Forecast {
    pub fetched: i64,
    pub hours: Vec<Hour>,
    /// None in a forecast kept before the days were asked for.
    #[serde(default)]
    pub days: Vec<Day>,
}

#[derive(Deserialize)]
struct Answer {
    hourly: Hourly,
    #[serde(default)]
    daily: Option<Daily>,
    /// The place's offset from UTC (its days start at its own midnight).
    #[serde(default)]
    utc_offset_seconds: i64,
}

#[derive(Deserialize)]
struct Daily {
    /// Each day's midnight at the place, Unix seconds.
    time: Vec<i64>,
    #[serde(default)]
    weather_code: Vec<Option<u8>>,
    #[serde(default)]
    temperature_2m_max: Vec<Option<f32>>,
    #[serde(default)]
    temperature_2m_min: Vec<Option<f32>>,
    #[serde(default)]
    precipitation_probability_max: Vec<Option<u8>>,
}

#[derive(Deserialize)]
struct Hourly {
    time: Vec<i64>,
    temperature_2m: Vec<Option<f32>>,
    #[serde(default)]
    precipitation_probability: Vec<Option<u8>>,
    weather_code: Vec<Option<u8>>,
    #[serde(default)]
    is_day: Vec<Option<u8>>,
}

/// The query for a place, times as Unix seconds: hourly, three days ahead
/// (the phone's card is written until the end of tomorrow, and each of its
/// hours shows the parts of the day after it); daily, today and the eight
/// days after (tomorrow's card still shows seven days to come).
pub fn query(latitude: f64, longitude: f64) -> String {
    format!(
        "https://api.open-meteo.com/v1/forecast?latitude={latitude:.2}&longitude={longitude:.2}&hourly=temperature_2m,precipitation_probability,weather_code,is_day&forecast_hours=72&daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max&forecast_days=9&timeformat=unixtime&timezone=auto"
    )
}

/// Reads Open-Meteo's answer.
pub fn parse(json: &str, now: i64) -> Result<Forecast, String> {
    let answer: Answer = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let days = answer.daily.map(|d| daily(&d, answer.utc_offset_seconds)).unwrap_or_default();
    let h = answer.hourly;
    let hours = h
        .time
        .iter()
        .enumerate()
        .filter_map(|(i, at)| {
            Some(Hour {
                at: *at,
                temperature: (*h.temperature_2m.get(i)?)?,
                rain: h.precipitation_probability.get(i).copied().flatten(),
                code: (*h.weather_code.get(i)?)?,
                day: h.is_day.get(i).copied().flatten().is_none_or(|d| d == 1),
            })
        })
        .collect();
    Ok(Forecast { fetched: now, hours, days })
}

/// The days of the answer, each dated as the place counts: its midnight
/// read at the place's noon, so that a change of hour between now (the
/// offset given) and that day does not move it to the day before.
fn daily(d: &Daily, offset: i64) -> Vec<Day> {
    d.time
        .iter()
        .enumerate()
        .filter_map(|(i, at)| {
            let noon = Timestamp::from_second(at.checked_add(offset)?.checked_add(12 * 3600)?).ok()?;
            Some(Day {
                date: noon.to_zoned(jiff::tz::TimeZone::UTC).date(),
                code: (*d.weather_code.get(i)?)?,
                low: (*d.temperature_2m_min.get(i)?)?,
                high: (*d.temperature_2m_max.get(i)?)?,
                rain: d.precipitation_probability_max.get(i).copied().flatten(),
            })
        })
        .collect()
}

/// A WMO code as a word key and a Breeze icon, by day or night.
pub fn describe(code: u8, day: bool) -> (&'static str, &'static str) {
    let pick = |by_day: &'static str, by_night: &'static str| if day { by_day } else { by_night };
    match code {
        0 => ("clear", pick("weather-clear-symbolic", "weather-clear-night-symbolic")),
        1 => ("mainly-clear", pick("weather-few-clouds-symbolic", "weather-few-clouds-night-symbolic")),
        2 => ("partly-cloudy", pick("weather-clouds-symbolic", "weather-clouds-night-symbolic")),
        3 => ("overcast", "weather-overcast-symbolic"),
        45 | 48 => ("fog", "weather-fog-symbolic"),
        51 | 53 | 55 => ("drizzle", pick("weather-showers-scattered-day-symbolic", "weather-showers-scattered-night-symbolic")),
        56 | 57 => ("freezing-drizzle", "weather-freezing-scattered-rain-symbolic"),
        61 | 63 => ("rain", pick("weather-showers-day-symbolic", "weather-showers-night-symbolic")),
        65 => ("heavy-rain", "weather-showers-symbolic"),
        66 | 67 => ("freezing-rain", "weather-freezing-rain-symbolic"),
        71 | 73 | 75 | 77 => ("snow", pick("weather-snow-day-symbolic", "weather-snow-night-symbolic")),
        80..=82 => ("showers", "weather-showers-scattered-symbolic"),
        85 | 86 => ("snow-showers", "weather-snow-scattered-symbolic"),
        95 => ("thunderstorm", pick("weather-storm-day-symbolic", "weather-storm-night-symbolic")),
        96..=99 => ("hail", "weather-hail-symbolic"),
        _ => ("unknown", "weather-none-available-symbolic"),
    }
}

/// One line of the applet: an hour, or a part of a day.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Slot {
    /// "15:00", "This evening", "Tomorrow morning".
    pub label: String,
    pub icon: String,
    /// "14°", or "9° – 14°" over a part of a day.
    pub temperature: String,
    /// "40 %", or "" when rain is unlikely.
    pub rain: String,
    /// "Light rain", for screen readers and tips.
    pub words: String,
}

/// What the applet shows.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WeatherView {
    pub now: Option<Slot>,
    pub hours: Vec<Slot>,
    pub parts: Vec<Slot>,
}

/// Rain worth saying: a chance from 20 %.
fn rain_words(chance: Option<u8>, tr: &Translator) -> String {
    match chance {
        Some(c) if c >= 20 => {
            let mut args = crate::i18n::args();
            args.set("chance", c);
            tr.text("weather-rain-chance", Some(&args))
        }
        _ => String::new(),
    }
}

fn degrees(t: f32) -> String {
    format!("{}°", t.round() as i32)
}

/// An hour, as a line or a cell says it.
fn hour_slot(label: String, h: &Hour, tr: &Translator) -> Slot {
    let (word, icon) = describe(h.code, h.day);
    Slot { label, icon: icon.into(), temperature: degrees(h.temperature), rain: rain_words(h.rain, tr), words: tr.text(&format!("weather-{word}"), None) }
}

/// A part of a day, from its hours: the most marked weather of it, by day
/// if any hour of it is; its lowest and highest; its highest chance of rain.
fn part_slot(label: String, hours: &[&Hour], tr: &Translator) -> Slot {
    let worst = hours.iter().map(|h| h.code).max().unwrap_or(0);
    let (word, icon) = describe(worst, hours.iter().any(|h| h.day));
    let (low, high) = hours.iter().fold((f32::MAX, f32::MIN), |(lo, hi), h| (lo.min(h.temperature), hi.max(h.temperature)));
    let temperature = if degrees(low) == degrees(high) { degrees(high) } else { format!("{} – {}", degrees(low), degrees(high)) };
    Slot { label, icon: icon.into(), temperature, rain: rain_words(hours.iter().filter_map(|h| h.rain).max(), tr), words: tr.text(&format!("weather-{word}"), None) }
}

/// The applet's lines, from the forecast and the time now.
pub fn view(forecast: &Forecast, now: &Zoned, tr: &Translator) -> WeatherView {
    let zone = now.time_zone().clone();
    let this_hour = now.timestamp().as_second() - now.timestamp().as_second().rem_euclid(3600);
    let ahead: Vec<&Hour> = forecast.hours.iter().filter(|h| h.at >= this_hour).collect();
    let slot = |label: String, h: &Hour| hour_slot(label, h, tr);
    let local = |at: i64| Timestamp::from_second(at).map(|t| t.to_zoned(zone.clone())).ok();
    let now_slot = ahead.first().map(|h| slot(tr.text("weather-now", None), h));
    let hours: Vec<Slot> = ahead.iter().skip(1).take(4).filter_map(|h| Some(slot(local(h.at)?.strftime("%H:%M").to_string(), h))).collect();
    // Then the parts of days: morning 6–12, afternoon 12–18, evening 18–24, night 0–6.
    let mut parts: Vec<(String, Vec<&Hour>)> = Vec::new();
    for h in ahead.iter().skip(5) {
        let Some(at) = local(h.at) else { continue };
        // Today, tomorrow, the day after: further is not shown.
        let Ok(offset) = at.date().since(now.date()).map(|s| s.get_days()) else { continue };
        let key = match (at.hour(), offset) {
            (6..=11, 0) => "weather-this-morning",
            (12..=17, 0) => "weather-this-afternoon",
            (18..=23, 0) => "weather-this-evening",
            (0..=5, 0 | 1) => "weather-tonight",
            (6..=11, 1) => "weather-tomorrow-morning",
            (12..=17, 1) => "weather-tomorrow-afternoon",
            (18..=23, 1) => "weather-tomorrow-evening",
            (0..=5, 2) => "weather-tomorrow-night",
            _ => continue,
        };
        match parts.last_mut() {
            Some((last, hours)) if last == key => hours.push(h),
            _ => parts.push((key.to_string(), vec![h])),
        }
    }
    let parts = parts.into_iter().filter(|(_, hours)| hours.len() >= 2).map(|(key, hours)| part_slot(tr.text(&key, None), &hours, tr)).collect();
    WeatherView { now: now_slot, hours, parts }
}

/// The hours the card shows one by one after the hour under way, on the
/// same row as it (the owner's choice, 8 October 2026: four, not two).
pub const CARD_HOURS: usize = 4;
/// The parts of the day the card shows after them.
pub const CARD_PARTS: usize = 4;

/// What the phone's card shows of the weather at `at` (docs/android.md,
/// "The card on the home screen"): the hour under way ("Now"), the next four
/// hours, then the next four parts of the day that hold two hours of the
/// forecast at least, each the next of its kind, in order: morning (6–12),
/// afternoon (12–18), evening (18–24), night (0–6), named shortly
/// ("Afternoon", "Night"), since their order says which day they are. None
/// when the forecast does not hold the hour under way.
pub fn card(forecast: &Forecast, at: &Zoned, tr: &Translator) -> Option<WeatherView> {
    let zone = at.time_zone().clone();
    let stamp = at.timestamp().as_second();
    // The forecast's hours start at the place's hours: a zone half an hour off keeps them.
    let under_way = forecast.hours.iter().position(|h| h.at <= stamp && stamp < h.at + 3600)?;
    let first = &forecast.hours[under_way];
    let local = |at: i64| Timestamp::from_second(at).map(|t| t.to_zoned(zone.clone())).ok();
    let rest: Vec<&Hour> = forecast.hours[under_way + 1..].iter().collect();
    let hours = rest.iter().take(CARD_HOURS).filter_map(|h| Some(hour_slot(local(h.at)?.strftime("%H:%M").to_string(), h, tr))).collect();
    let mut parts: Vec<((Date, &'static str), Vec<&Hour>)> = Vec::new();
    for h in rest.iter().skip(CARD_HOURS) {
        let Some(when) = local(h.at) else { continue };
        let key = match when.hour() {
            0..=5 => "weather-part-night",
            6..=11 => "weather-part-morning",
            12..=17 => "weather-part-afternoon",
            _ => "weather-part-evening",
        };
        match parts.last_mut() {
            Some(((day, last), hours)) if *day == when.date() && *last == key => hours.push(h),
            _ => parts.push(((when.date(), key), vec![h])),
        }
    }
    let parts = parts.into_iter().filter(|(_, hours)| hours.len() >= 2).take(CARD_PARTS).map(|((_, key), hours)| part_slot(tr.text(key, None), &hours, tr)).collect();
    Some(WeatherView { now: Some(hour_slot(tr.text("weather-now", None), first, tr)), hours, parts })
}

/// A day of the forecast, as the card shows it among the days to come.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DaySlot {
    pub date: Date,
    /// "Fri", "ven.".
    pub label: String,
    /// The day's icon, by day.
    pub icon: String,
    /// "16°".
    pub high: String,
    /// "9°".
    pub low: String,
    /// "40 %", or "" when rain is unlikely.
    pub rain: String,
    /// "Light rain", for screen readers.
    pub words: String,
}

/// The forecast's days, as the card shows them: each with its short name,
/// its icon, its high and low, its chance of rain.
pub fn days(forecast: &Forecast, tr: &Translator) -> Vec<DaySlot> {
    forecast
        .days
        .iter()
        .map(|d| {
            let (word, icon) = describe(d.code, true);
            DaySlot { date: d.date, label: tr.weekday_short(d.date), icon: icon.into(), high: degrees(d.high), low: degrees(d.low), rain: rain_words(d.rain, tr), words: tr.text(&format!("weather-{word}"), None) }
        })
        .collect()
}

/// A place found by name.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Place {
    /// "Lyon, Auvergne-Rhône-Alpes, France".
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Deserialize)]
struct Found {
    #[serde(default)]
    results: Vec<FoundPlace>,
}

#[derive(Deserialize)]
struct FoundPlace {
    name: String,
    latitude: f64,
    longitude: f64,
    #[serde(default)]
    admin1: Option<String>,
    #[serde(default)]
    country: Option<String>,
}

/// The query to find a place by its name, in your language.
pub fn place_query(name: &str, language: &str) -> String {
    let encoded: String = name.trim().bytes().map(|b| if b.is_ascii_alphanumeric() || b == b'-' { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
    format!("https://geocoding-api.open-meteo.com/v1/search?name={encoded}&count=6&language={language}&format=json")
}

/// The places found, with their region and country so namesakes are told apart.
pub fn parse_places(json: &str) -> Result<Vec<Place>, String> {
    let found: Found = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(found
        .results
        .into_iter()
        .map(|p| Place { name: [Some(p.name), p.admin1, p.country].into_iter().flatten().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", "), latitude: p.latitude, longitude: p.longitude })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hours_then_parts_of_days() {
        // Saturday 3 October 2026, 13:20 in Paris; hourly from 13:00.
        let now: Zoned = "2026-10-03T13:20[Europe/Paris]".parse().unwrap();
        let start: Zoned = "2026-10-03T13:00[Europe/Paris]".parse().unwrap();
        let first = start.timestamp().as_second();
        let count = 36;
        let time: Vec<i64> = (0..count).map(|i| first + i * 3600).collect();
        let codes: Vec<u8> = (0..count).map(|i| if (5..10).contains(&i) { 61 } else { 2 }).collect();
        let json = serde_json::json!({
            "hourly": {
                "time": time,
                "temperature_2m": (0..count).map(|i| 10.0 + (i % 6) as f32).collect::<Vec<_>>(),
                "precipitation_probability": (0..count).map(|i| if (5..10).contains(&i) { 70 } else { 5 }).collect::<Vec<_>>(),
                "weather_code": codes,
                "is_day": (0..count).map(|i| u8::from(i < 6)).collect::<Vec<_>>()
            }
        })
        .to_string();
        let forecast = parse(&json, now.timestamp().as_second()).unwrap();
        let tr = Translator::new("en");
        let shown = view(&forecast, &now, &tr);
        assert_eq!(shown.now.as_ref().map(|s| s.icon.as_str()), Some("weather-clouds-symbolic"));
        let hours: Vec<&str> = shown.hours.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(hours, vec!["14:00", "15:00", "16:00", "17:00"]);
        let parts: Vec<(&str, &str)> = shown.parts.iter().map(|s| (s.label.as_str(), s.rain.as_str())).collect();
        assert_eq!(parts[0], ("This evening", "70 %"), "rain from 18:00 makes the evening rainy");
        assert_eq!(parts[1].0, "Tonight");
        let labels: Vec<&str> = parts.iter().map(|p| p.0).collect();
        assert_eq!(labels, vec!["This evening", "Tonight", "Tomorrow morning", "Tomorrow afternoon", "Tomorrow evening"], "a single hour of the next night is not a part");
    }

    /// Open-Meteo's answer for Geneva, asked on Thursday 8 October 2026 at
    /// 09:29 with `query`'s parts: 72 hours from 09:00, nine days from the 8th.
    const SAVED: &str = include_str!("../tests/fixtures/weather/open-meteo.json");

    #[test]
    fn the_card_reads_a_saved_answer() {
        let at = |text: &str| text.parse::<Zoned>().unwrap();
        let forecast = parse(SAVED, at("2026-10-08T09:29[Europe/Paris]").timestamp().as_second()).unwrap();
        assert_eq!(forecast.hours.len(), 72);
        // The days, as the place counts them, today first.
        assert_eq!(forecast.days.len(), 9);
        assert_eq!(forecast.days[0], Day { date: Date::constant(2026, 10, 8), code: 80, low: 13.3, high: 16.4, rain: Some(98) });
        assert_eq!(forecast.days[8].date, Date::constant(2026, 10, 16));
        let tr = Translator::new("en");
        // 09:20: the hour under way, the next four, then the next afternoon, evening, night and morning.
        let shown = card(&forecast, &at("2026-10-08T09:20[Europe/Paris]"), &tr).unwrap();
        let now = shown.now.as_ref().unwrap();
        assert_eq!((now.label.as_str(), now.icon.as_str(), now.temperature.as_str(), now.rain.as_str(), now.words.as_str()), ("Now", "weather-showers-day-symbolic", "16°", "98 %", "Rain"));
        let hours: Vec<(&str, &str)> = shown.hours.iter().map(|s| (s.label.as_str(), s.temperature.as_str())).collect();
        assert_eq!(hours, [("10:00", "16°"), ("11:00", "15°"), ("12:00", "15°"), ("13:00", "14°")]);
        let parts: Vec<(&str, &str, &str, &str)> = shown.parts.iter().map(|s| (s.label.as_str(), s.icon.as_str(), s.temperature.as_str(), s.rain.as_str())).collect();
        assert_eq!(
            parts,
            [
                ("Afternoon", "weather-showers-day-symbolic", "14° – 15°", "98 %"),
                ("Evening", "weather-overcast-symbolic", "13° – 14°", "65 %"),
                ("Night", "weather-overcast-symbolic", "13° – 14°", ""),
                ("Morning", "weather-clouds-symbolic", "13° – 14°", ""),
            ]
        );
        // 16:10: the afternoon's last hour and the evening's first among the four; the evening's rest next, then Friday's afternoon.
        let later = card(&forecast, &at("2026-10-08T16:10[Europe/Paris]"), &tr).unwrap();
        assert_eq!(later.hours.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(), ["17:00", "18:00", "19:00", "20:00"]);
        assert_eq!(later.parts.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(), ["Evening", "Night", "Morning", "Afternoon"]);
        // At 11:00, two hours of the afternoon are left after the four shown (16:00, 17:00): a part of its own.
        let edge = card(&forecast, &at("2026-10-08T11:00[Europe/Paris]"), &tr).unwrap();
        assert_eq!(edge.parts[0].label, "Afternoon");
        assert_eq!(edge.parts[0].temperature, "14° – 15°");
        // Before the forecast's first hour, or past its last: nothing said.
        assert!(card(&forecast, &at("2026-10-08T08:59[Europe/Paris]"), &tr).is_none());
        assert!(card(&forecast, &at("2026-10-11T09:00[Europe/Paris]"), &tr).is_none());
        // Near its end, fewer parts: never one of a single hour.
        let end = card(&forecast, &at("2026-10-11T02:00[Europe/Paris]"), &tr).unwrap();
        assert_eq!(end.hours.len(), 4);
        assert_eq!(end.parts.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(), ["Morning"]);
        // The days: their short names, icons by day, high, low, rain from 20 %.
        let days = days(&forecast, &tr);
        let said: Vec<(&str, &str, &str, &str, &str)> = days.iter().take(3).map(|d| (d.label.as_str(), d.icon.as_str(), d.high.as_str(), d.low.as_str(), d.rain.as_str())).collect();
        assert_eq!(said, [("Thu", "weather-showers-scattered-symbolic", "16°", "13°", "98 %"), ("Fri", "weather-showers-day-symbolic", "17°", "13°", ""), ("Sat", "weather-showers-day-symbolic", "16°", "12°", "40 %")]);
        // In French.
        let fr = Translator::new("fr");
        let shown = card(&forecast, &at("2026-10-08T09:20[Europe/Paris]"), &fr).unwrap();
        assert_eq!(shown.parts.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(), ["Après-midi", "Soir", "Nuit", "Matin"]);
        assert_eq!(super::days(&forecast, &fr)[1].label, "ven.");
        // A forecast kept before the days were asked for still reads, without them.
        let older: Forecast = serde_json::from_str(r#"{"fetched": 1, "hours": []}"#).unwrap();
        assert!(older.days.is_empty());
        // The query asks for both, enough of each.
        let asked = query(46.2, 6.15);
        assert!(asked.contains("&forecast_hours=72&") && asked.contains("&daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max&forecast_days=9&"));
    }

    #[test]
    fn a_day_keeps_its_date_across_a_change_of_hour() {
        // Asked in summer time (UTC+2): the day after the change to winter time
        // starts at 23:00 UTC, the one after it at 00:00 UTC +1.
        let d = Daily { time: vec![1_792_879_200, 1_792_969_200], weather_code: vec![Some(0), Some(3)], temperature_2m_max: vec![Some(10.0), Some(9.0)], temperature_2m_min: vec![Some(2.0), Some(1.0)], precipitation_probability_max: vec![None, Some(30)] };
        let days = daily(&d, 7200);
        assert_eq!(days.iter().map(|d| d.date).collect::<Vec<_>>(), [Date::constant(2026, 10, 25), Date::constant(2026, 10, 26)]);
    }

    #[test]
    fn namesakes_told_apart() {
        let json = r#"{"results":[{"name":"Paris","latitude":33.66,"longitude":-95.56,"country":"United States","admin1":"Texas"},{"name":"Paris","latitude":48.85,"longitude":2.35,"country":"France","admin1":"Île-de-France"}]}"#;
        let places = parse_places(json).unwrap();
        assert_eq!((places[0].name.as_str(), places[1].name.as_str()), ("Paris, Texas, United States", "Paris, Île-de-France, France"));
        assert!(place_query("Saint-Dié", "fr").contains("name=Saint-Di%C3%A9&"));
    }
}
