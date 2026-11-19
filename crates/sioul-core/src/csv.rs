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

/// What is read of a file at most, so that a hostile or huge file never
/// holds the window (review F7): rows, columns, and one cell's size. What
/// goes past is not read, and said.
pub const MOST_ROWS: usize = 20_000;
pub const MOST_COLUMNS: usize = 200;
pub const MOST_CELL: usize = 64 * 1024;

/// A table read: its headings, then its rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    pub separator: char,
    pub headings: Vec<String>,
    pub rows: Vec<Row>,
    /// Rows that do not read, by their first line: a cell over `MOST_CELL`.
    /// (A row that gives nothing usable is the import's to say, `contactimport::read_csv`.)
    pub broken: Vec<usize>,
    /// A quote opened and never closed: the line it opens on, and how many
    /// lines from there to the end are not read.
    pub unclosed: Option<(usize, usize)>,
    /// Rows with more cells than the headings, by their first line: read as
    /// they are (an unquoted separator inside a cell shifts the row), said.
    pub overflow: Vec<usize>,
    /// The file had more columns than `MOST_COLUMNS`: how many; only the first are offered.
    pub wide: Option<usize>,
    /// The file had more rows than `MOST_ROWS`: the first line not read.
    pub cut: Option<usize>,
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

/// The encoding a file was read in, as its summary says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    /// A spreadsheet's "Unicode text", told by its byte-order mark.
    Utf16,
    /// Anything else: the Western encoding a French spreadsheet saves in.
    Windows1252,
}

impl Encoding {
    /// Its word in the messages: "utf8", "utf16", "windows1252".
    pub fn word(self) -> &'static str {
        match self {
            Encoding::Utf8 => "utf8",
            Encoding::Utf16 => "utf16",
            Encoding::Windows1252 => "windows1252",
        }
    }
}

/// A file's text: its byte-order mark dropped; UTF-16 when its mark says so,
/// UTF-8 when it is, else Windows-1252, as a French spreadsheet saves it.
pub fn decode(bytes: &[u8]) -> String {
    decode_with(bytes).0
}

/// `decode`, with the encoding taken.
pub fn decode_with(bytes: &[u8]) -> (String, Encoding) {
    let utf16 = |little: bool, rest: &[u8]| {
        let units: Vec<u16> = rest.chunks_exact(2).map(|p| if little { u16::from_le_bytes([p[0], p[1]]) } else { u16::from_be_bytes([p[0], p[1]]) }).collect();
        (String::from_utf16_lossy(&units), Encoding::Utf16)
    };
    if let Some(rest) = bytes.strip_prefix(b"\xff\xfe") {
        return utf16(true, rest);
    }
    if let Some(rest) = bytes.strip_prefix(b"\xfe\xff") {
        return utf16(false, rest);
    }
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => (text.to_string(), Encoding::Utf8),
        Err(_) => (bytes.iter().map(|&b| if (0x80..0xa0).contains(&b) { CP1252[usize::from(b - 0x80)] } else { char::from(b) }).collect(), Encoding::Windows1252),
    }
}

/// The separator of a table: of semicolon, comma and tab, the one its first
/// line holds most, outside quotes; a tie goes to the semicolon (a French
/// spreadsheet's), then the comma. The import lets you choose another.
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

/// A table from its text, its separator found (`separator`).
pub fn read(text: &str) -> Table {
    read_with(text, separator(text))
}

/// A row being read: its cells (`MOST_COLUMNS` at most, the others counted),
/// whether a cell went past `MOST_CELL`.
#[derive(Default)]
struct Reading {
    cells: Vec<String>,
    cell: String,
    count: usize,
    big: bool,
}

impl Reading {
    fn add(&mut self, c: char) {
        if self.cell.len() < MOST_CELL {
            self.cell.push(c);
        } else {
            self.big = true;
        }
    }

    fn end_cell(&mut self) {
        self.count += 1;
        let cell = std::mem::take(&mut self.cell);
        if self.cells.len() < MOST_COLUMNS {
            self.cells.push(cell);
        }
    }
}

