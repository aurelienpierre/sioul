// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Multimedia messages sent from a computer through the phone (docs/texts.md,
//! "Multimedia messages sent from a computer"): the message Android's `sendMultimediaMessage`
//! sends, an M-Send.req of the MMS 1.2 encapsulation (OMA-TS-MMS_ENC-V1_3,
//! over WSP's encoding, WAP-230-WSP), written on the phone from a request's
//! words and media (`encode`); the computer measures a message with the same
//! code (`size`), so both count the same bytes against the carrier's limit;
//! and the carrier's answer to it (M-Send.conf, `conf`), which Android hands
//! back with its result.
//!
//! The bytes are those AOSP's own composer writes for such a message
//! (`PduComposer`, android16-release), which Android's own parser reads back
//! (`android/jvm-checks/MmsCheck.java`, on the samples of
//! `android/jvm-checks/mms-samples.jsonl`, which this module's tests write
//! and check): its headers in its order; From as the insert-address token
//! (the carrier writes the sender's number: the phone need not know its
//! own); each number with `/TYPE=PLMN`; no subject; class Personal; no
//! delivery or read report (they would need another restricted permission,
//! and Android's carrier default offers none); the body `multipart/related`,
//! its SMIL first (AOSP Messaging's layouts), then each medium, then the
//! words in UTF-8. Each part is named plainly (`image1.jpg`, `text1.txt`),
//! never by its file's own name, which can say where or who: a medium here
//! has no name at all.

use std::fmt::Write as _;

/// A medium of a message: its content type, as the message names it
/// (`image/jpeg`, `video/mp4`, `text/x-vCard`), and its bytes.
#[derive(Debug, Clone, Copy)]
pub struct Medium<'a> {
    pub ct: &'a str,
    pub bytes: &'a [u8],
}

/// A multimedia message to send (M-Send.req).
#[derive(Debug, Clone, Copy)]
pub struct Message<'a> {
    /// The request's key: the message's transaction id, which Android files
    /// in the provider's `tr_id`, so that the phone finds it there again.
    pub key: &'a str,
    /// Its numbers, as written: one, or several for a group.
    pub to: &'a [&'a str],
    /// When it was written (seconds since 1970: the Date header).
    pub date: i64,
    /// Its words, if any: one text part after the media.
    pub words: &'a str,
    /// Its media, in their order.
    pub media: &'a [Medium<'a>],
}

/// Why a message cannot be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// No number, or one that is no phone number (letters, too short).
    Number,
    /// Neither words nor media.
    Empty,
    /// A key that cannot be a transaction id (empty, too long, not ASCII).
    Key,
    /// A medium's content type that cannot be written (empty, too long, not ASCII).
    Type,
}

/// The message's bytes, as Android sends them (the PDU).
pub fn encode(message: &Message) -> Result<Vec<u8>, Refused> {
    let mut out = Vec::with_capacity(message.media.iter().map(|m| m.bytes.len()).sum::<usize>() + message.words.len() + 1024);
    write(message, &mut out)?;
    Ok(out)
}

/// How many bytes the message is (`encode`'s, counted without being
/// written): what the carrier's limit is held against, on the computer that
/// reduces its pictures and on the phone that sends it.
pub fn size(message: &Message) -> Result<usize, Refused> {
    let mut count = Count(0);
    write(message, &mut count)?;
    Ok(count.0)
}

/// How many bytes the message is once its media hold `lengths` bytes each,
/// given with their content types alone (`media`' bytes left empty): what a
/// computer's page counts as words are typed, never reading the media again.
/// The same number `size` gives the message whole: a part's headers do not
/// depend on its bytes, only its data's length does (a uintvar).
pub fn size_with_lengths(message: &Message, lengths: &[usize]) -> Result<usize, Refused> {
    let empty = size(message)?;
    let grown = lengths.iter().map(|&length| {
        let mut count = Count(0);
        uintvar(&mut count, length);
        length + count.0 - 1
    });
    Ok(empty + grown.sum::<usize>())
}

/// The bytes left under `limit` (the carrier's, else the one chosen) for the
/// media's own bytes, all of them together, for a message whose media are
/// given empty (their content types alone): its size so, taken from the
/// limit, less what their lengths may add once they hold their bytes (two
/// bytes each, for a medium of up to 2 MB). None when even that is over.
/// A message whose media hold no more between them then fits (`size`).
pub fn room(message: &Message, limit: usize) -> Option<usize> {
    let empty = size(message).ok()?;
    limit.checked_sub(empty)?.checked_sub(2 * message.media.len())
}

/// A number as a message's To header writes it: its digits, with the `+`
/// that may begin it ("05 36 49 12 34" is `0536491234`, "+33 1 99 00 12 34"
/// is `+33199001234`); none for a name in letters or fewer than three
/// digits (a sender named in letters cannot be answered so).
pub fn address(number: &str) -> Option<String> {
    let number = number.trim();
    let mut out = String::new();
    for (i, c) in number.chars().enumerate() {
        match c {
            '0'..='9' => out.push(c),
            '+' if i == 0 => out.push(c),
            ' ' | '\u{a0}' | '\u{202f}' | '.' | '-' | '(' | ')' | '/' => {}
            _ => return None,
        }
    }
    (out.chars().filter(char::is_ascii_digit).count() >= 3).then_some(out)
}

/// The longest number a message goes to (E.164's 15 digits after its `+`),
/// for a measure that holds whatever number the phone then writes: a
/// computer measures a message with it, never shorter than the phone's own.
pub const LONGEST_NUMBER: &str = "+000000000000000";

/// The carrier's answer to a message sent (M-Send.conf, ENC 6.1.2), as
/// Android hands it back with its result (`SmsManager.EXTRA_MMS_DATA`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Conf {
    /// X-Mms-Response-Status: `OK` (0x80), else why not (ENC 7.3.48:
    /// 0xC0 to 0xDF for a moment, 0xE0 to 0xFF for good, 0x81 to 0x88 as
    /// MMS 1.0 said them).
    pub status: u8,
    /// Its Message-ID, given by the carrier when it took it.
    pub id: String,
    /// Its X-Mms-Transaction-ID: the request's key, as sent.
    pub key: String,
    /// Its X-Mms-Response-Text, when the carrier said why in words.
    pub said: String,
}

