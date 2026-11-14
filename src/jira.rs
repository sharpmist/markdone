//! Jira linking: checking the settings, turning bare keys such as `PROJ-123` into Markdown links,
//! and finding the keys in an item for `t list --json`. Pure functions over bytes. See
//! docs/designs/rust_port.md, "Jira linking and display".

/// Valid Jira settings: the project keys and the browse URL the keys are appended to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Jira {
    keys: Vec<Vec<u8>>,
    url: Vec<u8>,
}

/// White space as zsh's default `IFS` splits words: space, tab, newline, NUL.
fn is_ifs(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0)
}

fn is_key(k: &[u8]) -> bool {
    matches!(k.first(), Some(b'A'..=b'Z'))
        && k.iter()
            .all(|&b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

/// Matches 0.3.0's `http(s|)://?*/`: the scheme, at least one more byte, and a final `/`.
fn is_browse_url(u: &[u8]) -> bool {
    let rest = u
        .strip_prefix(b"https://")
        .or_else(|| u.strip_prefix(b"http://"));
    rest.is_some_and(|r| r.len() >= 2 && r.ends_with(b"/"))
}

/// Checks `TODO_JIRA_KEYS` and `TODO_JIRA_URL` as 0.3.0's `check_jira` does. Linking is on when
/// the keys aren't blank and both settings are valid; otherwise it is off, with a warning (without
/// the `t: ` prefix) unless the keys are blank.
pub fn check_jira(keys: &[u8], url: &[u8]) -> (Option<Jira>, Option<String>) {
    let keys: Vec<&[u8]> = keys
        .split(|&b| is_ifs(b))
        .filter(|k| !k.is_empty())
        .collect();
    if keys.is_empty() {
        return (None, None);
    }
    let bad: Vec<String> = keys
        .iter()
        .filter(|k| !is_key(k))
        .map(|k| String::from_utf8_lossy(k).into_owned())
        .collect();
    let warning = if !bad.is_empty() {
        format!(
            "TODO_JIRA_KEYS: '{}' isn't a Jira project key (use uppercase keys separated by spaces); Jira linking is off",
            bad.join("', '")
        )
    } else if url.is_empty() {
        "TODO_JIRA_KEYS is set but TODO_JIRA_URL isn't; Jira linking is off".to_string()
    } else if !is_browse_url(url) {
        format!(
            "TODO_JIRA_URL must start with http:// or https:// and end with / (got '{}'); Jira linking is off",
            String::from_utf8_lossy(url)
        )
    } else {
        let jira = Jira {
            keys: keys.into_iter().map(<[u8]>::to_vec).collect(),
            url: url.to_vec(),
        };
        return (Some(jira), None);
    };
    (None, Some(warning))
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// The length of the Jira key at `s[i]` (`KEY-123`), or `None`: one of the keys, `-`, one or more
/// ASCII digits; the byte before is not a word byte, `/` or `-`, and the byte after the digits is
/// not a word byte or `-`. Keys are tried in their configured order.
fn key_at(s: &[u8], i: usize, jira: &Jira) -> Option<usize> {
    if i > 0 && (is_word(s[i - 1]) || matches!(s[i - 1], b'/' | b'-')) {
        return None;
    }
    jira.keys.iter().find_map(|k| {
        let rest = s[i..].strip_prefix(k.as_slice())?.strip_prefix(b"-")?;
        let digits = rest.iter().take_while(|b| b.is_ascii_digit()).count();
        let end = i + k.len() + 1 + digits;
        let after_ok = s.get(end).is_none_or(|&b| !is_word(b) && b != b'-');
        (digits > 0 && after_ok).then_some(end - i)
    })
}

/// The length of the span at `s[i]` that linking copies unchanged, or `None`: a Markdown link
/// (`[` … `](` … `)`, the text may be empty), inline code (`` ` `` … `` ` ``), or a bare URL
/// (`http://` or `https://` and the bytes up to the next ASCII white space, at least one).
fn skip_at(s: &[u8], i: usize) -> Option<usize> {
    let rest = &s[i..];
    match rest.first()? {
        b'[' => {
            let close = 1 + rest[1..].iter().position(|&b| b == b']')?;
            if rest.get(close + 1) != Some(&b'(') {
                return None;
            }
            let end = close + 2 + rest[close + 2..].iter().position(|&b| b == b')')?;
            Some(end + 1)
        }
        b'`' => Some(2 + rest[1..].iter().position(|&b| b == b'`')?),
        b'h' => {
            let scheme = if rest.starts_with(b"https://") {
                8
            } else if rest.starts_with(b"http://") {
                7
            } else {
                return None;
            };
            let n = rest[scheme..]
                .iter()
                .take_while(|&&b| !crate::text::is_ascii_space(b))
                .count();
            (n > 0).then_some(scheme + n)
        }
        _ => None,
    }
}

/// `text` with each bare Jira key turned into `[KEY-123](<url>KEY-123)`. Existing Markdown links,
/// inline code and bare URLs are copied unchanged. Linking twice changes nothing more.
pub fn linkify(text: &[u8], jira: &Jira) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        if let Some(n) = skip_at(text, i) {
            out.extend_from_slice(&text[i..i + n]);
            i += n;
        } else if let Some(n) = key_at(text, i, jira) {
            let key = &text[i..i + n];
            out.push(b'[');
            out.extend_from_slice(key);
            out.extend_from_slice(b"](");
            out.extend_from_slice(&jira.url);
            out.extend_from_slice(key);
            out.push(b')');
            i += n;
        } else {
            out.push(text[i]);
            i += 1;
        }
    }
    out
}

/// The Jira keys in `text`, for the `jira` array of `t list --json`: every key by the key test
/// alone, inside links and code too, once each, in order of first appearance.
pub fn keys_in(text: &[u8], jira: &Jira) -> Vec<Vec<u8>> {
    let mut found: Vec<Vec<u8>> = Vec::new();
    let mut i = 0;
    while i < text.len() {
        if let Some(n) = key_at(text, i, jira) {
            let key = text[i..i + n].to_vec();
            if !found.contains(&key) {
                found.push(key);
            }
            i += n;
        } else {
            i += 1;
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "https://jira.example.com/browse/";

    fn jira(keys: &str) -> Jira {
        let (j, w) = check_jira(keys.as_bytes(), URL.as_bytes());
        assert_eq!(w, None);
        j.unwrap()
    }

    fn l(key: &str) -> String {
        format!("[{key}]({URL}{key})")
    }

    fn link(text: &str, keys: &str) -> String {
        String::from_utf8(linkify(text.as_bytes(), &jira(keys))).unwrap()
    }

    #[test]
    fn links_bare_keys() {
        assert_eq!(
            link("Retest PROJ-1877 and OPS-12.", "PROJ OPS WEB"),
            format!("Retest {} and {}.", l("PROJ-1877"), l("OPS-12"))
        );
    }

    #[test]
    fn leaves_non_keys_code_urls_and_links_alone() {
        let plain = "Leave UTF-8, ISO-8601, ABC-12, xPROJ-4, PROJ-5a, `PROJ-3`, https://example.com/PROJ-2 and [see PROJ-6](https://example.com) alone";
        assert_eq!(link(plain, "PROJ OPS WEB"), plain);
        for t in [
            "a/PROJ-1",
            "a-PROJ-1",
            "PROJ-1-2",
            "PROJ-",
            "PROJ-x",
            "PROJ_1",
            "[](PROJ-1)",
        ] {
            assert_eq!(link(t, "PROJ"), t, "{t:?}");
        }
    }

    #[test]
    fn key_boundaries() {
        assert_eq!(link("(PROJ-1)", "PROJ"), format!("({})", l("PROJ-1")));
        assert_eq!(
            link("PROJ-1,PROJ-2", "PROJ"),
            format!("{},{}", l("PROJ-1"), l("PROJ-2"))
        );
        assert_eq!(link("é PROJ-1", "PROJ"), format!("é {}", l("PROJ-1")));
        // Non-ASCII bytes aren't word bytes, as for perl on bytes.
        assert_eq!(link("éPROJ-1", "PROJ"), format!("é{}", l("PROJ-1")));
        // A backquote without a partner is an ordinary byte.
        assert_eq!(link("` PROJ-1", "PROJ"), format!("` {}", l("PROJ-1")));
        // "http://" followed by white space isn't a URL.
        assert_eq!(
            link("http:// PROJ-1", "PROJ"),
            format!("http:// {}", l("PROJ-1"))
        );
    }

    #[test]
    fn keys_with_digits_underscores_and_prefixes() {
        assert_eq!(
            link("PROJ-1 P2_X-2 PROJX-3", "  PROJ   P2_X  "),
            format!("{} {} PROJX-3", l("PROJ-1"), l("P2_X-2"))
        );
        // One key a prefix of another: whichever matches.
        assert_eq!(
            link("PR-1 P-2", "P PR"),
            format!("{} {}", l("PR-1"), l("P-2"))
        );
    }

    #[test]
    fn linking_is_idempotent() {
        let once = link("Shipped WEB-7 and `WEB-8`", "WEB");
        assert_eq!(link(&once, "WEB"), once);
    }

    #[test]
    fn keys_for_json() {
        let j = jira("PROJ OPS");
        let text = format!(
            "Say {} again {}, and {} `OPS-2` http://x?id=OPS-3 a/PROJ-9",
            l("PROJ-7"),
            l("PROJ-7"),
            l("OPS-1")
        );
        let keys: Vec<String> = keys_in(text.as_bytes(), &j)
            .into_iter()
            .map(|k| String::from_utf8(k).unwrap())
            .collect();
        // Inside links and code and after = count; after / doesn't; each once, first order.
        assert_eq!(keys, ["PROJ-7", "OPS-1", "OPS-2", "OPS-3"]);
    }

    fn warning(keys: &str, url: &str) -> Option<String> {
        let (j, w) = check_jira(keys.as_bytes(), url.as_bytes());
        assert!(j.is_none());
        w
    }

    #[test]
    fn blank_keys_turn_linking_off_silently() {
        for keys in ["", " ", "\t\n"] {
            assert_eq!(warning(keys, URL), None, "{keys:?}");
        }
    }

    #[test]
    fn wrong_settings_warn() {
        assert_eq!(
            warning("PROJ OPS WEB", "https://x.example/browse").unwrap(),
            "TODO_JIRA_URL must start with http:// or https:// and end with / (got 'https://x.example/browse'); Jira linking is off"
        );
        assert_eq!(
            warning("PROJ", "x.example/browse/").unwrap(),
            "TODO_JIRA_URL must start with http:// or https:// and end with / (got 'x.example/browse/'); Jira linking is off"
        );
        assert_eq!(
            warning("PROJ", "").unwrap(),
            "TODO_JIRA_KEYS is set but TODO_JIRA_URL isn't; Jira linking is off"
        );
        assert_eq!(
            warning("PROJ,OPS WEB", URL).unwrap(),
            "TODO_JIRA_KEYS: 'PROJ,OPS' isn't a Jira project key (use uppercase keys separated by spaces); Jira linking is off"
        );
        assert_eq!(
            warning("proj A.B", URL).unwrap(),
            "TODO_JIRA_KEYS: 'proj', 'A.B' isn't a Jira project key (use uppercase keys separated by spaces); Jira linking is off"
        );
        // Keys are checked before the URL.
        assert!(
            warning("proj", "")
                .unwrap()
                .starts_with("TODO_JIRA_KEYS: 'proj'")
        );
    }

    #[test]
    fn browse_url_shape() {
        assert!(is_browse_url(b"https://a/"));
        assert!(is_browse_url(b"http://a/b/"));
        assert!(!is_browse_url(b"https:///"));
        assert!(!is_browse_url(b"https://"));
        assert!(!is_browse_url(b"ftp://a/"));
    }
}
