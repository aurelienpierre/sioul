// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The weather at a place you chose, for a small applet in the status line:
//! now, the next four hours one by one, then mornings, afternoons, evenings
//! and nights. From Open-Meteo (no key; data CC BY 4.0, credited where
//! shown: https://open-meteo.com/en/licence), in monochrome icons.

use crate::i18n::Translator;
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

/// The forecast kept: the hours from now on, and when it was fetched.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Forecast {
    pub fetched: i64,
    pub hours: Vec<Hour>,
}

#[derive(Deserialize)]
struct Answer {
    hourly: Hourly,
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

/// The query for a place: hourly, a day and a half ahead, times as Unix seconds.
pub fn query(latitude: f64, longitude: f64) -> String {
    format!(
        "https://api.open-meteo.com/v1/forecast?latitude={latitude:.2}&longitude={longitude:.2}&hourly=temperature_2m,precipitation_probability,weather_code,is_day&forecast_hours=40&timeformat=unixtime&timezone=auto"
    )
}

/// Reads Open-Meteo's answer.
pub fn parse(json: &str, now: i64) -> Result<Forecast, String> {
    let answer: Answer = serde_json::from_str(json).map_err(|e| e.to_string())?;
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
    Ok(Forecast { fetched: now, hours })
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

/// The applet's lines, from the forecast and the time now.
pub fn view(forecast: &Forecast, now: &Zoned, tr: &Translator) -> WeatherView {
    let zone = now.time_zone().clone();
    let this_hour = now.timestamp().as_second() - now.timestamp().as_second().rem_euclid(3600);
    let ahead: Vec<&Hour> = forecast.hours.iter().filter(|h| h.at >= this_hour).collect();
    let slot = |label: String, h: &Hour| {
        let (word, icon) = describe(h.code, h.day);
        Slot { label, icon: icon.into(), temperature: degrees(h.temperature), rain: rain_words(h.rain, tr), words: tr.text(&format!("weather-{word}"), None) }
    };
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
    let parts = parts
        .into_iter()
        .filter(|(_, hours)| hours.len() >= 2)
        .map(|(key, hours)| {
            // The most marked weather of the part, by day if any hour of it is.
            let worst = hours.iter().map(|h| h.code).max().unwrap_or(0);
            let (word, icon) = describe(worst, hours.iter().any(|h| h.day));
            let (low, high) = hours.iter().fold((f32::MAX, f32::MIN), |(lo, hi), h| (lo.min(h.temperature), hi.max(h.temperature)));
            let temperature = if degrees(low) == degrees(high) { degrees(high) } else { format!("{} – {}", degrees(low), degrees(high)) };
            Slot { label: tr.text(&key, None), icon: icon.into(), temperature, rain: rain_words(hours.iter().filter_map(|h| h.rain).max(), tr), words: tr.text(&format!("weather-{word}"), None) }
        })
        .collect();
    WeatherView { now: now_slot, hours, parts }
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

    #[test]
    fn namesakes_told_apart() {
        let json = r#"{"results":[{"name":"Paris","latitude":33.66,"longitude":-95.56,"country":"United States","admin1":"Texas"},{"name":"Paris","latitude":48.85,"longitude":2.35,"country":"France","admin1":"Île-de-France"}]}"#;
        let places = parse_places(json).unwrap();
        assert_eq!((places[0].name.as_str(), places[1].name.as_str()), ("Paris, Texas, United States", "Paris, Île-de-France, France"));
        assert!(place_query("Saint-Dié", "fr").contains("name=Saint-Di%C3%A9&"));
    }
}