/// The carrier took the message (X-Mms-Response-Status Ok).
pub const OK: u8 = 0x80;

impl Conf {
    /// Whether the carrier took it.
    pub fn ok(&self) -> bool {
        self.status == OK
    }
}

/// The carrier's answer, read; none for bytes that are no M-Send.conf. A
/// header Sioul does not need is passed over by its WSP form.
pub fn conf(pdu: &[u8]) -> Option<Conf> {
    let mut at = Reader { bytes: pdu, at: 0 };
    if at.byte()? != MESSAGE_TYPE || at.byte()? != SEND_CONF {
        return None;
    }
    let mut conf = Conf::default();
    let mut status = None;
    while let Some(field) = at.byte() {
        match field {
            TRANSACTION_ID => conf.key = at.text()?,
            MESSAGE_ID => conf.id = at.text()?,
            RESPONSE_STATUS => status = Some(at.byte()?),
            RESPONSE_TEXT => conf.said = at.encoded()?,
            field if field >= 0x80 => at.skip()?,
            _ => return None,
        }
    }
    conf.status = status?;
    Some(conf)
}

// ------------------------------------------------------------------ writing

/// Header fields and values (ENC 7.3; AOSP's `PduHeaders`, `PduPart`).
const CONTENT_TYPE: u8 = 0x84;
const DATE: u8 = 0x85;
const DELIVERY_REPORT: u8 = 0x86;
const FROM: u8 = 0x89;
const MESSAGE_CLASS: u8 = 0x8A;
const MESSAGE_ID: u8 = 0x8B;
const MESSAGE_TYPE: u8 = 0x8C;
const MMS_VERSION: u8 = 0x8D;
const READ_REPORT: u8 = 0x90;
const RESPONSE_STATUS: u8 = 0x92;
const RESPONSE_TEXT: u8 = 0x93;
const TO: u8 = 0x97;
const TRANSACTION_ID: u8 = 0x98;
const SEND_REQ: u8 = 0x80;
const SEND_CONF: u8 = 0x81;
/// MMS 1.2 (`PduHeaders.CURRENT_MMS_VERSION`), as a short integer.
const VERSION_1_2: u8 = 0x80 | (1 << 4) | 2;
const INSERT_ADDRESS: u8 = 0x81;
const PERSONAL: u8 = 0x80;
const NO: u8 = 0x81;
/// A part's Content-Type parameters and headers (WSP 8.4.2).
const P_CHARSET: u8 = 0x81;
const P_NAME: u8 = 0x85;
const P_TYPE: u8 = 0x89;
const P_START: u8 = 0x8A;
const P_CONTENT_LOCATION: u8 = 0x8E;
const P_CONTENT_ID: u8 = 0xC0;
/// UTF-8's MIBenum.
const UTF_8: u8 = 106;
/// How long a picture, a card or the words show, for a phone that follows the SMIL (AOSP Messaging's).
const SHOWN_MS: u32 = 5000;

/// The content types WSP numbers (AOSP's `PduContentTypes`, by number): a
/// part of one of these types names it by its number, any other by its
/// words, as Android's composer does (an exact match: `text/x-vCard`).
const NUMBERED: [&str; 0x53] = [
    "*/*",
    "text/*",
    "text/html",
    "text/plain",
    "text/x-hdml",
    "text/x-ttml",
    "text/x-vCalendar",
    "text/x-vCard",
    "text/vnd.wap.wml",
    "text/vnd.wap.wmlscript",
    "text/vnd.wap.wta-event",
    "multipart/*",
    "multipart/mixed",
    "multipart/form-data",
    "multipart/byterantes",
    "multipart/alternative",
    "application/*",
    "application/java-vm",
    "application/x-www-form-urlencoded",
    "application/x-hdmlc",
    "application/vnd.wap.wmlc",
    "application/vnd.wap.wmlscriptc",
    "application/vnd.wap.wta-eventc",
    "application/vnd.wap.uaprof",
    "application/vnd.wap.wtls-ca-certificate",
    "application/vnd.wap.wtls-user-certificate",
    "application/x-x509-ca-cert",
    "application/x-x509-user-cert",
    "image/*",
    "image/gif",
    "image/jpeg",
    "image/tiff",
    "image/png",
    "image/vnd.wap.wbmp",
    "application/vnd.wap.multipart.*",
    "application/vnd.wap.multipart.mixed",
    "application/vnd.wap.multipart.form-data",
    "application/vnd.wap.multipart.byteranges",
    "application/vnd.wap.multipart.alternative",
    "application/xml",
    "text/xml",
    "application/vnd.wap.wbxml",
    "application/x-x968-cross-cert",
    "application/x-x968-ca-cert",
    "application/x-x968-user-cert",
    "text/vnd.wap.si",
    "application/vnd.wap.sic",
    "text/vnd.wap.sl",
    "application/vnd.wap.slc",
    "text/vnd.wap.co",
    "application/vnd.wap.coc",
    "application/vnd.wap.multipart.related",
    "application/vnd.wap.sia",
    "text/vnd.wap.connectivity-xml",
    "application/vnd.wap.connectivity-wbxml",
    "application/pkcs7-mime",
    "application/vnd.wap.hashed-certificate",
    "application/vnd.wap.signed-certificate",
    "application/vnd.wap.cert-response",
    "application/xhtml+xml",
    "application/wml+xml",
    "text/css",
    "application/vnd.wap.mms-message",
    "application/vnd.wap.rollover-certificate",
    "application/vnd.wap.locc+wbxml",
    "application/vnd.wap.loc+xml",
    "application/vnd.syncml.dm+wbxml",
    "application/vnd.syncml.dm+xml",
    "application/vnd.syncml.notification",
    "application/vnd.wap.xhtml+xml",
    "application/vnd.wv.csp.cir",
    "application/vnd.oma.dd+xml",
    "application/vnd.oma.drm.message",
    "application/vnd.oma.drm.content",
    "application/vnd.oma.drm.rights+xml",
    "application/vnd.oma.drm.rights+wbxml",
    "application/vnd.wv.csp+xml",
    "application/vnd.wv.csp+wbxml",
    "application/vnd.syncml.ds.notification",
    "audio/*",
    "video/*",
    "application/vnd.oma.dd2+xml",
    "application/mikey",
];

