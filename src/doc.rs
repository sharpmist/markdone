//! The lines of a list and what the commands do to them. Pure: functions take a list's bytes and
//! change them in memory; reading and writing files is `store.rs`'s job. Works on bytes, so a list
//! that isn't valid UTF-8 works as in 0.3.0. See docs/designs/rust_port.md, "The list in memory"
//! and "Line endings".

use crate::jira::{self, Jira};
use crate::text;

const BOM: &[u8] = b"\xef\xbb\xbf";
const OPEN: &[u8] = b"- [ ] ";
const DONE: &[u8] = b"- [x] ";

/// One line: its content (without the line ending) and whether it ends in CRLF.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Line {
    content: Vec<u8>,
    crlf: bool,
}

/// A list in memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Doc {
    bom: bool,
    lines: Vec<Line>,
    /// The file's line ending, for the lines `t` writes: CRLF when the first line ends in CRLF.
    crlf: bool,
}

/// An open item: its number (from 1, across all sections), section and text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub n: usize,
    pub section: Vec<u8>,
    pub text: Vec<u8>,
}

/// Why a command can't change the list. The caller turns these into 0.3.0's messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// No open item with this number.
    NoItem,
    /// No `## <section>` heading.
    NoHeading(Vec<u8>),
    /// `t done`: no open item with this number (or it has no text), or no `## Done` heading.
    CantFinish,
    /// The item is already in this section.
    AlreadyIn(Vec<u8>),
    /// Text matches no open item.
    NoMatch,
    /// Text matches several open items: their `t list` rows.
    Ambiguous(Vec<Vec<u8>>),
}

/// The result of tidying.
#[derive(Debug, PartialEq, Eq)]
pub enum Tidied {
    /// The list tidied, and the lines taken out for the archives (content only), in list order.
    Done { archived: Vec<Vec<u8>> },
    /// A ticked item has no `## Done` heading after it; the list is left as it was.
    NoDoneHeading,
}

fn heading_name(content: &[u8]) -> Option<&[u8]> {
    content.strip_prefix(b"## ")
}

fn is_blank(content: &[u8]) -> bool {
    content.iter().all(|&b| b == b' ' || b == b'\t')
}

/// Whether the 10 bytes after `- [x] ` have the shape of a date, `NNNN-NN-NN`.
fn is_dated(content: &[u8]) -> bool {
    let Some(d) = content.get(6..16) else {
        return false;
    };
    d.iter().enumerate().all(|(i, &b)| {
        if i == 4 || i == 7 {
            b == b'-'
        } else {
            b.is_ascii_digit()
        }
    })
}

/// The date of a dated done item (`- [x] YYYY-MM-DD …`), or empty.
fn date_of(content: &[u8]) -> &[u8] {
    content.get(6..16).unwrap_or(&[])
}

/// A `t list` row: `printf "%3d  %-6s %s"`.
pub fn row(n: usize, section: &[u8], text: &[u8]) -> Vec<u8> {
    let mut out = format!("{n:>3}  ").into_bytes();
    out.extend(text::pad(section, 6));
    out.push(b' ');
    out.extend_from_slice(text);
    out
}

/// The number an `<item>` reference of digits only names, or `None` for other references. A
/// number too large to be an item is `Some(usize::MAX)`, which names no item.
pub fn number(reference: &[u8]) -> Option<usize> {
    if reference.is_empty() || !reference.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let digits = std::str::from_utf8(reference).ok()?;
    Some(digits.parse().unwrap_or(usize::MAX))
}

