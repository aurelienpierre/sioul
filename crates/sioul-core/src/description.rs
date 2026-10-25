// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The notes of a task or an event (their `DESCRIPTION`), written in Markdown
//! and shown as HTML that is safe to draw whoever wrote them: an invitation's
//! notes are its sender's words, and a list shared with other programs may
//! hold anything (docs/tasks.md).
//!
//! - **No HTML.** HTML written in the notes shows as it was written, as words.
//! - **No picture.** None is ever drawn, so showing the notes fetches nothing:
//!   a picture of the web is a link to it, under its words; any other shows its
//!   words alone.
//! - **Links for the browser.** Links to the web, to an address (`mailto:`)
//!   and to a number (`tel:`) are kept, and the window hands them on; it never
//!   fetches one itself. Any other link shows its words and, after them, its
//!   address as text. A web address written plainly is a link.
//! - **Task lists** (GFM: `- [ ] item`, `- [x] item`, and `- [-] item`
//!   for not needed this time, `checklist`) show a box, ☐, ☑ or ⊟, and no
//!   bullet (`task_tables`). Drawn to be ticked (`description_html_tickable`,
//!   the notes of your own task or event), each box of the checklist is a
//!   link, `sioul-check:<its line>`, which the window ticks.
//! - Headings are bold lines, not larger type: notes are a few lines in a
//!   panel. A line break is kept, as other calendar programs show it.
//!
//! The notes themselves stay as they were written: this is only how they are
//! drawn.

use crate::checklist::Mark;
use pulldown_cmark::{CowStr, Event, LinkType, Options, Parser, Tag, TagEnd, html};
use std::collections::BTreeSet;
use std::ops::Range;

/// The links kept: the window opens them in the browser, the mail page or the phone's dialer.
const KEPT: &[&str] = &["http", "https", "mailto", "tel"];

/// A box as HTML, held to its item's first word: a link that ticks the item
/// on `line` when given.
fn box_html(mark: Mark, line: Option<usize>) -> String {
    let glyph = match mark {
        Mark::Open => "\u{2610}",
        Mark::Done => "\u{2611}",
        Mark::Skipped => "\u{229f}",
    };
    match line {
        Some(line) => format!("<a href=\"sioul-check:{line}\">{glyph}</a>\u{a0}"),
        None => format!("{glyph}\u{a0}"),
    }
}

/// The parser's events with where each was read, its texts merged:
/// pulldown-cmark cuts a text at "&" or "[", and a web address with them.
fn read(markdown: &str) -> Vec<(Event<'_>, Range<usize>)> {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut out: Vec<(Event, Range<usize>)> = Vec::new();
    for (event, range) in Parser::new_ext(markdown, options).into_offset_iter() {
        if let (Event::Text(more), Some((Event::Text(text), at))) = (&event, out.last_mut()) {
            *text = CowStr::from(format!("{text}{more}"));
            at.end = range.end;
            continue;
        }
        out.push((event, range));
    }
    out
}

/// A picture being read: its address, whether it has words, whether a link
/// to it was opened (a picture of the web, outside any link).
struct Picture {
    address: String,
    words: bool,
    linked: bool,
}

/// Notes in Markdown as HTML safe to draw (the module's rules), their boxes
/// for reading only. Empty notes give an empty string.
pub fn description_html(markdown: &str) -> String {
    drawn(markdown, false)
}

/// The same, each box of the notes' checklist (`checklist::items`) a link,
/// `sioul-check:<its line>`, that the window ticks: the notes of your own
/// task or event. A box the checklist does not hold (in a quote) stays for
/// reading.
pub fn description_html_tickable(markdown: &str) -> String {
    drawn(markdown, true)
}