/// A table from its text with this separator: the first row its headings,
/// empty rows left out, at most `MOST_ROWS` rows of `MOST_COLUMNS` columns
/// of `MOST_CELL` bytes each.
pub fn read_with(text: &str, separator: char) -> Table {
    let mut table = Table { separator, ..Table::default() };
    // Each row read, with how many cells it had.
    let mut read: Vec<(Row, usize)> = Vec::new();
    let mut row = Reading::default();
    let (mut line, mut start) = (1usize, 1usize);
    let (mut quoted, mut was_quoted) = (false, false);
    let mut chars = text.chars().peekable();
    let end_row = |row: &mut Reading, start: usize, read: &mut Vec<(Row, usize)>, broken: &mut Vec<usize>| {
        row.end_cell();
        let done = std::mem::take(row);
        if done.big {
            broken.push(start);
        } else if done.cells.iter().any(|c| !c.trim().is_empty()) {
            read.push((Row { line: start, cells: done.cells }, done.count));
        }
    };
    while let Some(c) = chars.next() {
        if read.len() > MOST_ROWS {
            table.cut = Some(start);
            break;
        }
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    row.add('"');
                }
                '"' => quoted = false,
                '\r' if chars.peek() == Some(&'\n') => {}
                '\n' => {
                    line += 1;
                    row.add('\n');
                }
                _ => row.add(c),
            }
            continue;
        }
        match c {
            '"' if row.cell.trim().is_empty() && !was_quoted => {
                row.cell.clear();
                quoted = true;
                was_quoted = true;
            }
            c if c == separator => {
                row.end_cell();
                was_quoted = false;
            }
            '\r' if chars.peek() == Some(&'\n') => {}
            '\n' | '\r' => {
                end_row(&mut row, start, &mut read, &mut table.broken);
                was_quoted = false;
                line += 1;
                start = line;
            }
            _ => row.add(c),
        }
    }
    if table.cut.is_none() {
        if quoted {
            // A quote never closed: what follows it is no row.
            table.unclosed = Some((start, text.lines().count().saturating_sub(start) + 1));
        } else if !row.cell.is_empty() || !row.cells.is_empty() {
            end_row(&mut row, start, &mut read, &mut table.broken);
        }
    }
    read.truncate(MOST_ROWS + 1);
    if read.is_empty() {
        return table;
    }
    let (headings, width) = read.remove(0);
    if width > MOST_COLUMNS {
        table.wide = Some(width);
    }
    table.headings = headings.cells.into_iter().map(|h| h.trim().to_string()).collect();
    table.overflow = read.iter().filter(|(_, n)| *n > width).map(|(r, _)| r.line).collect();
    table.rows = read.into_iter().map(|(r, _)| r).collect();
    table
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
        // A quote never closed: its line named, with how many lines it took; the rows before kept.
        let table = read("Nom;Note\nA;ok\nB;\"never closed\nC;x\n");
        assert_eq!((table.rows.len(), table.unclosed), (1, Some((3, 2))));
        assert_eq!(decode(b"\x80 \x9c"), "€ œ");
    }

    /// Review F7, F22: the limits that keep a hostile file small, each said;
    /// UTF-16 read as such; a row of more cells than the headings said; the
    /// separator chosen by hand.
    #[test]
    fn limits_encodings_and_what_is_said() {
        let wide = format!("{}\n", vec!["h"; MOST_COLUMNS + 50].join(";"));
        let table = read(&wide);
        assert_eq!((table.headings.len(), table.wide), (MOST_COLUMNS, Some(MOST_COLUMNS + 50)));
        let long: String = "Nom\n".to_string() + &"x\n".repeat(MOST_ROWS + 10);
        let table = read(&long);
        assert_eq!((table.rows.len(), table.cut), (MOST_ROWS, Some(MOST_ROWS + 2)));
        let big = format!("Nom;Note\nA;{}\nB;ok\n", "x".repeat(MOST_CELL + 1));
        let table = read(&big);
        assert_eq!((table.rows.len(), table.broken.clone()), (1, vec![2]));
        let utf16: Vec<u8> = "\u{feff}Nom\tPrénom\r\nMartin\tJeanne\r\n".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        let (text, encoding) = decode_with(&utf16);
        let table = read(&text);
        assert_eq!((encoding, table.separator, table.headings.clone(), table.rows[0].cell(1)), (Encoding::Utf16, '\t', vec!["Nom".to_string(), "Prénom".to_string()], "Jeanne"));
        let shifted = read("Nom;Adresse\nMartin;3 rue de l'Exemple;Lyon\nExemple;Lyon\n");
        assert_eq!(shifted.overflow, [2]);
        let comma = "Nom;Adresse (rue, code postal, ville, pays)\nMartin;3 rue de l'Exemple\n";
        assert_eq!(separator(comma), ',', "found by the heading line alone");
        assert_eq!(read_with(comma, ';').headings.len(), 2, "chosen by hand");
    }
}