impl Doc {
    /// Splits a list's bytes into lines.
    pub fn parse(bytes: &[u8]) -> Doc {
        let (bom, bytes) = match bytes.strip_prefix(BOM) {
            Some(rest) => (true, rest),
            None => (false, bytes),
        };
        let mut pieces: Vec<&[u8]> = bytes.split(|&b| b == b'\n').collect();
        let ends_with_newline = pieces.last().is_some_and(|p| p.is_empty());
        if ends_with_newline {
            pieces.pop();
        }
        let has_newline = !pieces.is_empty() && (ends_with_newline || pieces.len() > 1);
        let crlf = has_newline && pieces[0].ends_with(b"\r");
        let last = pieces.len().wrapping_sub(1);
        let lines = pieces
            .iter()
            .enumerate()
            .map(|(i, p)| match p.strip_suffix(b"\r") {
                Some(c) => Line {
                    content: c.to_vec(),
                    crlf: true,
                },
                // A last line without a newline gets the file's ending when written.
                None if i == last && !ends_with_newline => Line {
                    content: p.to_vec(),
                    crlf,
                },
                None => Line {
                    content: p.to_vec(),
                    crlf: false,
                },
            })
            .collect();
        Doc { bom, lines, crlf }
    }

    /// The list's bytes. Every line ends with its line ending, the last one included.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        if self.bom {
            out.extend_from_slice(BOM);
        }
        for l in &self.lines {
            out.extend_from_slice(&l.content);
            out.extend_from_slice(if l.crlf { b"\r\n" } else { b"\n" });
        }
        out
    }

    /// Whether the file's line ending is CRLF (for a new archive file of this list).
    pub fn crlf(&self) -> bool {
        self.crlf
    }

    /// A line `t` writes, with the file's ending.
    fn new_line(&self, content: Vec<u8>) -> Line {
        Line {
            content,
            crlf: self.crlf,
        }
    }

    /// The open items, in order, with the index of their line.
    fn open_with_index(&self) -> Vec<(usize, Item)> {
        let mut section: &[u8] = b"";
        let mut out = Vec::new();
        for (i, l) in self.lines.iter().enumerate() {
            if let Some(name) = heading_name(&l.content) {
                section = name;
            } else if let Some(text) = l.content.strip_prefix(OPEN) {
                let n = out.len() + 1;
                out.push((
                    i,
                    Item {
                        n,
                        section: section.to_vec(),
                        text: text.to_vec(),
                    },
                ));
            }
        }
        out
    }

    /// The open items, numbered from 1 across all sections.
    pub fn open_items(&self) -> Vec<Item> {
        self.open_with_index()
            .into_iter()
            .map(|(_, it)| it)
            .collect()
    }

    fn open_item(&self, n: usize) -> Option<(usize, Item)> {
        self.open_with_index().into_iter().find(|(_, it)| it.n == n)
    }

    /// The `- [x] ` lines in `## Done` sections, in order (content only).
    pub fn done_items(&self) -> Vec<Vec<u8>> {
        let mut in_done = false;
        let mut out = Vec::new();
        for l in &self.lines {
            if let Some(name) = heading_name(&l.content) {
                in_done = name == b"Done";
            } else if in_done && l.content.starts_with(DONE) {
                out.push(l.content.clone());
            }
        }
        out
    }

    /// The `t list` rows of the open items, optionally only one section's (keeping the numbers).
    pub fn rows(&self, only: Option<&[u8]>) -> Vec<Vec<u8>> {
        self.open_items()
            .into_iter()
            .filter(|it| only.is_none_or(|s| s == it.section))
            .map(|it| row(it.n, &it.section, &it.text))
            .collect()
    }

    /// The number of the open item `reference` names: digits are the number (checked later by the
    /// command); other text must match exactly one open item, case-insensitively and literally, as
    /// a terminal shows it (web links by their text).
    pub fn find(&self, reference: &[u8]) -> Result<usize, Error> {
        if let Some(n) = number(reference) {
            return Ok(n);
        }
        let want = text::lower(reference);
        let matches: Vec<Item> = self
            .open_items()
            .into_iter()
            .filter(|it| text::lower(&text::shown(&it.text)).contains(&want))
            .collect();
        match matches.as_slice() {
            [] => Err(Error::NoMatch),
            [one] => Ok(one.n),
            _ => Err(Error::Ambiguous(
                matches
                    .iter()
                    .map(|it| row(it.n, &it.section, &it.text))
                    .collect(),
            )),
        }
    }

    /// Where an item added to `section` goes: after the section's last non-blank line, before its
    /// trailing blank lines. `None` when there is no `## <section>` heading.
    fn insert_at(&self, section: &[u8]) -> Option<usize> {
        let start = self
            .lines
            .iter()
            .position(|l| heading_name(&l.content) == Some(section))?;
        let mut at = start + 1;
        for (i, l) in self.lines.iter().enumerate().skip(start + 1) {
            if heading_name(&l.content).is_some() {
                break;
            }
            if !is_blank(&l.content) {
                at = i + 1;
            }
        }
        Some(at)
    }

    /// Appends `- [ ] <text>` to the end of `section`; returns the new item's number.
    pub fn insert(&mut self, section: &[u8], text: &[u8]) -> Result<usize, Error> {
        let at = self
            .insert_at(section)
            .ok_or_else(|| Error::NoHeading(section.to_vec()))?;
        let n = 1 + self.lines[..at]
            .iter()
            .filter(|l| l.content.starts_with(OPEN))
            .count();
        let line = self.new_line([OPEN, text].concat());
        self.lines.insert(at, line);
        Ok(n)
    }

    /// `t done`: removes open item `n` and writes `- [x] <today> <text>` directly under the first
    /// `## Done`; returns the text.
    pub fn finish(&mut self, n: usize, today: &str) -> Result<Vec<u8>, Error> {
        let (i, item) = self.open_item(n).ok_or(Error::CantFinish)?;
        if item.text.is_empty() {
            return Err(Error::CantFinish);
        }
        let done = self
            .lines
            .iter()
            .position(|l| l.content == b"## Done")
            .ok_or(Error::CantFinish)?;
        let line = self.new_line([DONE, today.as_bytes(), b" ", &item.text].concat());
        self.lines.insert(done + 1, line);
        let i = if i > done { i + 1 } else { i };
        self.lines.remove(i);
        Ok(item.text)
    }

    /// `t rm`: deletes open item `n`'s line; returns its text.
    pub fn remove(&mut self, n: usize) -> Result<Vec<u8>, Error> {
        let (i, item) = self.open_item(n).ok_or(Error::NoItem)?;
        self.lines.remove(i);
        Ok(item.text)
    }

    /// `t reword`: replaces open item `n`'s text in place; returns its section.
    pub fn reword(&mut self, n: usize, text: &[u8]) -> Result<Vec<u8>, Error> {
        let (i, item) = self.open_item(n).ok_or(Error::NoItem)?;
        self.lines[i] = self.new_line([OPEN, text].concat());
        Ok(item.section)
    }

    /// `t today` and `t later`: moves open item `n` to the end of `section`; returns its new
    /// number and its text.
    pub fn move_within(&mut self, n: usize, section: &[u8]) -> Result<(usize, Vec<u8>), Error> {
        let (i, item) = self.open_item(n).ok_or(Error::NoItem)?;
        if item.section == section {
            return Err(Error::AlreadyIn(section.to_vec()));
        }
        let mut moved = self.clone();
        moved.lines.remove(i);
        let new_n = moved.insert(section, &item.text)?;
        *self = moved;
        Ok((new_n, item.text))
    }

    /// Open item `n`'s section and text, for the source half of `t move`.
    pub fn take(&self, n: usize) -> Result<Item, Error> {
        self.open_item(n).map(|(_, it)| it).ok_or(Error::NoItem)
    }

    /// Tidies the list: links Jira keys on item lines, gives today's date to undated done items,
    /// moves ticked items from other sections to the top of the next `## Done` after them, and
    /// takes done items dated before `cutoff` (compared as strings) out for the archives.
    pub fn tidy(&mut self, today: &str, cutoff: &str, jira: Option<&Jira>) -> Tidied {
        let stamp = |content: &[u8]| -> Vec<u8> {
            if is_dated(content) {
                content.to_vec()
            } else {
                [DONE, today.as_bytes(), b" ", &content[6..]].concat()
            }
        };
        let mut lines: Vec<Line> = Vec::with_capacity(self.lines.len());
        let mut archived = Vec::new();
        let mut moved: Vec<Vec<u8>> = Vec::new();
        let mut in_done = false;
        for l in &self.lines {
            let content = match jira {
                Some(j) if l.content.starts_with(OPEN) || l.content.starts_with(DONE) => {
                    [&l.content[..6], &jira::linkify(&l.content[6..], j)].concat()
                }
                _ => l.content.clone(),
            };
            let changed = content != l.content;
            let mut keep = |c: Vec<u8>, lines: &mut Vec<Line>, crlf: bool| {
                if is_dated(&c) && date_of(&c) < cutoff.as_bytes() {
                    archived.push(c);
                } else {
                    lines.push(Line { content: c, crlf });
                }
            };
            if content == b"## Done" {
                lines.push(Line {
                    content,
                    crlf: l.crlf,
                });
                in_done = true;
                for c in moved.drain(..) {
                    keep(c, &mut lines, self.crlf);
                }
            } else if content.starts_with(DONE) {
                let c = stamp(&content);
                if in_done {
                    let crlf = if changed || c != content {
                        self.crlf
                    } else {
                        l.crlf
                    };
                    keep(c, &mut lines, crlf);
                } else {
                    moved.push(c);
                }
            } else {
                if heading_name(&content).is_some() {
                    in_done = false;
                }
                let crlf = if changed { self.crlf } else { l.crlf };
                lines.push(Line { content, crlf });
            }
        }
        if !moved.is_empty() {
            return Tidied::NoDoneHeading;
        }
        self.lines = lines;
        Tidied::Done { archived }
    }
}