fn drawn(markdown: &str, tickable: bool) -> String {
    if markdown.trim().is_empty() {
        return String::new();
    }
    // Where each line starts, and the lines of the checklist's items.
    let starts: Vec<usize> = std::iter::once(0).chain(markdown.match_indices('\n').map(|(at, _)| at + 1)).collect();
    let line_of = |offset: usize| starts.partition_point(|&s| s <= offset).saturating_sub(1);
    let listed: BTreeSet<usize> = if tickable { crate::checklist::items(markdown).iter().map(|i| i.line).collect() } else { BTreeSet::new() };
    let tick = |offset: usize| Some(line_of(offset)).filter(|line| listed.contains(line));
    let mut events: Vec<Event> = Vec::new();
    // The events that are boxes.
    let mut boxes: BTreeSet<usize> = BTreeSet::new();
    // Each link open: none when kept, else its address, said after its words.
    let mut links: Vec<Option<String>> = Vec::new();
    let mut pictures: Vec<Picture> = Vec::new();
    // In a block of code, its words stay as written; the lines of a block of HTML so far.
    let (mut code, mut lines) = (false, 0usize);
    for (event, range) in read(markdown) {
        if let Event::Text(text) | Event::Code(text) = &event
            && !text.trim().is_empty()
        {
            pictures.iter_mut().for_each(|p| p.words = true);
        }
        let in_link = !links.is_empty() || pictures.iter().any(|p| p.linked);
        // An item's first words: "[-] …" is a box too (not needed this time), which GFM does not read.
        let first_words = match events.as_slice() {
            [.., Event::Start(Tag::Item)] | [.., Event::Start(Tag::Item), Event::Start(Tag::Paragraph)] => true,
            _ => false,
        };
        match event {
            Event::Text(text) if !code && !in_link && first_words && text.starts_with("[-]") && text[3..].starts_with(char::is_whitespace) => {
                boxes.insert(events.len());
                events.push(Event::Html(CowStr::from(box_html(Mark::Skipped, tick(range.start)))));
                bare_links(text[3..].trim_start(), &mut events);
            }
            Event::Text(text) if !code && !in_link => bare_links(&text, &mut events),
            Event::SoftBreak => events.push(Event::HardBreak),
            Event::Start(Tag::CodeBlock(kind)) => {
                code = true;
                events.push(Event::Start(Tag::CodeBlock(kind)));
            }
            Event::End(TagEnd::CodeBlock) => {
                code = false;
                events.push(Event::End(TagEnd::CodeBlock));
            }
            Event::Start(Tag::HtmlBlock) => {
                lines = 0;
                events.push(Event::Start(Tag::Paragraph));
            }
            Event::End(TagEnd::HtmlBlock) => events.push(Event::End(TagEnd::Paragraph)),
            Event::Html(raw) => {
                if lines > 0 {
                    events.push(Event::HardBreak);
                }
                lines += 1;
                events.push(Event::Text(CowStr::from(raw.trim_end_matches('\n').to_string())));
            }
            Event::InlineHtml(raw) => events.push(Event::Text(raw)),
            Event::Start(Tag::Heading { .. }) => {
                events.push(Event::Start(Tag::Paragraph));
                events.push(Event::Start(Tag::Strong));
            }
            Event::End(TagEnd::Heading(_)) => {
                events.push(Event::End(TagEnd::Strong));
                events.push(Event::End(TagEnd::Paragraph));
            }
            Event::Start(Tag::Link { link_type, dest_url, title, id }) => {
                if pictures.is_empty() && (link_type == LinkType::Email || kept(&dest_url, KEPT)) {
                    links.push(None);
                    events.push(Event::Start(Tag::Link { link_type, dest_url, title, id }));
                } else {
                    links.push(Some(dest_url.to_string()));
                }
            }
            Event::End(TagEnd::Link) => match links.pop() {
                Some(None) => events.push(Event::End(TagEnd::Link)),
                Some(Some(address)) if !address.trim().is_empty() => {
                    events.push(Event::Text(CowStr::from(format!(" ({})", address.trim()))));
                }
                _ => {}
            },
            Event::Start(Tag::Image { dest_url, title, .. }) => {
                let linked = !in_link && kept(&dest_url, &["http", "https"]);
                if linked {
                    let id = CowStr::from("");
                    events.push(Event::Start(Tag::Link { link_type: LinkType::Inline, dest_url: dest_url.clone(), title, id }));
                }
                pictures.push(Picture { address: dest_url.to_string(), words: false, linked });
            }
            Event::End(TagEnd::Image) => {
                if let Some(picture) = pictures.pop() {
                    // A picture written in the notes themselves says nothing as text.
                    if !picture.words && !picture.address.trim_start().to_ascii_lowercase().starts_with("data:") {
                        events.push(Event::Text(CowStr::from(picture.address.trim().to_string())));
                    }
                    if picture.linked {
                        events.push(Event::End(TagEnd::Link));
                    }
                }
            }
            Event::TaskListMarker(done) => {
                boxes.insert(events.len());
                events.push(Event::Html(CowStr::from(box_html(if done { Mark::Done } else { Mark::Open }, tick(range.start)))));
            }
            // Notes as written: neither footnotes nor mathematics are asked of the parser.
            Event::FootnoteReference(text) | Event::InlineMath(text) | Event::DisplayMath(text) => events.push(Event::Text(text)),
            other => events.push(other),
        }
    }
    task_tables(&mut events, &boxes);
    let mut out = String::new();
    html::push_html(&mut out, events.into_iter());
    out
}

