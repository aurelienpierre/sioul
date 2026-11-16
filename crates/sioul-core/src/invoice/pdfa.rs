// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The Factur-X properties in a PDF/A's XMP (Factur-X 1.07.2 §6.3,
//! research-generic.md §5): `fx:DocumentType` INVOICE, `fx:DocumentFileName`
//! factur-x.xml, `fx:Version` 1.0, `fx:ConformanceLevel` EN 16931, and the
//! PDF/A extension schema that declares them, as the specification's sample
//! XMP has it.
//!
//! krilla 0.8.2 writes the XMP but takes no property of one's own yet (its
//! issue #209, draft pull request #408). So the file krilla wrote gets its
//! metadata stream again, in place: krilla's own XMP, the Factur-X schema
//! among the extension schemas it declares and the properties beside, the
//! stream's `/Length` with it, and the offsets of
//! the cross-reference table after it moved by as much. One object
//! changes; the file stays one revision, its trailer and identifiers as
//! they were. A file this cannot read as krilla writes it (a classic
//! cross-reference table, the catalog naming its metadata) is refused,
//! never written half.

use super::cii;

/// The Factur-X schema, as a PDF/A extension schema describes it (an `rdf:li`
/// of `pdfaExtension:schemas`).
fn schema() -> String {
    let property = |name: &str, description: &str| {
        format!(
            "<rdf:li rdf:parseType=\"Resource\"><pdfaProperty:name>{name}</pdfaProperty:name><pdfaProperty:valueType>Text</pdfaProperty:valueType><pdfaProperty:category>external</pdfaProperty:category><pdfaProperty:description>{description}</pdfaProperty:description></rdf:li>"
        )
    };
    format!(
        "<rdf:li rdf:parseType=\"Resource\">\
         <pdfaSchema:schema>Factur-X PDFA Extension Schema</pdfaSchema:schema>\
         <pdfaSchema:namespaceURI>urn:factur-x:pdfa:CrossIndustryDocument:invoice:1p0#</pdfaSchema:namespaceURI>\
         <pdfaSchema:prefix>fx</pdfaSchema:prefix>\
         <pdfaSchema:property><rdf:Seq>{}{}{}{}</rdf:Seq></pdfaSchema:property>\
         </rdf:li>",
        property("DocumentFileName", "The name of the embedded XML document"),
        property("DocumentType", "The type of the hybrid document in capital letters, e.g. INVOICE or ORDER"),
        property("Version", "The actual version of the standard applying to the embedded XML document"),
        property("ConformanceLevel", "The conformance level of the embedded XML document"),
    )
}

/// The Factur-X properties themselves, in a description of their own.
fn properties() -> String {
    format!(
        "<rdf:Description rdf:about=\"\" xmlns:fx=\"urn:factur-x:pdfa:CrossIndustryDocument:invoice:1p0#\">\
         <fx:DocumentType>INVOICE</fx:DocumentType><fx:DocumentFileName>{}</fx:DocumentFileName><fx:Version>1.0</fx:Version><fx:ConformanceLevel>{}</fx:ConformanceLevel>\
         </rdf:Description>",
        cii::FILE_NAME,
        cii::CONFORMANCE,
    )
}

/// `xmp` with the Factur-X schema and properties: the schema joins the
/// extension schemas krilla declares (one `pdfaExtension:schemas` per
/// resource, else no XMP reader takes it: a property twice is an error),
/// or comes in a description of its own when there are none.
fn with_properties(xmp: &str) -> Option<String> {
    let closing = xmp.rfind("</rdf:RDF>")?;
    let mut out = String::with_capacity(xmp.len() + 2048);
    match xmp.find("</pdfaExtension:schemas>").and_then(|end| xmp[..end].rfind("</rdf:Bag>")) {
        Some(bag) => {
            out.push_str(&xmp[..bag]);
            out.push_str(&schema());
            out.push_str(&xmp[bag..closing]);
        }
        None => {
            out.push_str(&xmp[..closing]);
            out.push_str("<rdf:Description rdf:about=\"\" xmlns:pdfaExtension=\"http://www.aiim.org/pdfa/ns/extension/\" xmlns:pdfaSchema=\"http://www.aiim.org/pdfa/ns/schema#\" xmlns:pdfaProperty=\"http://www.aiim.org/pdfa/ns/property#\"><pdfaExtension:schemas><rdf:Bag>");
            out.push_str(&schema());
            out.push_str("</rdf:Bag></pdfaExtension:schemas></rdf:Description>");
        }
    }
    out.push_str(&properties());
    out.push_str(&xmp[closing..]);
    Some(out)
}