/// `t list --done` rows from done items (the list's, then its archives'): sorted by date, newest
/// first, stable for equal dates; each `DATE  text`.
pub fn done_rows(mut items: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    items.sort_by(|a, b| date_of(b).cmp(date_of(a)));
    items
        .into_iter()
        .map(|c| {
            let mut out = date_of(&c).to_vec();
            out.extend_from_slice(b"  ");
            out.extend_from_slice(c.get(17..).unwrap_or(&[]));
            out
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TODAY: &str = "2026-11-14";
    const D7: &str = "2026-11-07";
    const D8: &str = "2026-11-06";
    const CUTOFF: &str = "2026-11-07";
    const EMPTY: &str = "## Today\n\n## Later\n\n## Done\n";

    fn doc(s: &str) -> Doc {
        Doc::parse(s.as_bytes())
    }

    fn text(d: &Doc) -> String {
        String::from_utf8(d.to_bytes()).unwrap()
    }

    fn rows(d: &Doc, only: Option<&str>) -> String {
        d.rows(only.map(str::as_bytes))
            .into_iter()
            .map(|r| String::from_utf8(r).unwrap() + "\n")
            .collect()
    }

    fn s(b: Vec<u8>) -> String {
        String::from_utf8(b).unwrap()
    }

    #[test]
    fn round_trip_keeps_bytes() {
        for t in [
            EMPTY,
            "",
            "## Today\r\n- [ ] a\r\n\r\n## Done\r\n",
            "## Today\n- [ ] a\r\n## Done\n",
            "\u{feff}## Today\n- [ ] a\n",
            "notes\n# not a heading\n### nor this\n",
        ] {
            assert_eq!(text(&doc(t)), t, "{t:?}");
        }
        let latin1 = b"## Today\n- [ ] caf\xe9\n".to_vec();
        assert_eq!(Doc::parse(&latin1).to_bytes(), latin1);
    }

    #[test]
    fn a_missing_final_newline_is_added() {
        assert_eq!(text(&doc("## Today\n- [ ] a")), "## Today\n- [ ] a\n");
        assert_eq!(text(&doc("## Today\r\n- [ ] a")), "## Today\r\n- [ ] a\r\n");
        assert_eq!(text(&doc("## Today")), "## Today\n");
    }

    #[test]
    fn adding_and_listing() {
        let mut d = doc(EMPTY);
        assert_eq!(rows(&d, None), "");
        assert_eq!(d.insert(b"Today", b"first"), Ok(1));
        assert_eq!(d.insert(b"Later", b"some day"), Ok(2));
        assert_eq!(d.insert(b"Later", b"short"), Ok(3));
        assert_eq!(d.insert(b"Today", b"second"), Ok(2));
        let weird = r#"Ticket & "quotes" back\slash \n $HOME `x`"#;
        assert_eq!(d.insert(b"Today", weird.as_bytes()), Ok(3));
        assert_eq!(
            rows(&d, None),
            format!(
                "  1  Today  first\n  2  Today  second\n  3  Today  {weird}\n  4  Later  some day\n  5  Later  short\n"
            )
        );
        assert_eq!(
            rows(&d, Some("Later")),
            "  4  Later  some day\n  5  Later  short\n"
        );
        assert_eq!(
            d.insert(b"Nowhere", b"x"),
            Err(Error::NoHeading(b"Nowhere".to_vec()))
        );
    }

    #[test]
    fn insert_goes_before_trailing_blank_lines() {
        let mut d = doc("## Today\n- [ ] a\n\nnote\n \t\n\n## Done\n");
        assert_eq!(d.insert(b"Today", b"b"), Ok(2));
        assert_eq!(
            text(&d),
            "## Today\n- [ ] a\n\nnote\n- [ ] b\n \t\n\n## Done\n"
        );
        let mut d = doc("## Today\n\n## Done\n\n## Later");
        assert_eq!(d.insert(b"Later", b"x"), Ok(1));
        assert_eq!(text(&d), "## Today\n\n## Done\n\n## Later\n- [ ] x\n");
        // The first heading with the name wins.
        let mut d = doc("## Today\n## Today\n- [ ] later one\n");
        assert_eq!(d.insert(b"Today", b"x"), Ok(1));
        assert_eq!(text(&d), "## Today\n- [ ] x\n## Today\n- [ ] later one\n");
    }

    #[test]
    fn done_writes_under_the_first_done_heading() {
        let weird = r#"Ticket & "quotes""#;
        let mut d = doc(&format!(
            "## Today\n- [ ] first\n- [ ] second\n- [ ] {weird}\n\n## Later\n- [ ] some day\n\n## Done\n"
        ));
        assert_eq!(d.finish(2, TODAY).map(s), Ok("second".to_string()));
        assert_eq!(d.finish(2, TODAY).map(s), Ok(weird.to_string()));
        assert_eq!(
            text(&d),
            format!(
                "## Today\n- [ ] first\n\n## Later\n- [ ] some day\n\n## Done\n- [x] {TODAY} {weird}\n- [x] {TODAY} second\n"
            )
        );
        assert_eq!(d.finish(99, TODAY), Err(Error::CantFinish));
        let mut d = doc("## Today\n- [ ] a\n");
        assert_eq!(d.finish(1, TODAY), Err(Error::CantFinish));
        let mut d = doc("## Today\n- [ ] \n## Done\n");
        assert_eq!(d.finish(1, TODAY), Err(Error::CantFinish), "empty text");
        // An item below Done is removed from the right place.
        let mut d = doc("## Done\n## Other\n- [ ] x\n");
        assert_eq!(d.finish(1, TODAY).map(s), Ok("x".to_string()));
        assert_eq!(text(&d), format!("## Done\n- [x] {TODAY} x\n## Other\n"));
    }

    #[test]
    fn today_and_later() {
        let mut d = doc("## Today\n- [ ] A\n\n## Later\n- [ ] B\n- [ ] C\n\n## Done\n");
        assert_eq!(
            d.move_within(3, b"Today").map(|(n, t)| (n, s(t))),
            Ok((2, "C".into()))
        );
        assert_eq!(
            d.move_within(3, b"Today").map(|(n, t)| (n, s(t))),
            Ok((3, "B".into()))
        );
        assert_eq!(
            text(&d),
            "## Today\n- [ ] A\n- [ ] C\n- [ ] B\n\n## Later\n\n## Done\n"
        );
        let before = d.clone();
        assert_eq!(
            d.move_within(1, b"Today"),
            Err(Error::AlreadyIn(b"Today".to_vec()))
        );
        assert_eq!(d.move_within(9, b"Today"), Err(Error::NoItem));
        assert_eq!(d, before);
        let mut d = doc("## Later\n- [ ] B\n\n## Done\n");
        assert_eq!(
            d.move_within(1, b"Today"),
            Err(Error::NoHeading(b"Today".to_vec()))
        );
        assert_eq!(text(&d), "## Later\n- [ ] B\n\n## Done\n", "item not lost");
        let mut d = doc("## Today\n- [ ] A\n- [ ] B\n\n## Later\n- [ ] C\n\n## Done\n");
        assert_eq!(d.move_within(1, b"Later").map(|(n, _)| n), Ok(3));
        assert_eq!(d.move_within(1, b"Later").map(|(n, _)| n), Ok(3));
        assert_eq!(
            text(&d),
            "## Today\n\n## Later\n- [ ] C\n- [ ] A\n- [ ] B\n\n## Done\n"
        );
    }

    #[test]
    fn sections_follow_0_3_rules() {
        let d = doc(
            "- [ ] before any heading\n## Today\n- [ ] a\n# not a heading\n- [X] capital\n  - [ ] indented\n## Someday\n- [ ] b\n## Today \n- [ ] c\n",
        );
        assert_eq!(
            rows(&d, None),
            "  1         before any heading\n  2  Today  a\n  3  Someday b\n  4  Today  c\n"
        );
        let it = d.take(4).unwrap();
        assert_eq!(
            it.section, b"Today ",
            "a trailing space is part of the name"
        );
        // Items under other headings can be moved.
        let mut d = doc("## Today\n## Someday\n- [ ] b\n");
        assert_eq!(d.move_within(1, b"Today").map(|(n, _)| n), Ok(1));
    }

    #[test]
    fn find_by_number_or_text() {
        let p7 = "[PROJ-7](https://jira.example.com/browse/PROJ-7)";
        let d = doc(&format!(
            "## Today\n- [ ] Fix login bug\n- [ ] Write docs (v2.1)\n\n## Later\n- [ ] Fix logout {p7}\n\n## Done\n- [x] {TODAY} done item\n"
        ));
        assert_eq!(d.find(b"2"), Ok(2));
        assert_eq!(d.find(b"007"), Ok(7));
        assert_eq!(d.find(b"99999999999999999999999"), Ok(usize::MAX));
        assert_eq!(d.find(b"write"), Ok(2));
        assert_eq!(d.find(b"LOGIN BUG"), Ok(1));
        assert_eq!(d.find(b"(v2"), Ok(2), "literal, not a pattern");
        assert_eq!(d.find(b"PROJ-7"), Ok(3), "a linked key matches by its text");
        assert_eq!(d.find(b"f.x"), Err(Error::NoMatch));
        assert_eq!(d.find(b"done item"), Err(Error::NoMatch), "only open items");
        assert_eq!(
            d.find(b"fix"),
            Err(Error::Ambiguous(vec![
                b"  1  Today  Fix login bug".to_vec(),
                format!("  3  Later  Fix logout {p7}").into_bytes(),
            ]))
        );
    }

    #[test]
    fn find_matches_what_a_terminal_shows() {
        let d = doc(
            "## Today\n- [ ] Fix logout [PROJ-7](https://jira.example.com/browse/PROJ-7)\n- [ ] Read [the guide](https://docs.example.com/guide) on https://example.org/faq\n- [ ] Ask about [local notes](notes.md)\n",
        );
        assert_eq!(d.find(b"jira.example"), Err(Error::NoMatch));
        assert_eq!(d.find(b"docs.example"), Err(Error::NoMatch));
        assert_eq!(d.find(b"browse"), Err(Error::NoMatch));
        assert_eq!(d.find(b"logout proj-7"), Ok(1));
        assert_eq!(d.find(b"the guide on"), Ok(2));
        assert_eq!(d.find(b"example.org"), Ok(2));
        assert_eq!(d.find(b"notes.md"), Ok(3));
        let d = doc("## Today\n- [ ] ÅSA meeting\n");
        assert_eq!(d.find("åsa".as_bytes()), Ok(1), "Unicode lower-casing");
    }

    #[test]
    fn rm_and_reword() {
        let mut d = doc(&format!(
            "## Today\n- [ ] keep\n- [ ] old idea\n\n## Later\n- [ ] later idea\n\n## Done\n- [x] {TODAY} done item\n"
        ));
        assert_eq!(d.remove(2).map(s), Ok("old idea".into()));
        assert_eq!(d.remove(9), Err(Error::NoItem));
        assert_eq!(
            text(&d),
            format!(
                "## Today\n- [ ] keep\n\n## Later\n- [ ] later idea\n\n## Done\n- [x] {TODAY} done item\n"
            )
        );
        assert_eq!(d.reword(2, b"Plan the release").map(s), Ok("Later".into()));
        assert_eq!(d.reword(9, b"x"), Err(Error::NoItem));
        assert_eq!(
            text(&d),
            format!(
                "## Today\n- [ ] keep\n\n## Later\n- [ ] Plan the release\n\n## Done\n- [x] {TODAY} done item\n"
            )
        );
    }

    #[test]
    fn tidy_dates_moves_and_archives() {
        let mut d = doc(&format!(
            "## Today\n- [ ] Open one\n- [x] Ticked in an editor\n\n## Later\n- [x] {D8} Old ticked later\n\n## Done\n- [x] {D7} Seven days\n- [x] Undated\n- [x] {D8} Eight days\n- [x] 2020-05-01 Ancient\n"
        ));
        let r = d.tidy(TODAY, CUTOFF, None);
        assert_eq!(
            r,
            Tidied::Done {
                archived: vec![
                    format!("- [x] {D8} Old ticked later").into_bytes(),
                    format!("- [x] {D8} Eight days").into_bytes(),
                    b"- [x] 2020-05-01 Ancient".to_vec(),
                ]
            }
        );
        assert_eq!(
            text(&d),
            format!(
                "## Today\n- [ ] Open one\n\n## Later\n\n## Done\n- [x] {TODAY} Ticked in an editor\n- [x] {D7} Seven days\n- [x] {TODAY} Undated\n"
            )
        );
        let again = d.clone();
        assert_eq!(
            d.tidy(TODAY, CUTOFF, None),
            Tidied::Done { archived: vec![] }
        );
        assert_eq!(d, again, "a second tidy changes nothing");
    }

    #[test]
    fn tidy_dates_are_a_shape_test() {
        let mut d = doc(
            "## Done\n- [x] 9999-99-99 never archived\n- [x] 2026-11-1234 dated by shape\n- [x] \n",
        );
        assert_eq!(
            d.tidy(TODAY, CUTOFF, None),
            Tidied::Done { archived: vec![] }
        );
        assert_eq!(
            text(&d),
            format!(
                "## Done\n- [x] 9999-99-99 never archived\n- [x] 2026-11-1234 dated by shape\n- [x] {TODAY} \n"
            )
        );
    }

    #[test]
    fn tidy_needs_a_done_heading_after_ticked_items() {
        let mut d = doc("## Today\n- [x] orphan\n");
        let before = d.clone();
        assert_eq!(d.tidy(TODAY, CUTOFF, None), Tidied::NoDoneHeading);
        assert_eq!(d, before);
        // The next Done heading after the item takes it.
        let mut d = doc("## Done\n## Today\n- [x] a\n## Done\n");
        assert_eq!(
            d.tidy(TODAY, CUTOFF, None),
            Tidied::Done { archived: vec![] }
        );
        assert_eq!(
            text(&d),
            format!("## Done\n## Today\n## Done\n- [x] {TODAY} a\n")
        );
    }

    #[test]
    fn tidy_links_jira_keys_on_item_lines_only() {
        let (j, _) = crate::jira::check_jira(b"PROJ WEB", b"https://jira.example.com/browse/");
        let l = |k: &str| format!("[{k}](https://jira.example.com/browse/{k})");
        let mut d = doc(&format!(
            "## Today\n- [ ] Typed in an editor PROJ-10\nNotes about PROJ-11\n\n## Done\n- [x] {TODAY} Shipped WEB-7\n"
        ));
        d.tidy(TODAY, CUTOFF, j.as_ref());
        assert_eq!(
            text(&d),
            format!(
                "## Today\n- [ ] Typed in an editor {}\nNotes about PROJ-11\n\n## Done\n- [x] {TODAY} Shipped {}\n",
                l("PROJ-10"),
                l("WEB-7")
            )
        );
    }

    #[test]
    fn done_rows_newest_first_stable() {
        let items = vec![
            format!("- [x] {TODAY} Ticked in an editor").into_bytes(),
            format!("- [x] {TODAY} Alpha").into_bytes(),
            format!("- [x] {D7} Bravo").into_bytes(),
            format!("- [x] {D8} Charlie").into_bytes(),
            b"- [x] 2020-05-01 Ancient".to_vec(),
            b"- [x] 2020-06-01 Later ancient".to_vec(),
        ];
        let got: Vec<String> = done_rows(items).into_iter().map(s).collect();
        assert_eq!(
            got,
            [
                format!("{TODAY}  Ticked in an editor"),
                format!("{TODAY}  Alpha"),
                format!("{D7}  Bravo"),
                format!("{D8}  Charlie"),
                "2020-06-01  Later ancient".to_string(),
                "2020-05-01  Ancient".to_string(),
            ]
        );
        let d = doc(&format!(
            "## Today\n- [x] {TODAY} not in Done\n## Done\n- [x] {TODAY} a\n- [ ] open\n## Later\n- [x] {TODAY} b\n"
        ));
        assert_eq!(
            d.done_items(),
            vec![format!("- [x] {TODAY} a").into_bytes()]
        );
    }

    #[test]
    fn crlf_lists() {
        let mut d = doc("## Today\r\n- [ ] a\r\n\r\n## Later\r\n\r\n## Done\r\n");
        assert_eq!(rows(&d, None), "  1  Today  a\n", "no CR in the text");
        assert_eq!(d.insert(b"Today", b"x"), Ok(2));
        assert_eq!(d.finish(1, TODAY).map(s), Ok("a".into()));
        assert_eq!(
            text(&d),
            format!("## Today\r\n- [ ] x\r\n\r\n## Later\r\n\r\n## Done\r\n- [x] {TODAY} a\r\n")
        );
        // A CR-only line counts as blank for the insert position.
        let mut d = doc("## Today\r\n- [ ] a\r\n\r\n## Done\r\n");
        d.insert(b"Today", b"b").unwrap();
        assert_eq!(
            text(&d),
            "## Today\r\n- [ ] a\r\n- [ ] b\r\n\r\n## Done\r\n"
        );
    }

    #[test]
    fn mixed_endings_keep_untouched_lines() {
        let mut d = doc("## Today\n- [ ] a\r\n- [ ] b\n## Done\r\n");
        d.reword(1, b"A").unwrap();
        assert_eq!(
            text(&d),
            "## Today\n- [ ] A\n- [ ] b\n## Done\r\n",
            "written lines take the first line's LF"
        );
        let mut d = doc("## Today\r\n- [x] ticked\n## Done\n");
        d.tidy(TODAY, CUTOFF, None);
        assert_eq!(
            text(&d),
            format!("## Today\r\n## Done\n- [x] {TODAY} ticked\r\n")
        );
    }

    #[test]
    fn byte_order_mark() {
        let mut d = doc("\u{feff}## Today\n\n## Done\n");
        assert_eq!(d.insert(b"Today", b"x"), Ok(1));
        assert_eq!(text(&d), "\u{feff}## Today\n- [ ] x\n\n## Done\n");
    }

    #[test]
    fn empty_file_has_no_lines() {
        let mut d = doc("");
        assert_eq!(
            d.tidy(TODAY, CUTOFF, None),
            Tidied::Done { archived: vec![] }
        );
        assert_eq!(d.to_bytes(), b"");
        assert!(!d.crlf());
    }

    #[test]
    fn rows_pad_by_characters() {
        assert_eq!(s(row(1, b"Today", b"x")), "  1  Today  x");
        assert_eq!(s(row(12, "Idé".as_bytes(), b"x")), " 12  Idé    x");
        assert_eq!(s(row(1000, b"Someday", b"x")), "1000  Someday x");
    }

    #[test]
    fn numbers() {
        assert_eq!(number(b"3"), Some(3));
        assert_eq!(number(b"03"), Some(3));
        assert_eq!(number(b""), None);
        assert_eq!(number(b"3a"), None);
        assert_eq!(number(b"-3"), None);
    }
}