/// Whether a link's address has one of these schemes, capitals aside.
fn kept(address: &str, schemes: &[&str]) -> bool {
    let address = address.trim();
    address.split_once(':').is_some_and(|(scheme, rest)| !rest.is_empty() && schemes.iter().any(|s| scheme.eq_ignore_ascii_case(s)))
}

/// A text with its web addresses ("https://…") made links, the way a message's
/// plain text has them (`reading::linkify`): an address ends at a space, a
/// bracket or a quote, without the stop that ends its sentence.
fn bare_links<'a>(text: &str, events: &mut Vec<Event<'a>>) {
    let mut rest = text;
    while let Some(at) = crate::reading::next_link(rest) {
        let length = rest[at..].find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | ')' | ']')).unwrap_or(rest.len() - at);
        let address = rest[at..at + length].trim_end_matches(['.', ',', ';', ':', '!', '?']);
        if address.len() <= "https://".len() {
            events.push(Event::Text(CowStr::from(rest[..at + address.len()].to_string())));
        } else {
            if at > 0 {
                events.push(Event::Text(CowStr::from(rest[..at].to_string())));
            }
            let dest = CowStr::from(address.to_string());
            events.push(Event::Start(Tag::Link { link_type: LinkType::Autolink, dest_url: dest.clone(), title: CowStr::from(""), id: CowStr::from("") }));
            events.push(Event::Text(dest));
            events.push(Event::End(TagEnd::Link));
        }
        rest = &rest[at + address.len()..];
    }
    if !rest.is_empty() {
        events.push(Event::Text(CowStr::from(rest.to_string())));
    }
}

/// A list open while the task lists are found: where it starts, its items
/// (where each starts, where its box is), where they end, and whether every
/// item so far starts with a box.
struct List {
    start: usize,
    items: Vec<(usize, usize)>,
    ends: Vec<usize>,
    boxed: bool,
}