fn find(bytes: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    bytes.get(from..)?.windows(needle.len()).position(|w| w == needle).map(|at| at + from)
}

fn rfind(bytes: &[u8], needle: &[u8]) -> Option<usize> {
    bytes.windows(needle.len()).rposition(|w| w == needle)
}

/// The decimal number at `at`, and where it ends.
fn number(bytes: &[u8], at: usize) -> Option<(usize, usize)> {
    let digits = bytes.get(at..)?.iter().take_while(|b| b.is_ascii_digit()).count();
    let text = std::str::from_utf8(&bytes[at..at + digits]).ok()?;
    Some((text.parse().ok()?, at + digits))
}

/// The object a dictionary's `key` refers to: `/Key 12 0 R` → 12.
fn reference(dictionary: &[u8], key: &[u8]) -> Option<usize> {
    let mut at = 0;
    while let Some(found) = find(dictionary, key, at) {
        let after = found + key.len();
        // `/Metadata` and not `/MetadataX`.
        if dictionary.get(after).is_some_and(|b| b.is_ascii_whitespace()) {
            let start = after + dictionary[after..].iter().take_while(|b| b.is_ascii_whitespace()).count();
            if let Some((object, end)) = number(dictionary, start)
                && dictionary.get(end..end + 4) == Some(b" 0 R")
            {
                return Some(object);
            }
        }
        at = after;
    }
    None
}

