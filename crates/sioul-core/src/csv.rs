// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! CSV files as spreadsheets and address books write them (RFC 4180, and
//! what a French spreadsheet does instead), for the contacts' import: the
//! byte-order mark dropped; UTF-8, else Windows-1252 (the Western encoding a
//! French spreadsheet saves in); the separator found among semicolon, comma
//! and tab, by the heading line; cells in quotes, a quote inside doubled, a
//! line break inside kept. Each row says the line it starts on, so that what
//! does not read is named by its line.
//!
//! The bank's import keeps its own line-based reader (`bank::read_csv`).

/// A table read: its headings, then its rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    pub separator: char,
    pub headings: Vec<String>,
    pub rows: Vec<Row>,
    /// Rows that do not read, by their first line: a quote never closed.
    pub broken: Vec<usize>,
}

/// One row: the line it starts on (from 1), its cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub line: usize,
    pub cells: Vec<String>,
}

impl Row {
    /// Its cell under heading `column`, trimmed; "" past its end.
    pub fn cell(&self, column: usize) -> &str {
        self.cells.get(column).map(|c| c.trim()).unwrap_or("")
    }
}

/// Windows-1252's characters for the bytes 0x80 to 0x9F (the others are
/// Latin-1's); five bytes there mean nothing, read as U+FFFD.
const CP1252: [char; 32] = [
    '€', '\u{fffd}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{fffd}', 'Ž', '\u{fffd}', '\u{fffd}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{fffd}', 'ž', 'Ÿ',
];

/// A file's text: its byte-order mark dropped; UTF-8 when it is, else
/// Windows-1252, as a French spreadsheet saves it.
pub fn decode(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => bytes.iter().map(|&b| if (0x80..0xa0).contains(&b) { CP1252[usize::from(b - 0x80)] } else { char::from(b) }).collect(),
    }
}

/// The separator of a table: of semicolon, comma and tab, the one its first
/// line holds most, outside quotes; a tie goes to the semicolon (a French
/// spreadsheet's), then the comma.
pub fn separator(text: &str) -> char {
    let mut first = String::new();
    let mut quoted = false;
    for c in text.chars() {
        match c {
            '"' => quoted = !quoted,
            '\n' if !quoted => break,
            _ => {}
        }
        if !quoted {
            first.push(c);
        }
    }
    let count = |s: char| first.chars().filter(|&c| c == s).count();
    [';', ',', '\t'].into_iter().max_by_key(|&s| (count(s), match s {
        ';' => 2,
        ',' => 1,
        _ => 0,
    })).unwrap_or(';')
}

/// A table from its text: the first row its headings, empty rows left out.
pub fn read(text: &str) -> Table {
    let separator = separator(text);
    let mut rows: Vec<Row> = Vec::new();
    let mut broken = Vec::new();
    let (mut cells, mut cell) = (Vec::new(), String::new());
    let (mut line, mut start) = (1usize, 1usize);
    let (mut quoted, mut was_quoted) = (false, false);
    let mut chars = text.chars().peekable();
    let end_row = |cells: &mut Vec<String>, cell: &mut String, start: usize, rows: &mut Vec<Row>| {
        cells.push(std::mem::take(cell));
        let row = std::mem::take(cells);
        if row.iter().any(|c| !c.trim().is_empty()) {
            rows.push(Row { line: start, cells: row });
        }
    };
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    cell.push('"');
                }
                '"' => quoted = false,
                '\r' if chars.peek() == Some(&'\n') => {}
                '\n' => {
                    line += 1;
                    cell.push('\n');
                }
                _ => cell.push(c),
            }
            continue;
        }
        match c {
            '"' if cell.trim().is_empty() && !was_quoted => {
                cell.clear();
                quoted = true;
                was_quoted = true;
            }
            c if c == separator => {
                cells.push(std::mem::take(&mut cell));
                was_quoted = false;
            }
            '\r' if chars.peek() == Some(&'\n') => {}
            '\n' | '\r' => {
                end_row(&mut cells, &mut cell, start, &mut rows);
                was_quoted = false;
                line += 1;
                start = line;
            }
            _ => cell.push(c),
        }
    }
    if quoted {
        // A quote never closed: what follows it is no row.
        broken.push(start);
        cells.clear();
        cell.clear();
    } else if !cell.is_empty() || !cells.is_empty() {
        end_row(&mut cells, &mut cell, start, &mut rows);
    }
    let headings = if rows.is_empty() { Vec::new() } else { rows.remove(0).cells.into_iter().map(|h| h.trim().to_string()).collect() };
    Table { separator, headings, rows, broken }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// As a French spreadsheet saves it: semicolons, Windows-1252, a quoted
    /// note over two lines, a quote doubled inside; then as Google writes it.
    #[test]
    fn tables_as_spreadsheets_write_them() {
        let bytes: &[u8] = b"Civilit\xe9;Nom;Pr\xe9nom;Commentaire\r\nMme;Exemple;L\xe9a;\"Vient le mardi\r\npour \"\"le th\xe9\"\"\"\r\n\r\nM.;Martin;Jean;\r\n";
        let table = read(&decode(bytes));
        assert_eq!(table.separator, ';');
        assert_eq!(table.headings, ["Civilité", "Nom", "Prénom", "Commentaire"]);
        assert_eq!(table.rows.len(), 2);
        assert_eq!((table.rows[0].line, table.rows[0].cell(2), table.rows[0].cell(3)), (2, "Léa", "Vient le mardi\npour \"le thé\""));
        assert_eq!((table.rows[1].line, table.rows[1].cell(1), table.rows[1].cell(9)), (5, "Martin", ""));
        // UTF-8 with its mark, commas, Google's headings.
        let google = "\u{feff}First Name,Last Name,E-mail 1 - Value,Phone 1 - Value\nJane,Example,jane@example.org,+1 555 0100\n";
        let table = read(&decode(google.as_bytes()));
        assert_eq!((table.separator, table.headings[0].as_str(), table.rows[0].cell(3)), (',', "First Name", "+1 555 0100"));
        assert_eq!(separator("a\tb\tc\n"), '\t');
        assert_eq!(separator("\"a;b\",c\n"), ',', "never inside quotes");
        // A quote never closed: its row named, the rows before kept.
        let table = read("Nom;Note\nA;ok\nB;\"never closed\nC;x\n");
        assert_eq!((table.rows.len(), table.broken.clone()), (1, vec![3]));
        assert_eq!(decode(b"\x80 \x9c"), "€ œ");
    }
}