/// Where a message's bytes go: written (`Vec`), or counted (`Count`).
trait Out {
    fn put(&mut self, bytes: &[u8]);
    fn byte(&mut self, byte: u8) {
        self.put(&[byte]);
    }
}

impl Out for Vec<u8> {
    fn put(&mut self, bytes: &[u8]) {
        self.extend_from_slice(bytes);
    }
}

/// Bytes counted, not kept.
struct Count(usize);

impl Out for Count {
    fn put(&mut self, bytes: &[u8]) {
        self.0 += bytes.len();
    }
}

/// An unsigned integer of 7 bits a byte, the most significant first (WSP 8.1.2).
fn uintvar(out: &mut impl Out, value: usize) {
    let mut bytes = [0u8; 10];
    let mut at = bytes.len();
    let mut rest = value;
    loop {
        at -= 1;
        bytes[at] = (rest & 0x7F) as u8 | if at == bytes.len() - 1 { 0 } else { 0x80 };
        rest >>= 7;
        if rest == 0 {
            break;
        }
    }
    out.put(&bytes[at..]);
}

/// A length: one byte up to 30, else 31 and a uintvar (WSP 8.4.2.2).
fn value_length(out: &mut impl Out, length: usize) {
    if length <= 30 {
        out.byte(length as u8);
    } else {
        out.byte(31);
        uintvar(out, length);
    }
}

/// A Text-string: the bytes and a 0, behind a quote when the first is 128 or more (WSP 8.4.2.1).
fn text(out: &mut impl Out, bytes: &[u8]) {
    if bytes.first().is_some_and(|b| *b >= 0x80) {
        out.byte(0x7F);
    }
    out.put(bytes);
    out.byte(0);
}

/// A Quoted-string: a `"`, the bytes and a 0.
fn quoted(out: &mut impl Out, bytes: &[u8]) {
    out.byte(b'"');
    out.put(bytes);
    out.byte(0);
}

/// A Long-integer: its byte count, then its bytes, the most significant first, as few as hold it.
fn long_integer(out: &mut impl Out, value: u64) {
    let bytes = value.to_be_bytes();
    let first = bytes.iter().position(|b| *b != 0).unwrap_or(bytes.len());
    out.byte((bytes.len() - first) as u8);
    out.put(&bytes[first..]);
}

/// A content type, by its WSP number when it has one, else by its words.
fn content_type(out: &mut impl Out, ct: &str) {
    match NUMBERED.iter().position(|known| *known == ct) {
        Some(number) => out.byte(0x80 | number as u8),
        None => text(out, ct.as_bytes()),
    }
}

/// What a medium is, for its name and its place in the SMIL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Image,
    Video,
    Audio,
    Card,
    Other,
}

fn kind_of(ct: &str) -> Kind {
    let ct = ct.to_ascii_lowercase();
    if ct.starts_with("image/") {
        Kind::Image
    } else if ct.starts_with("video/") {
        Kind::Video
    } else if ct.starts_with("audio/") {
        Kind::Audio
    } else if matches!(ct.as_str(), "text/x-vcard" | "text/vcard" | "text/x-vcalendar" | "text/calendar") {
        Kind::Card
    } else {
        Kind::Other
    }
}

/// A medium's plain name in the message, by its kind and its rank among
/// those of its kind (`image1.jpg`, `video1.mp4`, `contact1.vcf`): never
/// its file's own.
fn names(media: &[Medium]) -> Vec<(Kind, String)> {
    let mut seen = [0usize; 5];
    media
        .iter()
        .map(|medium| {
            let kind = kind_of(medium.ct);
            let rank = &mut seen[kind as usize];
            *rank += 1;
            let ct = medium.ct.to_ascii_lowercase();
            let (stem, extension) = match kind {
                Kind::Image => ("image", match ct.as_str() {
                    "image/png" => "png",
                    "image/gif" => "gif",
                    "image/webp" => "webp",
                    "image/bmp" => "bmp",
                    _ => "jpg",
                }),
                Kind::Video => ("video", if ct.contains("3gpp") { "3gp" } else if ct.contains("webm") { "webm" } else { "mp4" }),
                Kind::Audio => ("audio", match ct.as_str() {
                    "audio/amr" => "amr",
                    "audio/mpeg" | "audio/mp3" => "mp3",
                    "audio/ogg" | "audio/opus" => "ogg",
                    "audio/aac" => "aac",
                    "audio/wav" | "audio/x-wav" => "wav",
                    _ => "m4a",
                }),
                Kind::Card if ct.contains("cal") => ("calendar", "vcs"),
                Kind::Card => ("contact", "vcf"),
                Kind::Other => ("other", "dat"),
            };
            (kind, format!("{stem}{rank}.{extension}"))
        })
        .collect()
}

/// The words' part's name.
const WORDS_NAME: &str = "text1.txt";