/// `pdf` with the Factur-X properties in its XMP.
pub fn with_factur_x(pdf: &[u8]) -> Result<Vec<u8>, String> {
    let fail = |what: &str| format!("the invoice's PDF: {what}");
    // The cross-reference table: `xref`, `0 N`, then N entries of 20 bytes.
    let startxref = rfind(pdf, b"startxref").ok_or_else(|| fail("no startxref"))?;
    let at = startxref + b"startxref".len();
    let at = at + pdf[at..].iter().take_while(|b| b.is_ascii_whitespace()).count();
    let (xref, _) = number(pdf, at).ok_or_else(|| fail("no table's offset"))?;
    if pdf.get(xref..xref + 7) != Some(b"xref\n0 ") {
        return Err(fail("not a classic cross-reference table"));
    }
    let (count, after_count) = number(pdf, xref + 7).ok_or_else(|| fail("no table's size"))?;
    let entries = after_count + 1;
    let trailer = entries + 20 * count;
    if pdf.get(trailer..trailer + 7) != Some(b"trailer") {
        return Err(fail("a cross-reference table of another shape"));
    }
    let offsets: Vec<Option<usize>> = (0..count)
        .map(|n| {
            let entry = &pdf[entries + 20 * n..entries + 20 * n + 20];
            (entry[17] == b'n').then(|| std::str::from_utf8(&entry[..10]).ok().and_then(|o| o.parse().ok())).flatten()
        })
        .collect();
    let object = |n: usize| -> Option<(usize, usize)> {
        let start = (*offsets.get(n)?)?;
        let end = find(pdf, b"endobj", start)? + b"endobj".len();
        Some((start, end))
    };
    let root = reference(&pdf[trailer..startxref], b"/Root").ok_or_else(|| fail("no catalog"))?;
    let (catalog_start, catalog_end) = object(root).ok_or_else(|| fail("no catalog"))?;
    let metadata = reference(&pdf[catalog_start..catalog_end], b"/Metadata").ok_or_else(|| fail("no metadata"))?;
    let (start, end) = object(metadata).ok_or_else(|| fail("no metadata"))?;
    let old = &pdf[start..end];
    let stream = find(old, b"stream\n", 0).ok_or_else(|| fail("no metadata stream"))? + b"stream\n".len();
    let stream_end = rfind(old, b"\nendstream").ok_or_else(|| fail("no metadata stream"))?;
    if find(&old[..stream], b"/Filter", 0).is_some() {
        return Err(fail("a filtered metadata stream"));
    }
    let xmp = std::str::from_utf8(&old[stream..stream_end]).map_err(|_| fail("an XMP that is not UTF-8"))?;
    let xmp = with_properties(xmp).ok_or_else(|| fail("an XMP without rdf:RDF"))?;
    // The stream's dictionary as krilla wrote it, its length the new one.
    let head = std::str::from_utf8(&old[..stream]).map_err(|_| fail("a metadata dictionary that is not text"))?;
    let length = head.find("/Length").ok_or_else(|| fail("a metadata stream without its length"))? + "/Length".len();
    let digits_at = length + head[length..].len() - head[length..].trim_start().len();
    let digits = head[digits_at..].bytes().take_while(u8::is_ascii_digit).count();
    let head = format!("{}{}{}", &head[..digits_at], xmp.len(), &head[digits_at + digits..]);
    let mut replaced = head.into_bytes();
    replaced.extend_from_slice(xmp.as_bytes());
    replaced.extend_from_slice(&old[stream_end..]);
    let delta = replaced.len() as i64 - old.len() as i64;
    // The file again: the objects before, the new one, those after, then the table moved.
    let mut out = Vec::with_capacity(pdf.len() + replaced.len());
    out.extend_from_slice(&pdf[..start]);
    out.extend_from_slice(&replaced);
    out.extend_from_slice(&pdf[end..xref]);
    let moved = |offset: usize| if offset > start { (offset as i64 + delta) as usize } else { offset };
    let new_xref = out.len();
    out.extend_from_slice(&pdf[xref..entries]);
    for (n, offset) in offsets.iter().enumerate() {
        match offset {
            Some(offset) => out.extend_from_slice(format!("{:010} {}", moved(*offset), std::str::from_utf8(&pdf[entries + 20 * n + 11..entries + 20 * n + 20]).unwrap_or("00000 n\r\n")).as_bytes()),
            None => out.extend_from_slice(&pdf[entries + 20 * n..entries + 20 * n + 20]),
        }
    }
    out.extend_from_slice(&pdf[trailer..startxref]);
    out.extend_from_slice(format!("startxref\n{new_xref}\n%%EOF").as_bytes());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One `pdfaExtension:schemas` whatever comes: the Factur-X schema joins
    /// krilla's, or comes alone; the properties after.
    #[test]
    fn the_schema_joins_the_ones_declared() {
        let krilla = "<x:xmpmeta><rdf:RDF><rdf:Description rdf:about=\"\"><pdfaExtension:schemas><rdf:Bag><rdf:li>xmpMM</rdf:li></rdf:Bag></pdfaExtension:schemas></rdf:Description></rdf:RDF></x:xmpmeta>";
        let joined = with_properties(krilla).unwrap();
        assert_eq!(joined.matches("<pdfaExtension:schemas>").count(), 1, "{joined}");
        assert!(joined.find("<rdf:li>xmpMM</rdf:li>").unwrap() < joined.find("Factur-X PDFA Extension Schema").unwrap());
        assert!(joined.find("</pdfaExtension:schemas>").unwrap() > joined.find("<pdfaSchema:prefix>fx</pdfaSchema:prefix>").unwrap());
        assert!(joined.contains("<fx:ConformanceLevel>EN 16931</fx:ConformanceLevel></rdf:Description></rdf:RDF>"));
        let bare = with_properties("<rdf:RDF><rdf:Description rdf:about=\"\"/></rdf:RDF>").unwrap();
        assert_eq!(bare.matches("<pdfaExtension:schemas>").count(), 1, "{bare}");
        assert!(with_properties("no RDF").is_none());
    }
}
