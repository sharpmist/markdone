//! Text helpers shared by the commands: web links, terminal styling, JSON strings, case folding,
//! padding and shell quoting. Pure functions over bytes, except `styled_stdout`, which asks the
//! terminal. See docs/designs/rust_port.md, "Jira linking and display" and "List names".

use std::io::IsTerminal;
use std::ops::Range;

/// ASCII white space as perl's `\s` sees bytes: space, tab, LF, VT, FF, CR.
pub fn is_ascii_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// A web link starting at `s[i]`: `[text](http://target)` or `https://`, the text not empty and
/// without `]`, the target at least one byte after `//`, none of them `)` or ASCII white space.
/// Returns the text's range, the target's range (including the scheme) and the index after `)`.
fn web_link_at(s: &[u8], i: usize) -> Option<(Range<usize>, Range<usize>, usize)> {
    if s.get(i) != Some(&b'[') {
        return None;
    }
    let close = i + 1 + s[i + 1..].iter().position(|&b| b == b']')?;
    if close == i + 1 || s.get(close + 1) != Some(&b'(') {
        return None;
    }
    let target = close + 2;
    let rest = &s[target..];
    let scheme = if rest.starts_with(b"https://") {
        8
    } else if rest.starts_with(b"http://") {
        7
    } else {
        return None;
    };
    let after_scheme = target + scheme;
    let mut j = after_scheme;
    while j < s.len() && s[j] != b')' && !is_ascii_space(s[j]) {
        j += 1;
    }
    if j == after_scheme || s.get(j) != Some(&b')') {
        return None;
    }
    Some((i + 1..close, target..j, j + 1))
}

/// `s` with each web link, left to right and without overlaps, replaced by `f(text, target)`.
fn replace_web_links(s: &[u8], mut f: impl FnMut(&[u8], &[u8], &mut Vec<u8>)) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if let Some((text, target, end)) = web_link_at(s, i) {
            f(&s[text], &s[target], &mut out);
            i = end;
        } else {
            out.push(s[i]);
            i += 1;
        }
    }
    out
}

/// `s` as a terminal shows it: each web link reduced to its text. Used to match `<item>` text and
/// for Tab completion (0.3.0's awk `shown`).
pub fn shown(s: &[u8]) -> Vec<u8> {
    replace_web_links(s, |text, _, out| out.extend_from_slice(text))
}

/// `s` for output: when `styled`, each web link becomes an OSC 8 hyperlink showing its text;
/// otherwise `s` unchanged (0.3.0's `render`).
pub fn render(s: &[u8], styled: bool) -> Vec<u8> {
    if !styled {
        return s.to_vec();
    }
    replace_web_links(s, |text, target, out| {
        out.extend_from_slice(b"\x1b]8;;");
        out.extend_from_slice(target);
        out.extend_from_slice(b"\x1b\\");
        out.extend_from_slice(text);
        out.extend_from_slice(b"\x1b]8;;\x1b\\");
    })
}

/// `name` in bold, for the list-name line above `t list` in styled output.
pub fn bold(name: &[u8]) -> Vec<u8> {
    [b"\x1b[1m", name, b"\x1b[0m"].concat()
}

/// Whether output to stdout is styled: stdout is a terminal and, on Windows, the console accepts
/// escape sequences once asked to.
pub fn styled_stdout() -> bool {
    std::io::stdout().is_terminal() && enable_escape_sequences()
}

#[cfg(unix)]
fn enable_escape_sequences() -> bool {
    true
}

#[cfg(windows)]
fn enable_escape_sequences() -> bool {
    use windows_sys::Win32::System::Console::{
        ENABLE_VIRTUAL_TERMINAL_PROCESSING, GetConsoleMode, GetStdHandle, STD_OUTPUT_HANDLE,
        SetConsoleMode,
    };
    // SAFETY: Win32 calls on this process's own stdout handle; `mode` is a local the call fills.
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut mode = 0;
        if GetConsoleMode(handle, &mut mode) == 0 {
            return false;
        }
        SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING) != 0
    }
}

