use crate::{Error, Result};
use std::borrow::Cow;
use std::str::Chars;

/// Takes in a string with backslash escapes written out with literal backslash characters and
/// converts it to a string with the proper escaped characters.
///
/// ## Errors
///
/// Returns an error if an invalid or incomplete escape sequence or unicode code point is
/// encountered.
pub fn unescape(s: &str) -> Result<Cow<str>> {
    for (idx, ch) in s.chars().enumerate() {
        if ch == '\\' {
            // At least one char needs unescaping so we need to return a new `String` instead of a
            // borrowed `&str`.
            return unescape_owned(s, idx).map(Cow::Owned);
        }
    }

    Ok(Cow::Borrowed(s))
}

fn unescape_owned(s: &str, idx: usize) -> Result<String> {
    let mut buf = String::with_capacity(s.len());

    // Put all preceeding chars into buf already.
    buf.push_str(&s[..idx]);

    let mut chars = s[idx..].chars();
    let mut scratch = String::new();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            buf.push(ch);
            continue;
        }

        let ch = match chars.next() {
            Some('b') => '\u{0008}',
            Some('f') => '\u{000C}',
            Some('n') => '\n',
            Some('r') => '\r',
            Some('t') => '\t',
            Some('\'') => '\'',
            Some('\"') => '\"',
            Some('\\') => '\\',
            Some('u') => match unescape_unicode(&mut chars, &mut scratch) {
                Some(ch) => ch,
                None => return Err(Error::InvalidUnicodeCodePoint(scratch)),
            },
            Some(ch) => return Err(Error::InvalidEscape(ch)),
            None => return Err(Error::Eof),
        };

        buf.push(ch);
    }

    Ok(buf)
}

fn unescape_unicode(chars: &mut Chars<'_>, scratch: &mut String) -> Option<char> {
    scratch.clear();

    for _ in 0..4 {
        scratch.push(chars.next()?);
    }

    char::from_u32(u32::from_str_radix(scratch, 16).ok()?)
}

/// Like [`unescape`], but returns the original `&str` if it contains invalid escape sequences
/// instead of failing.
pub fn try_unescape(s: &str) -> Cow<str> {
    match unescape(s) {
        Ok(s) => s,
        Err(_) => Cow::Borrowed(s),
    }
}

/// Scan `s` for sequences that introduce a template interpolation or directive. Returns `true`
/// once it found one of these start markers, `false` otherwise.
///
/// This function only looks for start markers and does not check if the template is actually
/// valid.
#[inline]
pub fn is_templated(s: &str) -> bool {
    if s.len() < 3 {
        return false;
    }

    let mut skip_next = false;

    // Because calling `s.contains("${")` would also match escaped interpolations (`$${`) a
    // window iterator is used here to detect and ignore these. The same applies to escaped
    // directives.
    for window in s.as_bytes().windows(3) {
        if skip_next {
            skip_next = false;
            continue;
        }

        match window {
            [b'$', b'$', b'{'] | [b'%', b'%', b'{'] => {
                // The next window would incorrectly match the next arm, so it must be
                // skipped.
                skip_next = true;
            }
            [b'$' | b'%', b'{', _] => return true,
            _ => {}
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_templated() {
        assert!(is_templated("${a}"));
        assert!(is_templated("${\"a\"}"));
        assert!(is_templated("%{ if foo }foo%{ else }bar%{ endif }"));
        assert!(is_templated("$${ introduces an ${\"interpolation\"}"));
        assert!(!is_templated(
            "escaped directive %%{ if foo }foo%%{ else }bar%%{ endif }"
        ));
        assert!(!is_templated("escaped interpolation $${a}"));
    }
}