/// The message's SMIL, as AOSP Messaging lays one out: the pictures and
/// videos above (four fifths of the height when there are words), the words
/// below, one `<par>` each, the words last. A video or a sound plays its
/// own length (no duration said: Sioul does not read it, and a wrong one
/// would cut it short on a phone that follows the SMIL).
fn smil(parts: &[(Kind, String)], words: bool) -> String {
    let mut body = String::new();
    for (kind, name) in parts {
        let _ = match kind {
            Kind::Image => write!(body, "<par dur=\"{SHOWN_MS}ms\"><img src=\"{name}\" region=\"Image\" /></par>"),
            Kind::Video => write!(body, "<par><video src=\"{name}\" region=\"Image\" /></par>"),
            Kind::Audio => write!(body, "<par><audio src=\"{name}\" /></par>"),
            Kind::Card | Kind::Other => write!(body, "<par dur=\"{SHOWN_MS}ms\"><ref src=\"{name}\" /></par>"),
        };
    }
    if words {
        let _ = write!(body, "<par dur=\"{SHOWN_MS}ms\"><text src=\"{WORDS_NAME}\" region=\"Text\" /></par>");
    }
    let visual = parts.iter().any(|(kind, _)| matches!(kind, Kind::Image | Kind::Video));
    let other = parts.iter().any(|(kind, _)| matches!(kind, Kind::Audio | Kind::Card));
    let layout = match (visual, other, words) {
        (true, _, true) => r#"<region id="Image" fit="meet" top="0" left="0" height="80%" width="100%"/><region id="Text" top="80%" left="0" height="20%" width="100%"/>"#,
        (true, _, false) => r#"<region id="Image" fit="meet" top="0" left="0" height="100%" width="100%"/>"#,
        (false, true, false) => "",
        _ => r#"<region id="Text" top="0" left="0" height="100%" width="100%"/>"#,
    };
    format!("<smil><head><layout><root-layout/>{layout}</layout></head><body>{body}</body></smil>")
}

/// One part of the body: its Content-Type (with its name, and its charset
/// for words), its Content-ID and Content-Location, then its bytes (WSP 8.5,
/// as `PduComposer.makeMessageBody` writes them).
fn part(out: &mut impl Out, ct: &str, name: &str, charset: Option<u8>, data: &[u8]) {
    let mut typed = Vec::new();
    content_type(&mut typed, ct);
    typed.push(P_NAME);
    text(&mut typed, name.as_bytes());
    if let Some(charset) = charset {
        typed.push(P_CHARSET);
        typed.push(0x80 | charset);
    }
    let mut headers = Vec::new();
    value_length(&mut headers, typed.len());
    headers.extend_from_slice(&typed);
    let id = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    headers.push(P_CONTENT_ID);
    quoted(&mut headers, format!("<{id}>").as_bytes());
    headers.push(P_CONTENT_LOCATION);
    text(&mut headers, name.as_bytes());
    uintvar(out, headers.len());
    uintvar(out, data.len());
    out.put(&headers);
    out.put(data);
}

/// What `encode` and `size` write, in AOSP's order (`PduComposer.makeSendRetrievePdu`).
fn write(message: &Message, out: &mut impl Out) -> Result<(), Refused> {
    let key = message.key.as_bytes();
    if key.is_empty() || key.len() > 64 || !key.iter().all(|b| b.is_ascii_graphic()) {
        return Err(Refused::Key);
    }
    let numbers: Vec<String> = message.to.iter().map(|n| address(n)).collect::<Option<_>>().ok_or(Refused::Number)?;
    if numbers.is_empty() {
        return Err(Refused::Number);
    }
    if message.words.is_empty() && message.media.is_empty() {
        return Err(Refused::Empty);
    }
    if message.media.iter().any(|m| m.ct.is_empty() || m.ct.len() > 100 || !m.ct.bytes().all(|b| b.is_ascii_graphic())) {
        return Err(Refused::Type);
    }
    out.put(&[MESSAGE_TYPE, SEND_REQ, TRANSACTION_ID]);
    text(out, key);
    out.put(&[MMS_VERSION, VERSION_1_2, DATE]);
    long_integer(out, u64::try_from(message.date).unwrap_or(0).max(1));
    out.put(&[FROM, 1, INSERT_ADDRESS]);
    for number in &numbers {
        // Encoded-string-value with its charset, as Android's composer writes every address.
        let mut value = vec![0x80 | UTF_8];
        text(&mut value, format!("{number}/TYPE=PLMN").as_bytes());
        out.byte(TO);
        value_length(out, value.len());
        out.put(&value);
    }
    out.put(&[MESSAGE_CLASS, PERSONAL, DELIVERY_REPORT, NO, READ_REPORT, NO, CONTENT_TYPE]);
    // multipart/related, its SMIL first: start and type.
    let mut related = Vec::new();
    content_type(&mut related, "application/vnd.wap.multipart.related");
    related.push(P_START);
    text(&mut related, b"<smil>");
    related.push(P_TYPE);
    text(&mut related, b"application/smil");
    value_length(out, related.len());
    out.put(&related);
    let named = names(message.media);
    let words = !message.words.is_empty();
    uintvar(out, 1 + message.media.len() + usize::from(words));
    // The SMIL part, as AOSP Messaging names it: "smil" and smil.xml.
    let layout = smil(&named, words);
    part(out, "application/smil", "smil.xml", None, layout.as_bytes());
    for (medium, (_, name)) in message.media.iter().zip(&named) {
        part(out, medium.ct, name, None, medium.bytes);
    }
    if words {
        part(out, "text/plain", WORDS_NAME, Some(UTF_8), message.words.as_bytes());
    }
    Ok(())
}

// ------------------------------------------------------------------ reading