/// A list whose every item starts with a box is drawn as GitHub draws a task
/// list, without bullets: a bullet before a box says the same thing twice,
/// and Qt's rich text draws a bullet whatever the style asks. Each item is a
/// row of a table without borders, its box in the first cell, its words in
/// the second: a long item wraps under its words, not under its box. A
/// numbered list keeps its numbers, and a list with any item that has no box
/// its bullets.
fn task_tables(events: &mut [Event], boxes: &BTreeSet<usize>) {
    let mut open: Vec<List> = Vec::new();
    for i in 0..events.len() {
        match &events[i] {
            Event::Start(Tag::List(first)) => open.push(List { start: i, items: Vec::new(), ends: Vec::new(), boxed: first.is_none() }),
            Event::Start(Tag::Item) => {
                // In a loose list, the box comes after the paragraph's start.
                let at = if matches!(events.get(i + 1), Some(Event::Start(Tag::Paragraph))) { i + 2 } else { i + 1 };
                let boxed = boxes.contains(&at);
                if let Some(list) = open.last_mut() {
                    list.boxed &= boxed;
                    list.items.push((i, at));
                }
            }
            Event::End(TagEnd::Item) => {
                if let Some(list) = open.last_mut() {
                    list.ends.push(i);
                }
            }
            Event::End(TagEnd::List(_)) => {
                let Some(list) = open.pop() else { continue };
                if !list.boxed || list.items.is_empty() {
                    continue;
                }
                events[list.start] = Event::Html(CowStr::from("<table cellspacing=\"0\" cellpadding=\"0\">\n"));
                for &(item, at) in &list.items {
                    let shown = if let Event::Html(t) = &events[at] { t.to_string() } else { String::new() };
                    events[item] = Event::Html(CowStr::from(format!("<tr><td valign=\"top\">{shown}</td><td>")));
                    events[at] = Event::Text(CowStr::from(""));
                }
                for &end in &list.ends {
                    events[end] = Event::Html(CowStr::from("</td></tr>\n"));
                }
                events[i] = Event::Html(CowStr::from("</table>\n"));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_is_drawn() {
        let html = description_html("**Bring** the *green* folder, ~~not~~ the `red` one.\n\n# Before\n\n1. Call\n2. Go");
        assert!(html.contains("<strong>Bring</strong>"), "{html}");
        assert!(html.contains("<em>green</em>") && html.contains("<del>not</del>") && html.contains("<code>red</code>"), "{html}");
        // A heading is a bold line, not larger type.
        assert!(html.contains("<p><strong>Before</strong></p>") && !html.contains("<h1"), "{html}");
        assert!(html.contains("<ol>") && html.contains("<li>Call</li>"), "{html}");
        assert_eq!(description_html("  \n "), "");
    }

    #[test]
    fn line_breaks_are_kept() {
        // As written in a calendar program that knows no Markdown.
        let html = description_html("Room 4\nSecond floor");
        assert!(html.contains("Room 4<br />\nSecond floor"), "{html}");
    }

    #[test]
    fn html_shows_as_words() {
        for raw in [
            "<script>alert(1)</script>",
            "<b onmouseover=\"x()\">bold</b>",
            "<img src=\"https://tracker.example.org/p.gif\">",
            "<iframe src=\"https://example.org\"></iframe>",
            "<style>body { background: url(https://example.org/x) }</style>",
            "Hello <img src=x onerror=alert(1)> there",
            "<a href=\"javascript:alert(1)\">x</a>",
            "<div>\n<p>block</p>\n</div>",
            "<link rel=\"stylesheet\" href=\"https://example.org/a.css\">",
        ] {
            let html = description_html(raw);
            for tag in ["<script", "<b ", "<img", "<iframe", "<style", "<a ", "<div", "<link", "<p>block"] {
                assert!(!html.contains(tag), "{raw:?} gave {html}");
            }
            assert!(html.contains("&lt;"), "{raw:?} gave {html}");
        }
        // Its words are kept, as written.
        assert!(description_html("<b>bold</b>").contains("&lt;b&gt;bold&lt;/b&gt;"));
    }

    #[test]
    fn no_picture_is_drawn() {
        for (raw, shown) in [
            ("![The clinic](https://example.org/clinic.png)", "The clinic"),
            ("![](https://example.org/pixel.gif)", "https://example.org/pixel.gif"),
            ("![map](/home/someone/map.png)", "map"),
            ("![](file:///etc/x.png)", "file:///etc/x.png"),
            ("![plan][p]\n\n[p]: https://example.org/plan.png", "plan"),
            ("[![badge](https://example.org/b.svg)](https://example.org)", "badge"),
            ("![a ![b](https://example.org/b.png)](https://example.org/a.png)", "a"),
        ] {
            let html = description_html(raw);
            assert!(!html.contains("<img") && !html.contains("src="), "{raw:?} gave {html}");
            assert!(html.contains(shown), "{raw:?} gave {html}");
        }
        // A picture of the web is a link to it; one of this computer only its words.
        assert!(description_html("![The clinic](https://example.org/clinic.png)").contains("<a href=\"https://example.org/clinic.png\">The clinic</a>"));
        assert!(!description_html("![map](/home/someone/map.png)").contains("<a"));
        // One written in the notes themselves shows nothing of its bytes.
        let data = description_html("![](data:image/png;base64,iVBORw0KGgo=)");
        assert!(!data.contains("<img") && !data.contains("base64"), "{data}");
    }

    #[test]
    fn links_are_kept_for_the_browser_alone() {
        let html = description_html("[The clinic](https://example.org/clinic), [mail](mailto:desk@example.org), [call](tel:+33199001234), <desk@example.org>");
        assert!(html.contains("<a href=\"https://example.org/clinic\">The clinic</a>"), "{html}");
        assert!(html.contains("<a href=\"mailto:desk@example.org\">mail</a>") && html.contains("<a href=\"tel:+33199001234\">call</a>"), "{html}");
        assert!(html.contains("<a href=\"mailto:desk@example.org\">desk@example.org</a>"), "{html}");
        for (raw, shown) in [
            ("[x](javascript:alert(1))", "x (javascript:alert(1))"),
            ("[run](file:///usr/bin/thing)", "run (file:///usr/bin/thing)"),
            ("[page](data:text/html,hello)", "page (data:text/html,hello)"),
            ("[other note](other.md)", "other note (other.md)"),
            ("[share](smb://host/share)", "share (smb://host/share)"),
        ] {
            let html = description_html(raw);
            assert!(!html.contains("<a"), "{raw:?} gave {html}");
            assert!(html.contains(shown), "{raw:?} gave {html}");
        }
        // An address cannot leave its quotes.
        let html = description_html("[x](https://example.org/\"onclick=\"y)");
        assert!(!html.contains("\"onclick"), "{html}");
    }

    #[test]
    fn web_addresses_written_plainly_are_links() {
        let html = description_html("Join: https://meet.example.org/abc-def?pwd=1&x=2. Or call.");
        assert!(html.contains("Join: <a href=\"https://meet.example.org/abc-def?pwd=1&amp;x=2\">https://meet.example.org/abc-def?pwd=1&amp;x=2</a>. Or call."), "{html}");
        // In a link, in code: as written.
        let html = description_html("[site](https://example.org) `https://example.org/code` and\n\n    https://example.org/block");
        assert_eq!(html.matches("<a ").count(), 1, "{html}");
        // "https://" alone is no link.
        assert!(!description_html("https:// then").contains("<a"));
    }

    #[test]
    fn task_lists_show_boxes_without_bullets() {
        let html = description_html("Bring:\n\n- [ ] Carte Vitale\n- [x] Last blood test\n  The paper one.\n\nCheck:\n\n* [ ] The address\n* plain line");
        assert!(!html.contains("<input"), "{html}");
        // The first list is all boxes: a table, each box in its own cell, no bullet.
        assert!(html.contains("<table cellspacing=\"0\" cellpadding=\"0\">\n<tr><td valign=\"top\">\u{2610}\u{a0}</td><td>Carte Vitale</td></tr>"), "{html}");
        assert!(html.contains("<tr><td valign=\"top\">\u{2611}\u{a0}</td><td>Last blood test<br />\nThe paper one.</td></tr>\n</table>"), "{html}");
        // The second has a plain line: its bullets stay, the box after one.
        assert!(html.contains("<ul>\n<li>\u{2610}\u{a0}The address</li>\n<li>plain line</li>\n</ul>"), "{html}");
        assert_eq!(html.matches("<table").count(), 1, "{html}");
        // A loose list, and one in an item: each a table.
        let loose = description_html("- [ ] One\n\n- [x] Two\n  - [ ] Inner");
        assert_eq!(loose.matches("<table").count(), 2, "{loose}");
        assert_eq!(loose.matches("</table>").count(), 2, "{loose}");
        assert!(!loose.contains("<li") && !loose.contains("</li>"), "{loose}");
        assert!(loose.contains("<tr><td valign=\"top\">\u{2610}\u{a0}</td><td>\n<p>One</p>"), "{loose}");
        // Numbered, its numbers stay.
        let numbered = description_html("1. [ ] First\n2. [x] Second");
        assert!(numbered.contains("<ol>") && !numbered.contains("<table"), "{numbered}");
        // Written out of a list, it is only words.
        assert!(description_html("[ ] not an item").contains("[ ] not an item"));
    }

    #[test]
    fn boxes_tick_their_item_when_asked() {
        let notes = "Bring:\n\n- [ ] Carte Vitale\n- [x] Last blood test\n- [-] Vaccination record\n\n> - [ ] Quoted\n\n    - [ ] code";
        let read = description_html(notes);
        assert!(!read.contains("sioul-check"), "for reading: {read}");
        assert!(read.contains("\u{229f}\u{a0}</td><td>Vaccination record"), "not needed: its own box: {read}");
        let html = description_html_tickable(notes);
        // Each by its line in the notes.
        for (line, glyph) in [(2, '\u{2610}'), (3, '\u{2611}'), (4, '\u{229f}')] {
            assert!(html.contains(&format!("<a href=\"sioul-check:{line}\">{glyph}</a>")), "line {line}: {html}");
        }
        // A box the checklist does not hold stays for reading; a line of code is no box.
        assert_eq!(html.matches("sioul-check:").count(), 3, "{html}");
        assert!(html.contains("<td valign=\"top\">\u{2610}\u{a0}</td><td>Quoted"), "{html}");
        // In a loose list, and after a group's name and words: the same lines.
        let loose = "Words first.\n\nCheck:\n\n- [ ] Keys\n\n- [ ] Phone";
        let html = description_html_tickable(loose);
        assert!(html.contains("sioul-check:4\"") && html.contains("sioul-check:6\""), "{html}");
        // "[-]" anywhere but an item's start is words.
        assert!(description_html_tickable("A [-] B").contains("A [-] B"));
    }
}