/// `s` as a JSON string: invalid UTF-8 becomes U+FFFD; `"`, `\` and U+0000 to U+001F are
/// escaped, the control characters as `\u00xx` in lowercase hex.
pub fn json_string(s: &[u8]) -> String {
    let text = String::from_utf8_lossy(s);
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `s` lower-cased for case-insensitive matching of item text and list names: invalid UTF-8
/// becomes U+FFFD, then Unicode lower-casing.
pub fn lower(s: &[u8]) -> String {
    String::from_utf8_lossy(s).to_lowercase()
}

/// The number of characters in `s`, counting each invalid UTF-8 sequence as one.
pub fn char_count(s: &[u8]) -> usize {
    String::from_utf8_lossy(s).chars().count()
}

/// `s` followed by spaces up to `width` characters, like printf's `%-<width>s` counting
/// characters (as gawk and zsh do).
pub fn pad(s: &[u8], width: usize) -> Vec<u8> {
    let mut out = s.to_vec();
    out.resize(s.len() + width.saturating_sub(char_count(s)), b' ');
    out
}

/// Bytes that make zsh's `(q-)` put a stretch of a name in single quotes, anywhere in it.
fn needs_quotes(b: u8) -> bool {
    b < 0x20
        || b == 0x7f
        || matches!(
            b,
            b' ' | b'"'
                | b'#'
                | b'$'
                | b'&'
                | b'('
                | b')'
                | b'*'
                | b';'
                | b'<'
                | b'>'
                | b'?'
                | b'['
                | b'\\'
                | b']'
                | b'^'
                | b'`'
                | b'{'
                | b'|'
                | b'}'
        )
}

/// `name` quoted for a shell as zsh's `${(q-)name}` does: the stretches between single quotes are
/// each wrapped in single quotes when they hold a special character (or, at the very start of
/// the name, `=` or `~`), else left bare; each single quote is written as `\'`; an empty name is
/// `''`. Used in `t complete` and the `t lists add` message.
pub fn shell_quote(name: &[u8]) -> Vec<u8> {
    if name.is_empty() {
        return b"''".to_vec();
    }
    let mut out = Vec::with_capacity(name.len() + 2);
    for (k, stretch) in name.split(|&b| b == b'\'').enumerate() {
        if k > 0 {
            out.extend_from_slice(b"\\'");
        }
        if stretch.is_empty() {
            continue;
        }
        let special = stretch.iter().any(|&b| needs_quotes(b))
            || (k == 0 && matches!(stretch[0], b'=' | b'~'));
        if special {
            out.push(b'\'');
            out.extend_from_slice(stretch);
            out.push(b'\'');
        } else {
            out.extend_from_slice(stretch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(b: Vec<u8>) -> String {
        String::from_utf8(b).unwrap()
    }

    #[test]
    fn shown_reduces_web_links_to_their_text() {
        let t = b"Fix [PROJ-1](https://jira.example.com/browse/PROJ-1) now";
        assert_eq!(s(shown(t)), "Fix PROJ-1 now");
        assert_eq!(s(shown(b"[a](http://x) and [b](https://y/z)")), "a and b");
    }

    #[test]
    fn not_web_links() {
        for t in [
            "[](https://x)",       // empty text
            "[a](ftp://x)",        // not http
            "[a](https://)",       // nothing after //
            "[a](https://x y)",    // space in the target
            "[a](https://x",       // no )
            "[a] (https://x)",     // space before (
            "[a](https://x\ty)",   // tab
            "[a](https://x\x0by)", // VT
            "plain text",
        ] {
            assert_eq!(s(shown(t.as_bytes())), t, "{t:?}");
            assert_eq!(s(render(t.as_bytes(), true)), t, "{t:?}");
        }
    }

    #[test]
    fn the_leftmost_bracket_starts_the_link() {
        // As the awk and perl patterns match: the text runs from the first [ to the first ].
        assert_eq!(s(shown(b"[a [b](https://x)")), "a [b");
        assert_eq!(s(shown(b"[a] [b](https://x)")), "[a] b");
    }

    #[test]
    fn render_writes_osc8_only_when_styled() {
        let t = b"See [docs](https://example.com/d) ok";
        assert_eq!(render(t, false), t.to_vec());
        assert_eq!(
            render(t, true),
            b"See \x1b]8;;https://example.com/d\x1b\\docs\x1b]8;;\x1b\\ ok".to_vec()
        );
    }

    #[test]
    fn bold_wraps_the_name() {
        assert_eq!(bold(b"Work"), b"\x1b[1mWork\x1b[0m".to_vec());
    }

    #[test]
    fn json_strings() {
        assert_eq!(json_string(b"plain"), r#""plain""#);
        assert_eq!(json_string(br#"a "q" \ b"#), r#""a \"q\" \\ b""#);
        assert_eq!(json_string(b"t\tn\ne\x1b"), r#""t\u0009n\u000ae\u001b""#);
        assert_eq!(json_string(b"bad \xff byte"), "\"bad \u{fffd} byte\"");
        assert_eq!(json_string("Café ✓".as_bytes()), "\"Café ✓\"");
        assert_eq!(json_string(b"\x7f"), "\"\x7f\"");
    }

    #[test]
    fn lower_and_count() {
        assert_eq!(lower("ÅSA Fix".as_bytes()), "åsa fix");
        assert_eq!(char_count("Café".as_bytes()), 4);
        assert_eq!(char_count(b"a\xffb"), 3);
    }

    #[test]
    fn pad_counts_characters() {
        assert_eq!(pad(b"Today", 6), b"Today ".to_vec());
        assert_eq!(pad(b"Done", 6), b"Done  ".to_vec());
        assert_eq!(pad("Idéer".as_bytes(), 6), "Idéer ".as_bytes().to_vec());
        assert_eq!(pad(b"LongerName", 6), b"LongerName".to_vec());
        assert_eq!(pad(b"", 6), b"      ".to_vec());
    }

    /// Vectors produced with zsh 5.9: `print -r -- ${(q-)name}`.
    #[test]
    fn shell_quote_matches_zsh_q_minus() {
        let cases: &[(&str, &str)] = &[
            ("Work", "Work"),
            ("my list", "'my list'"),
            ("it's", r"it\'s"),
            ("it's my", r"it\''s my'"),
            ("'", r"\'"),
            ("a b'c", r"'a b'\'c"),
            ("", "''"),
            ("x'", r"x\'"),
            ("x''y", r"x\'\'y"),
            ("'a b", r"\''a b'"),
            ("a=b c", "'a=b c'"),
            ("é", "é"),
            ("Café", "Café"),
            ("a\tb", "'a\tb'"),
            ("a\nb", "'a\nb'"),
            ("a~b", "a~b"),
            ("x#", "'x#'"),
            ("!x", "!x"),
            ("a!b", "a!b"),
            ("a'=b", r"a\'=b"),
            ("a'~b", r"a\'~b"),
            ("a'#b", r"a\''#b'"),
            ("=", "'='"),
            ("~", "'~'"),
            ("#", "'#'"),
            ("a.b-c_d", "a.b-c_d"),
        ];
        for (name, want) in cases {
            assert_eq!(s(shell_quote(name.as_bytes())), *want, "{name:?}");
        }
        // Every ASCII punctuation character, in the middle (a?b) and at the start (?x).
        let punct: &[(char, &str, &str)] = &[
            ('!', "a!b", "!x"),
            ('"', "'a\"b'", "'\"x'"),
            ('#', "'a#b'", "'#x'"),
            ('$', "'a$b'", "'$x'"),
            ('%', "a%b", "%x"),
            ('&', "'a&b'", "'&x'"),
            ('\'', r"a\'b", r"\'x"),
            ('(', "'a(b'", "'(x'"),
            (')', "'a)b'", "')x'"),
            ('*', "'a*b'", "'*x'"),
            ('+', "a+b", "+x"),
            (',', "a,b", ",x"),
            ('-', "a-b", "-x"),
            ('.', "a.b", ".x"),
            ('/', "a/b", "/x"),
            (':', "a:b", ":x"),
            (';', "'a;b'", "';x'"),
            ('<', "'a<b'", "'<x'"),
            ('=', "a=b", "'=x'"),
            ('>', "'a>b'", "'>x'"),
            ('?', "'a?b'", "'?x'"),
            ('@', "a@b", "@x"),
            ('[', "'a[b'", "'[x'"),
            ('\\', r"'a\b'", r"'\x'"),
            (']', "'a]b'", "']x'"),
            ('^', "'a^b'", "'^x'"),
            ('_', "a_b", "_x"),
            ('`', "'a`b'", "'`x'"),
            ('{', "'a{b'", "'{x'"),
            ('|', "'a|b'", "'|x'"),
            ('}', "'a}b'", "'}x'"),
            ('~', "a~b", "'~x'"),
        ];
        for (c, mid, start) in punct {
            assert_eq!(s(shell_quote(format!("a{c}b").as_bytes())), *mid, "a{c}b");
            assert_eq!(s(shell_quote(format!("{c}x").as_bytes())), *start, "{c}x");
        }
    }
}