/// The bytes of an answer, read in turn.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> Option<u8> {
        let byte = *self.bytes.get(self.at)?;
        self.at += 1;
        Some(byte)
    }

    fn take(&mut self, count: usize) -> Option<&[u8]> {
        let end = self.at.checked_add(count)?;
        let taken = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(taken)
    }

    fn uintvar(&mut self) -> Option<usize> {
        let mut value: usize = 0;
        for _ in 0..5 {
            let byte = self.byte()?;
            value = (value << 7) | usize::from(byte & 0x7F);
            if byte & 0x80 == 0 {
                return Some(value);
            }
        }
        None
    }

    /// A Text-string, its quote passed over.
    fn text(&mut self) -> Option<String> {
        let rest = self.bytes.get(self.at..)?;
        let end = rest.iter().position(|b| *b == 0)?;
        let mut bytes = &rest[..end];
        self.at += end + 1;
        if bytes.first() == Some(&0x7F) || bytes.first() == Some(&b'"') {
            bytes = &bytes[1..];
        }
        Some(String::from_utf8_lossy(bytes).into_owned())
    }

    /// An Encoded-string-value: a Text-string, or a length, a charset and a Text-string.
    fn encoded(&mut self) -> Option<String> {
        let first = *self.bytes.get(self.at)?;
        if first > 31 {
            return self.text();
        }
        let length = self.length()?;
        let value = self.take(length)?;
        let mut inner = Reader { bytes: value, at: 0 };
        // The charset: a short integer, or a long one.
        match inner.byte()? {
            byte if byte >= 0x80 => {}
            byte if byte <= 30 => {
                inner.take(usize::from(byte))?;
            }
            _ => return None,
        }
        inner.text()
    }

    /// A Value-length.
    fn length(&mut self) -> Option<usize> {
        match self.byte()? {
            31 => self.uintvar(),
            byte if byte <= 30 => Some(usize::from(byte)),
            _ => None,
        }
    }

    /// A header's value of a field this reader does not need, passed over by its form (WSP 8.4.1.2).
    fn skip(&mut self) -> Option<()> {
        match *self.bytes.get(self.at)? {
            0..=31 => {
                let length = self.length()?;
                self.take(length)?;
            }
            32..=127 => {
                self.text()?;
            }
            _ => {
                self.at += 1;
            }
        }
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const KEY: &str = "0f3c9a5e7b2d4f6a8c1e3b5d7f9a2c4e";
    /// 18 October 2026, 14:03 in Paris, in seconds.
    const WRITTEN: i64 = 1_792_324_980;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn sha256(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        hex(&Sha256::digest(bytes))
    }

    /// Bytes standing for a picture or a sound: `count` of them, from `seed`.
    fn stand_in(count: usize, seed: u8) -> Vec<u8> {
        (0..count).map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed)).collect()
    }

    /// A message read back as Android reads one: its headers and each part.
    #[derive(Debug, Default)]
    struct Read {
        headers: Vec<(u8, Vec<u8>)>,
        start: String,
        parts: Vec<ReadPart>,
    }

    #[derive(Debug, Default, PartialEq)]
    struct ReadPart {
        ct: String,
        name: String,
        charset: u8,
        id: String,
        location: String,
        data: Vec<u8>,
    }

    /// The test's own decoder of an M-Send.req, as WSP reads one: every
    /// header (its value's bytes), then the body's parts.
    fn read(pdu: &[u8]) -> Read {
        let mut r = Reader { bytes: pdu, at: 0 };
        let mut read = Read::default();
        loop {
            let field = r.byte().expect("a header");
            if field == CONTENT_TYPE {
                break;
            }
            let from = r.at;
            match field {
                MESSAGE_TYPE | MMS_VERSION | MESSAGE_CLASS | DELIVERY_REPORT | READ_REPORT => {
                    r.byte().unwrap();
                }
                TRANSACTION_ID => {
                    r.text().unwrap();
                }
                DATE => {
                    let count = usize::from(r.byte().unwrap());
                    r.take(count).unwrap();
                }
                FROM | TO => {
                    let length = r.length().unwrap();
                    r.take(length).unwrap();
                }
                _ => panic!("a header Sioul does not write: {field:#x}"),
            }
            read.headers.push((field, pdu[from..r.at].to_vec()));
        }
        let length = r.length().unwrap();
        let related = r.take(length).unwrap().to_vec();
        assert_eq!(related[0], 0x80 | 0x33, "multipart/related");
        let mut inner = Reader { bytes: &related, at: 1 };
        assert_eq!(inner.byte(), Some(P_START));
        read.start = inner.text().unwrap();
        assert_eq!(inner.byte(), Some(P_TYPE));
        assert_eq!(inner.text().unwrap(), "application/smil");
        assert_eq!(inner.at, related.len());
        let count = r.uintvar().unwrap();
        for _ in 0..count {
            let headers_length = r.uintvar().unwrap();
            let data_length = r.uintvar().unwrap();
            let headers = r.take(headers_length).unwrap().to_vec();
            let data = r.take(data_length).unwrap().to_vec();
            let mut h = Reader { bytes: &headers, at: 0 };
            let typed_length = h.length().unwrap();
            let typed = h.take(typed_length).unwrap().to_vec();
            let mut t = Reader { bytes: &typed, at: 0 };
            let ct = if typed[0] >= 0x80 {
                NUMBERED[usize::from(t.byte().unwrap() & 0x7F)].to_string()
            } else {
                t.text().unwrap()
            };
            let mut part = ReadPart { ct, data, ..ReadPart::default() };
            while let Some(parameter) = t.byte() {
                match parameter {
                    P_NAME => part.name = t.text().unwrap(),
                    P_CHARSET => part.charset = t.byte().unwrap() & 0x7F,
                    _ => panic!("a parameter Sioul does not write: {parameter:#x}"),
                }
            }
            while let Some(header) = h.byte() {
                match header {
                    P_CONTENT_ID => part.id = h.text().unwrap(),
                    P_CONTENT_LOCATION => part.location = h.text().unwrap(),
                    _ => panic!("a part header Sioul does not write: {header:#x}"),
                }
            }
            read.parts.push(part);
        }
        assert_eq!(r.at, pdu.len(), "nothing after the last part");
        read
    }

    #[test]
    fn integers_and_lengths_at_their_edges() {
        let written = |f: &dyn Fn(&mut Vec<u8>)| {
            let mut out = Vec::new();
            f(&mut out);
            out
        };
        assert_eq!(written(&|o| uintvar(o, 0)), [0x00]);
        assert_eq!(written(&|o| uintvar(o, 127)), [0x7F]);
        assert_eq!(written(&|o| uintvar(o, 128)), [0x81, 0x00]);
        assert_eq!(written(&|o| uintvar(o, 16_383)), [0xFF, 0x7F]);
        assert_eq!(written(&|o| uintvar(o, 16_384)), [0x81, 0x80, 0x00]);
        assert_eq!(written(&|o| uintvar(o, 614_400)), [0xA5, 0xC0, 0x00]);
        assert_eq!(written(&|o| value_length(o, 30)), [30]);
        assert_eq!(written(&|o| value_length(o, 31)), [31, 31]);
        assert_eq!(written(&|o| value_length(o, 128)), [31, 0x81, 0x00]);
        assert_eq!(written(&|o| long_integer(o, WRITTEN as u64)), [4, 0x6A, 0xD4, 0xB5, 0x74]);
        assert_eq!(written(&|o| text(o, "é".as_bytes())), [0x7F, 0xC3, 0xA9, 0x00], "a text beginning past 127 is quoted");
        for value in [0, 1, 127, 128, 300, 16_383, 16_384, 2_097_151, 2_097_152, 614_400] {
            let mut out = Vec::new();
            uintvar(&mut out, value);
            assert_eq!(Reader { bytes: &out, at: 0 }.uintvar(), Some(value));
        }
        for length in [0, 30, 31, 127, 128, 20_000] {
            let mut out = Vec::new();
            value_length(&mut out, length);
            assert_eq!(Reader { bytes: &out, at: 0 }.length(), Some(length));
        }
    }

    /// The headers as Android's composer writes them, byte for byte, then a
    /// picture with words: the SMIL, the picture, the words, each named
    /// plainly.
    #[test]
    fn a_picture_with_words_is_written_as_android_writes_it() {
        let picture = stand_in(200, 7);
        let media = [Medium { ct: "image/jpeg", bytes: &picture }];
        let message = Message { key: KEY, to: &["+33 1 99 00 12 34"], date: WRITTEN, words: "Voilà l’entrée.", media: &media };
        let pdu = encode(&message).unwrap();
        let mut head = vec![0x8C, 0x80, 0x98];
        head.extend_from_slice(KEY.as_bytes());
        head.extend_from_slice(&[0x00, 0x8D, 0x92, 0x85, 0x04, 0x6A, 0xD4, 0xB5, 0x74, 0x89, 0x01, 0x81, 0x97, 24, 0xEA]);
        head.extend_from_slice(b"+33199001234/TYPE=PLMN\0");
        head.extend_from_slice(&[0x8A, 0x80, 0x86, 0x81, 0x90, 0x81, 0x84, 27, 0xB3, 0x8A]);
        head.extend_from_slice(b"<smil>\0\x89application/smil\0\x03");
        assert_eq!(hex(&pdu[..head.len()]), hex(&head));
        let read = read(&pdu);
        assert_eq!(read.headers.len(), 9, "type, key, version, date, from, to, class, two reports");
        assert_eq!(read.start, "<smil>");
        let names: Vec<(&str, &str, &str, &str, u8)> = read.parts.iter().map(|p| (p.ct.as_str(), p.name.as_str(), p.id.as_str(), p.location.as_str(), p.charset)).collect();
        assert_eq!(names, [("application/smil", "smil.xml", "<smil>", "smil.xml", 0), ("image/jpeg", "image1.jpg", "<image1>", "image1.jpg", 0), ("text/plain", "text1.txt", "<text1>", "text1.txt", 106)]);
        assert_eq!(read.parts[1].data, picture);
        assert_eq!(read.parts[2].data, "Voilà l’entrée.".as_bytes());
        let layout = String::from_utf8(read.parts[0].data.clone()).unwrap();
        assert!(layout.contains(r#"<region id="Image" fit="meet" top="0" left="0" height="80%" width="100%"/><region id="Text""#), "{layout}");
        assert!(layout.contains(r#"<body><par dur="5000ms"><img src="image1.jpg" region="Image" /></par><par dur="5000ms"><text src="text1.txt" region="Text" /></par></body>"#), "{layout}");
        assert_eq!(size(&message), Ok(pdu.len()));
    }

    /// Words alone to two numbers: two To headers, one message to both (a
    /// group's, for later); a number as written, its spaces and dots left out.
    #[test]
    fn words_to_two_are_one_message_to_both() {
        let message = Message { key: KEY, to: &["+33 2 61 91 00 42", "03.53.01.42.42"], date: WRITTEN, words: "Rendez-vous à 18 h.", media: &[] };
        let read = read(&encode(&message).unwrap());
        let to: Vec<String> = read.headers.iter().filter(|(field, _)| *field == TO).map(|(_, value)| String::from_utf8_lossy(&value[2..value.len() - 1]).into_owned()).collect();
        assert_eq!(to, ["+33261910042/TYPE=PLMN", "0353014242/TYPE=PLMN"]);
        assert_eq!(read.parts.len(), 2);
        let layout = String::from_utf8(read.parts[0].data.clone()).unwrap();
        assert_eq!(layout, r#"<smil><head><layout><root-layout/><region id="Text" top="0" left="0" height="100%" width="100%"/></layout></head><body><par dur="5000ms"><text src="text1.txt" region="Text" /></par></body></smil>"#);
    }

    /// Three media and words: each named by its kind and rank, never its
    /// file's; a video plays its own length; a card is a reference.
    #[test]
    fn three_media_are_named_by_their_kind() {
        let (gif, video, card) = (stand_in(64, 1), stand_in(300, 2), b"BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Alice\r\nEND:VCARD\r\n".to_vec());
        let media = [Medium { ct: "image/gif", bytes: &gif }, Medium { ct: "video/3gpp", bytes: &video }, Medium { ct: "text/x-vCard", bytes: &card }];
        let message = Message { key: KEY, to: &["0465714242"], date: WRITTEN, words: "Trois choses.", media: &media };
        let pdu = encode(&message).unwrap();
        let read = read(&pdu);
        let names: Vec<(&str, &str)> = read.parts.iter().map(|p| (p.ct.as_str(), p.name.as_str())).collect();
        assert_eq!(names, [("application/smil", "smil.xml"), ("image/gif", "image1.gif"), ("video/3gpp", "video1.3gp"), ("text/x-vCard", "contact1.vcf"), ("text/plain", "text1.txt")]);
        let layout = String::from_utf8(read.parts[0].data.clone()).unwrap();
        assert!(layout.contains(r#"<par><video src="video1.3gp" region="Image" /></par>"#), "{layout}");
        assert!(layout.contains(r#"<par dur="5000ms"><ref src="contact1.vcf" /></par>"#), "{layout}");
        // A content type WSP numbers goes by its number, another by its words.
        assert!(pdu.windows(b"video/3gpp\0".len()).any(|w| w == b"video/3gpp\0"));
        assert!(!pdu.windows(b"image/gif".len()).any(|w| w == b"image/gif"));
        assert_eq!(size(&message), Ok(pdu.len()));
    }

    /// The measure is the message's own length, whatever its size: a large
    /// picture (a length in three bytes), several, words of 2,000 bytes.
    #[test]
    fn the_measure_is_the_message_s_length() {
        let (large, small) = (stand_in(600_000, 3), stand_in(16_384, 4));
        let words = "é".repeat(1_000);
        for media in [vec![Medium { ct: "image/jpeg", bytes: &large }], vec![Medium { ct: "image/png", bytes: &small }; 5], vec![]] {
            let message = Message { key: KEY, to: &["0536494242"], date: WRITTEN, words: &words, media: &media };
            assert_eq!(size(&message), Ok(encode(&message).unwrap().len()));
        }
    }

    /// The size counted from the media's lengths alone is the message's own.
    #[test]
    fn the_size_from_lengths_is_the_message_s_own() {
        for lengths in [vec![0usize], vec![127, 128], vec![16_383, 16_384, 1], vec![600_000], vec![3, 30_000, 2_097_152]] {
            let blobs: Vec<Vec<u8>> = lengths.iter().map(|&n| stand_in(n, n as u8)).collect();
            let full: Vec<Medium> = blobs.iter().map(|bytes| Medium { ct: "image/jpeg", bytes }).collect();
            let empty: Vec<Medium> = lengths.iter().map(|_| Medium { ct: "image/jpeg", bytes: &[] }).collect();
            let message = Message { key: KEY, to: &["0199001234"], date: WRITTEN, words: "Le dîner.", media: &full };
            assert_eq!(size_with_lengths(&Message { media: &empty, ..message }, &lengths), size(&message), "{lengths:?}");
        }
    }

    /// The room for the media under a limit, shared among one, two or five
    /// pictures and filled: the message fits, within two bytes a picture of
    /// the limit; words over the limit leave none.
    #[test]
    fn the_media_fill_their_room_and_no_more() {
        let limit = 614_400;
        for count in [1usize, 2, 5] {
            let empty = vec![Medium { ct: "image/jpeg", bytes: &[] }; count];
            let message = Message { key: KEY, to: &["0199001234"], date: WRITTEN, words: "Les photos de samedi.", media: &empty };
            let room = room(&message, limit).unwrap();
            let shares: Vec<Vec<u8>> = (0..count).map(|i| stand_in(room / count + usize::from(i < room % count), i as u8)).collect();
            let media: Vec<Medium> = shares.iter().map(|bytes| Medium { ct: "image/jpeg", bytes }).collect();
            let full = size(&Message { media: &media, ..message }).unwrap();
            assert!(full <= limit && limit - full <= 2 * count, "{count}: {full} of {limit}");
        }
        let empty = [Medium { ct: "image/jpeg", bytes: &[] }];
        assert_eq!(room(&Message { key: KEY, to: &["0199001234"], date: WRITTEN, words: &"é".repeat(400_000), media: &empty }, limit), None, "words over the limit leave no room");
    }

    #[test]
    fn numbers_are_written_plainly() {
        assert_eq!(address("05 36 49 12 34").as_deref(), Some("0536491234"));
        assert_eq!(address(" +33 1-99.00/12 (34) ").as_deref(), Some("+33199001234"));
        assert_eq!(address("+33\u{202f}1\u{a0}99 00 12 34").as_deref(), Some("+33199001234"));
        assert_eq!(address("SIOUL"), None, "a sender named in letters");
        assert_eq!(address("36179").as_deref(), Some("36179"), "a short code is a number; the phone's rules refuse it");
        assert_eq!(address("12"), None);
        assert_eq!(address("05+36"), None, "a + inside");
    }

    #[test]
    fn what_cannot_be_written_is_refused() {
        let picture = stand_in(10, 5);
        let media = [Medium { ct: "image/jpeg", bytes: &picture }];
        let good = Message { key: KEY, to: &["0199001234"], date: WRITTEN, words: "", media: &media };
        assert!(encode(&good).is_ok(), "a picture alone");
        assert_eq!(encode(&Message { to: &[], ..good }), Err(Refused::Number));
        assert_eq!(encode(&Message { to: &["0199001234", "Alice"], ..good }), Err(Refused::Number));
        assert_eq!(encode(&Message { media: &[], ..good }), Err(Refused::Empty));
        assert_eq!(encode(&Message { key: "", ..good }), Err(Refused::Key));
        assert_eq!(encode(&Message { key: "clé", ..good }), Err(Refused::Key));
        assert_eq!(size(&Message { media: &[Medium { ct: "image/jpé", bytes: &picture }], ..good }), Err(Refused::Type));
        assert_eq!(size(&Message { media: &[Medium { ct: "", bytes: &picture }], ..good }), Err(Refused::Type));
    }

    /// The carrier's answer: taken, with its Message-ID; refused, with its
    /// words; a header not needed passed over; anything else, none.
    #[test]
    fn the_carrier_s_answer_is_read() {
        let taken = conf_taken();
        assert_eq!(conf(&taken), Some(Conf { status: OK, id: "msg-20261018-1403-42".into(), key: KEY.into(), said: String::new() }));
        assert!(conf(&taken).unwrap().ok());
        let refused = conf_refused();
        let read = conf(&refused).unwrap();
        assert_eq!((read.status, read.said.as_str(), read.ok()), (0xE5, "Contenu refusé", false));
        // A header Sioul does not need, in each of its forms, passed over.
        let mut more = taken.clone();
        more.extend_from_slice(&[0x83]);
        more.extend_from_slice(b"http://mms.example.org/x\0");
        more.extend_from_slice(&[0xA7, 0x80, 0x88, 0x05, 0x81, 0x03, 0x01, 0x51, 0x80]);
        assert_eq!(conf(&more).map(|c| c.id), Some("msg-20261018-1403-42".into()));
        assert_eq!(conf(&[0x8C, 0x80, 0x98, b'a', 0]), None, "a send request is no answer");
        assert_eq!(conf(&taken[..taken.len() - 4]), None, "cut short");
        assert_eq!(conf(&[0x8C, 0x81, 0x98, b'a', 0, 0x8D, 0x92]), None, "no status");
    }

    fn conf_taken() -> Vec<u8> {
        let mut out = vec![0x8C, 0x81, 0x98];
        text(&mut out, KEY.as_bytes());
        out.extend_from_slice(&[0x8D, 0x92, 0x92, 0x80, 0x8B]);
        text(&mut out, b"msg-20261018-1403-42");
        out
    }

    fn conf_refused() -> Vec<u8> {
        let mut out = vec![0x8C, 0x81, 0x98];
        text(&mut out, KEY.as_bytes());
        out.extend_from_slice(&[0x8D, 0x92, 0x92, 0xE5, 0x93]);
        let mut said = vec![0x80 | UTF_8];
        text(&mut said, "Contenu refusé".as_bytes());
        value_length(&mut out, said.len());
        out.extend_from_slice(&said);
        out
    }

    /// The samples Android's own parser reads (`android/jvm-checks/MmsCheck.java`):
    /// each message as `encode` writes it, with what it must read there.
    fn samples() -> Vec<serde_json::Value> {
        let picture = stand_in(200, 7);
        let (gif, video, card) = (stand_in(64, 1), stand_in(300, 2), b"BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Alice\r\nEND:VCARD\r\n".to_vec());
        let (edge_127, edge_128, large) = (stand_in(127, 8), stand_in(128, 9), stand_in(16_384, 10));
        let messages: Vec<(&str, Vec<&str>, String, Vec<(&str, &[u8])>)> = vec![
            ("a picture with words", vec!["+33 1 99 00 12 34"], "Voilà l’entrée.".into(), vec![("image/jpeg", &picture)]),
            ("words to two", vec!["+33 2 61 91 00 42", "03.53.01.42.42"], "Rendez-vous à 18 h.".into(), vec![]),
            ("three media", vec!["0465714242"], "Trois choses.".into(), vec![("image/gif", &gif), ("video/3gpp", &video), ("text/x-vCard", &card)]),
            // A part's bytes at 127 and 128 (one byte of length, then two); a content type's at 30 and 31.
            ("lengths at their edges", vec!["0536494242"], String::new(), vec![("image/png", &edge_127), ("image/png", &edge_128), ("application/x-abc", &edge_127), ("application/x-abcd", &edge_128)]),
            ("a large picture and long words", vec!["0199001234"], "é".repeat(1_000), vec![("image/jpeg", &large)]),
        ];
        let mut out = Vec::new();
        for (name, to, words, media) in &messages {
            let media: Vec<Medium> = media.iter().map(|&(ct, bytes)| Medium { ct, bytes }).collect();
            let message = Message { key: KEY, to, date: WRITTEN, words, media: &media };
            let pdu = encode(&message).unwrap();
            let read = read(&pdu);
            let parts: Vec<serde_json::Value> = read.parts.iter().map(|p| serde_json::json!({ "ct": p.ct, "name": p.name, "id": p.id, "location": p.location, "charset": p.charset, "size": p.data.len(), "sha256": sha256(&p.data) })).collect();
            let numbers: Vec<String> = to.iter().map(|n| address(n).unwrap()).collect();
            out.push(serde_json::json!({ "name": name, "kind": "send-req", "key": KEY, "to": numbers, "date": WRITTEN, "parts": parts, "pdu": hex(&pdu) }));
        }
        out.push(serde_json::json!({ "name": "the carrier took it", "kind": "send-conf", "key": KEY, "status": OK, "id": "msg-20261018-1403-42", "pdu": hex(&conf_taken()) }));
        out.push(serde_json::json!({ "name": "the carrier refused its content", "kind": "send-conf", "key": KEY, "status": 0xE5, "id": "", "pdu": hex(&conf_refused()) }));
        out
    }

    /// The samples in `android/jvm-checks/mms-samples.jsonl` are what
    /// `encode` writes now, so that Android's own parser checks this very
    /// encoder (`android/jvm-checks/run.sh`). After a change of the
    /// encoder: `SIOUL_MMS_SAMPLES_WRITE=1 cargo test --release -p sioul-core
    /// mms`, then that check.
    #[test]
    fn the_samples_android_reads_are_these() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../android/jvm-checks/mms-samples.jsonl");
        let now: String = samples().iter().map(|line| format!("{line}\n")).collect();
        if std::env::var_os("SIOUL_MMS_SAMPLES_WRITE").is_some() {
            std::fs::write(&path, &now).unwrap();
        }
        let kept = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(kept == now, "{} is not what the encoder writes now: SIOUL_MMS_SAMPLES_WRITE=1 cargo test --release -p sioul-core mms, then android/jvm-checks/run.sh", path.display());
    }
}
