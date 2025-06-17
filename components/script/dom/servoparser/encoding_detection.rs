/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use encoding_rs::{Encoding, UTF_8, UTF_16BE, UTF_16LE, WINDOWS_1252, X_USER_DEFINED};

#[derive(Default)]
struct Attribute {
    name: Vec<u8>,
    value: Vec<u8>,
}

/// <https://html.spec.whatwg.org/multipage/parsing.html#prescan-a-byte-stream-to-determine-its-encoding>
pub(super) fn prescan_the_byte_stream_to_determine_the_encoding(
    byte_stream: &[u8],
) -> Option<&'static Encoding> {
    println!("sniffing: {:?}", String::from_utf8_lossy(byte_stream));
    // Step 1. Let position be a pointer to a byte in the input byte stream,
    // initially pointing at the first byte.
    let mut position = 0;

    // Step 2. Prescan for UTF-16 XML declarations: If position points to:
    match byte_stream {
        // A sequence of bytes starting with: 0x3C, 0x0, 0x3F, 0x0, 0x78, 0x0
        // (case-sensitive UTF-16 little-endian '<?x')
        [0x3C, 0x0, 0x3F, 0x0, 0x78, 0x0, ..] => {
            // Return UTF-16LE.
            return Some(UTF_16LE);
        },
        // A sequence of bytes starting with: 0x0, 0x3C, 0x0, 0x3F, 0x0, 0x78
        // (case-sensitive UTF-16 big-endian '<?x')
        [0x0, 0x3C, 0x0, 0x3F, 0x0, 0x78, ..] => {
            // Return UTF-16BE.
            return Some(UTF_16BE);
        },
        _ => {},
    }

    loop {
        // Step 3. Loop: If position points to:
        let remaining_byte_stream = byte_stream.get(position..)?;
        println!(
            "remaining byte stream: {:?}",
            String::from_utf8_lossy(remaining_byte_stream)
        );

        // A sequence of bytes starting with: 0x3C 0x21 0x2D 0x2D (`<!--`)
        if remaining_byte_stream.starts_with(b"<!--") {
            // Advance the position pointer so that it points at the first 0x3E byte which is preceded by two 0x2D bytes
            // (i.e. at the end of an ASCII '-->' sequence) and comes after the 0x3C byte that was found.
            // (The two 0x2D bytes can be the same as those in the '<!--' sequence.)
            // NOTE: This is not very efficient, but likely not an issue...
            position += remaining_byte_stream
                .windows(3)
                .position(|window| window == b"-->")?;
        }
        // A sequence of bytes starting with: 0x3C, 0x4D or 0x6D, 0x45 or 0x65, 0x54 or 0x74, 0x41 or 0x61,
        // and one of 0x09, 0x0A, 0x0C, 0x0D, 0x20, 0x2F (case-insensitive ASCII '<meta' followed by a space or slash)
        else if *remaining_byte_stream.first()? == b'<' &&
            matches!(remaining_byte_stream.get(1)?, b'm' | b'M') &&
            matches!(remaining_byte_stream.get(2)?, b'e' | b'E') &&
            matches!(remaining_byte_stream.get(3)?, b't' | b'T') &&
            matches!(remaining_byte_stream.get(4)?, b'a' | b'A') &&
            matches!(
                remaining_byte_stream.get(5)?,
                0x09 | 0x0A | 0x0C | 0x0D | 0x20 | 0x2F
            )
        {
            println!("got meta tag");
            // Step 1. Advance the position pointer so that it points at the next 0x09, 0x0A, 0x0C, 0x0D, 0x20,
            // or 0x2F byte (the one in sequence of characters matched above).
            position += 5;

            // Step 2. Let attribute list be an empty list of strings.
            // NOTE: This is used to track which attributes we have already seen. As there are only
            // three attributes that we care about, we instead use three booleans.
            let mut have_seen_http_equiv_attribute = false;
            let mut have_seen_content_attribute = false;
            let mut have_seen_charset_attribute = false;

            // Step 3. Let got pragma be false.
            let mut got_pragma = false;

            // Step 4. Let need pragma be null.
            let mut need_pragma = None;

            // Step 5. Let charset be the null value (which, for the purposes of this algorithm,
            // is distinct from an unrecognized encoding or the empty string).
            let mut charset = None;

            // Step 6. Attributes: Get an attribute and its value. If no attribute was sniffed,
            // then jump to the processing step below.
            println!("gimme attribute");
            while let Some(attribute) = get_an_attribute(byte_stream, &mut position) {
                // Step 7 If the attribute's name is already in attribute list,
                // then return to the step labeled attributes.
                // Step 8. Add the attribute's name to attribute list.
                // NOTE: This happens in the match arms below

                // Step 9. Run the appropriate step from the following list, if one applies:
                println!(
                    "{:?} {:?}",
                    String::from_utf8_lossy(&attribute.name),
                    String::from_utf8_lossy(&attribute.value)
                );
                match attribute.name.as_slice() {
                    b"http-equiv" => {
                        if have_seen_http_equiv_attribute {
                            continue;
                        }
                        have_seen_http_equiv_attribute = true;

                        // If the attribute's value is "content-type", then set got pragma to true.
                        if attribute.value == b"content-type" {
                            got_pragma = true;
                        }
                    },
                    b"content" => {
                        if have_seen_content_attribute {
                            continue;
                        }
                        have_seen_content_attribute = true;

                        // Apply the algorithm for extracting a character encoding from a meta element,
                        // giving the attribute's value as the string to parse. If a character encoding
                        // is returned, and if charset is still set to null, let charset be the encoding
                        // returned, and set need pragma to true.
                        if charset.is_none() {
                            if let Some(extracted_charset) =
                                extract_a_character_encoding_from_a_meta_element(&attribute.value)
                            {
                                need_pragma = Some(true);
                                charset = Some(extracted_charset);
                            }
                        }
                    },
                    // If the attribute's name is "charset"
                    b"charset" => {
                        if have_seen_charset_attribute {
                            continue;
                        }
                        have_seen_charset_attribute = true;

                        // Let charset be the result of getting an encoding from the attribute's value,
                        // and set need pragma to false.
                        if let Some(extracted_charset) = Encoding::for_label(&attribute.value) {
                            charset = Some(extracted_charset);
                        }
                        need_pragma = Some(false);
                    },
                    _ => {},
                }

                // Step 10. Return to the step labeled attributes.
            }

            // Step 11. Processing: If need pragma is null, then jump to the step below labeled next byte.
            if let Some(need_pragma) = need_pragma {
                // Step 12. If need pragma is true but got pragma is false,
                // then jump to the step below labeled next byte.
                if !need_pragma || got_pragma {
                    // Step 13. If charset is UTF-16BE/LE, then set charset to UTF-8.
                    if charset.is_some_and(|charset| charset == UTF_16BE || charset == UTF_16LE) {
                        charset = Some(UTF_8);
                    }
                    // Step 14. If charset is x-user-defined, then set charset to windows-1252.
                    else if charset.is_some_and(|charset| charset == X_USER_DEFINED) {
                        charset = Some(WINDOWS_1252);
                    }

                    // Step 15. Return charset.
                    return charset;
                }
            }
        }
        // A sequence of bytes starting with a 0x3C byte (<), optionally a 0x2F byte (/),
        // and finally a byte in the range 0x41-0x5A or 0x61-0x7A (A-Z or a-z)
        else if *remaining_byte_stream.first()? == b'<' &&
            remaining_byte_stream
                .get(1)
                .filter(|byte| **byte != b'=')
                .or(remaining_byte_stream.get(2))?
                .is_ascii_alphabetic()
        {
            // Step 1. Advance the position pointer so that it points at the next 0x09 (HT),
            // 0x0A (LF), 0x0C (FF), 0x0D (CR), 0x20 (SP), or 0x3E (>) byte.
            position += remaining_byte_stream
                .iter()
                .position(|byte| byte.is_ascii_whitespace() || *byte == b'>')?;

            // Step 2. Repeatedly get an attribute until no further attributes can be found,
            // then jump to the step below labeled next byte.
            while get_an_attribute(byte_stream, &mut position).is_some() {}
        }
        // A sequence of bytes starting with: 0x3C 0x21 (`<!`)
        // A sequence of bytes starting with: 0x3C 0x2F (`</`)
        // A sequence of bytes starting with: 0x3C 0x3F (`<?`)
        else if remaining_byte_stream.starts_with(b"<!") ||
            remaining_byte_stream.starts_with(b"</") ||
            remaining_byte_stream.starts_with(b"<?")
        {
            // Advance the position pointer so that it points at the first 0x3E byte (>) that comes after the 0x3C byte that was found.
            position += remaining_byte_stream
                .iter()
                .position(|byte| *byte == b'>')?;
        }
        // Any other byte
        else {
            // Do nothing with that byte.
        }

        // Next byte: Move position so it points at the next byte in the input byte stream,
        // and return to the step above labeled loop.
        position += 1;
    }
}

/// <https://html.spec.whatwg.org/multipage/#concept-get-attributes-when-sniffing>
fn get_an_attribute(input: &[u8], position: &mut usize) -> Option<Attribute> {
    // NOTE: If we reach the end of the input during parsing then we return "None"
    // (because there obviously is no attribute). The caller will then also run
    // out of bytes and invoke "get an xml encoding" as mandated by the spec.

    // Step 1. If the byte at position is one of 0x09 (HT), 0x0A (LF), 0x0C (FF), 0x0D (CR),
    // 0x20 (SP), or 0x2F (/), then advance position to the next byte and redo this step.
    *position += &input[*position..]
        .iter()
        .position(|b| !matches!(b, 0x09 | 0x0A | 0x0C | 0x0D | 0x20 | 0x2F))?;

    // Step 2. If the byte at position is 0x3E (>), then abort the get an attribute algorithm.
    // There isn't one.
    if input[*position] == 0x3E {
        return None;
    }

    // Step 3. Otherwise, the byte at position is the start of the attribute name.
    // Let attribute name and attribute value be the empty string.
    let mut attribute = Attribute::default();

    let mut have_spaces = false;
    loop {
        // Step 4. Process the byte at position as follows:
        match *input.get(*position)? {
            // If it is 0x3D (=), and the attribute name is longer than the empty string
            b'=' if !attribute.name.is_empty() => {
                // Advance position to the next byte and jump to the step below labeled value.
                *position += 1;
                break;
            },
            // If it is 0x09 (HT), 0x0A (LF), 0x0C (FF), 0x0D (CR), or 0x20 (SP)
            0x09 | 0x0A | 0x0C | 0x0D | 0x20 => {
                // Jump to the step below labeled spaces.
                have_spaces = true;
                break;
            },
            // If it is 0x2F (/) or 0x3E (>)
            b'/' | b'>' => {
                // Abort the get an attribute algorithm.
                // The attribute's name is the value of attribute name, its value is the empty string.
                return Some(attribute);
            },
            // If it is in the range 0x41 (A) to 0x5A (Z)
            byte @ (b'A'..=b'Z') => {
                // Append the code point b+0x20 to attribute name (where b is the value of the byte at position).
                // (This converts the input to lowercase.)
                attribute.name.push(byte + 0x20);
            },
            // Anything else
            byte => {
                // Append the code point with the same value as the byte at position to attribute name.
                // (It doesn't actually matter how bytes outside the ASCII range are handled here, since only
                // ASCII bytes can contribute to the detection of a character encoding.)
                attribute.name.push(byte);
            },
        }

        // Step 5. Advance position to the next byte and return to the previous step.
        *position += 1;
    }

    if have_spaces {
        // Step 6. Spaces: If the byte at position is one of 0x09 (HT), 0x0A (LF), 0x0C (FF), 0x0D (CR),
        // or 0x20 (SP), then advance position to the next byte, then, repeat this step.
        *position += &input[*position..]
            .iter()
            .position(|b| !b.is_ascii_whitespace())?;

        // Step 7. If the byte at position is not 0x3D (=), abort the get an attribute algorithm.
        // The attribute's name is the value of attribute name, its value is the empty string.
        if input[*position] != b'=' {
            return Some(attribute);
        }

        // Step 8. Advance position past the 0x3D (=) byte.
        *position += 1;
    }

    // Step 9. Value: If the byte at position is one of 0x09 (HT), 0x0A (LF), 0x0C (FF), 0x0D (CR), or 0x20 (SP),
    // then advance position to the next byte, then, repeat this step.
    *position += &input[*position..]
        .iter()
        .position(|b| !b.is_ascii_whitespace())?;

    // Step 10. Process the byte at position as follows:
    match input[*position] {
        // If it is 0x22 (") or 0x27 (')
        b @ (b'"' | b'\'') => {
            // Step 1. Let b be the value of the byte at position.
            // NOTE: We already have b.

            loop {
                // Step 2. Quote loop: Advance position to the next byte.
                *position += 1;

                // Step 3. If the value of the byte at position is the value of b, then advance position to the next byte
                // and abort the "get an attribute" algorithm. The attribute's name is the value of attribute name, and
                // its value is the value of attribute value.
                let byte_at_position = *input.get(*position)?;
                if byte_at_position == b {
                    *position += 1;
                    return Some(attribute);
                }
                // Step 4. Otherwise, if the value of the byte at position is in the range 0x41 (A) to 0x5A (Z),
                // then append a code point to attribute value whose value is 0x20 more than the value of the byte
                // at position.
                else if byte_at_position.is_ascii_uppercase() {
                    attribute.value.push(byte_at_position + 0x20);
                }
                // Step 5. Otherwise, append a code point to attribute value whose value is the same
                // as the value of the byte at position.
                else {
                    attribute.value.push(byte_at_position);
                }

                // Step 6. Return to the step above labeled quote loop.
            }
        },
        // If it is 0x3E (>)
        b'>' => {
            // Abort the get an attribute algorithm. The attribute's name is the value of attribute name,
            // its value is the empty string.
            return Some(attribute);
        },
        // If it is in the range 0x41 (A) to 0x5A (Z)
        b @ (b'A'..=b'Z') => {
            // Append a code point b+0x20 to attribute value (where b is the value of the byte at position).
            // Advance position to the next byte.
            attribute.value.push(b + 0x20);
            *position += 1;
        },
        // Anything else
        b => {
            // Append a code point with the same value as the byte at position to attribute value.
            // Advance position to the next byte.
            attribute.value.push(b);
            *position += 1
        },
    }

    loop {
        // Step 11. Process the byte at position as follows:
        match *input.get(*position)? {
            // If it is 0x09 (HT), 0x0A (LF), 0x0C (FF), 0x0D (CR), 0x20 (SP), or 0x3E (>)
            0x09 | 0x0A | 0x0C | 0x0D | 0x20 | 0x3E => {
                // Abort the get an attribute algorithm. The attribute's name is the value of attribute name and
                // its value is the value of attribute value.
                return Some(attribute);
            },
            // If it is in the range 0x41 (A) to 0x5A (Z)
            b if b.is_ascii_uppercase() => {
                // Append a code point b+0x20 to attribute value (where b is the value of the byte at position).
                attribute.value.push(b + 0x20);
            },
            // Anything else
            b => {
                // Append a code point with the same value as the byte at position to attribute value.
                attribute.value.push(b);
            },
        }

        // Step 12. Advance position to the next byte and return to the previous step.
        *position += 1;
    }
}

/// <https://html.spec.whatwg.org/multipage/#algorithm-for-extracting-a-character-encoding-from-a-meta-element>
fn extract_a_character_encoding_from_a_meta_element(input: &[u8]) -> Option<&'static Encoding> {
    // Step 1. Let position be a pointer into s, initially pointing at the start of the string.
    let mut position = 0;

    loop {
        // Step 2. Loop: Find the first seven characters in s after position that are an ASCII case-insensitive
        // match for the word "charset". If no such match is found, return nothing.
        // NOTE: In our case, the attribute value always comes from "get_an_attribute" and is already lowercased.
        position += input[position..]
            .windows(7)
            .position(|window| window == b"charset")?;

        // Step 3. Skip any ASCII whitespace that immediately follow the word "charset" (there might not be any).
        position += &input[position + 7..]
            .iter()
            .position(|byte| !byte.is_ascii_whitespace())?;

        // Step 4. If the next character is not a U+003D EQUALS SIGN (=), then move position to point just before
        // that next character, and jump back to the step labeled loop.
        if *input.get(position)? != b'=' {
            position -= 1;
        } else {
            break;
        }
    }

    // Step 5. Skip any ASCII whitespace that immediately follow the equals sign (there might not be any).
    position += &input[position..]
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())?;

    // Step 6. Process the next character as follows:
    let next_character = input.get(position)?;
    // If it is a U+0022 QUOTATION MARK character (") and there is a later U+0022 QUOTATION MARK character (") in s
    // If it is a U+0027 APOSTROPHE character (') and there is a later U+0027 APOSTROPHE character (') in s
    if matches!(*next_character, b'"' | b'\'') {
        // Return the result of getting an encoding from the substring that is between
        // this character and the next earliest occurrence of this character.
        let remaining = input.get(position + 1..)?;
        let end = remaining.iter().position(|byte| byte == next_character)?;

        Encoding::for_label(&remaining[..end])
    }
    // If it is an unmatched U+0022 QUOTATION MARK character (")
    // If it is an unmatched U+0027 APOSTROPHE character (')
    // If there is no next character
    // NOTE: All of these cases are already covered above

    // Otherwise
    else {
        // Return the result of getting an encoding from the substring that consists of this character up
        // to but not including the first ASCII whitespace or U+003B SEMICOLON character (;), or the end of s,
        // whichever comes first.
        let remaining = input.get(position..)?;
        let end = remaining
            .iter()
            .position(|byte| byte.is_ascii_whitespace() || *byte == b';')
            .unwrap_or(remaining.len());
        Encoding::for_label(&remaining[..end])
    }
}

/// <https://html.spec.whatwg.org/multipage/#concept-get-xml-encoding-when-sniffing>
pub(super) fn get_xml_encoding(input: &[u8]) -> Option<&'static Encoding> {
    // Step 1. Let encodingPosition be a pointer to the start of the stream.
    // NOTE: We don't need this variable yet.

    // Step 2. If encodingPosition does not point to the start of a byte sequence 0x3C, 0x3F, 0x78,
    // 0x6D, 0x6C (`<?xml`), then return failure.
    if !input.starts_with(b"<?xml") {
        return None;
    }

    // Step 3. Let xmlDeclarationEnd be a pointer to the next byte in the input byte stream which is 0x3E (>).
    // If there is no such byte, then return failure.
    // NOTE: The spec does not use this variable but the intention is clear.
    let xml_declaration_end = input.iter().position(|byte| *byte == b'>')?;
    let input = &input[..xml_declaration_end];

    // Step 4. Set encodingPosition to the position of the first occurrence of the subsequence of bytes 0x65, 0x6E,
    // 0x63, 0x6F, 0x64, 0x69, 0x6E, 0x67 (`encoding`) at or after the current encodingPosition. If there is no
    // such sequence, then return failure.
    let mut encoding_position = input
        .windows(b"encoding".len())
        .position(|window| window == b"encoding")?;

    // Step 5. Advance encodingPosition past the 0x67 (g) byte.
    encoding_position += b"encoding".len();

    // Step 6. While the byte at encodingPosition is less than or equal to 0x20 (i.e., it is either an
    // ASCII space or control character), advance encodingPosition to the next byte.
    while *input.get(encoding_position)? <= 0x20 {
        encoding_position += 1;
    }

    // Step 7. If the byte at encodingPosition is not 0x3D (=), then return failure.
    if *input.get(encoding_position)? != b'=' {
        return None;
    }

    // Step 8. Advance encodingPosition to the next byte.
    encoding_position += 1;

    // Step 9. While the byte at encodingPosition is less than or equal to 0x20 (i.e., it is either an
    // ASCII space or control character), advance encodingPosition to the next byte.
    while *input.get(encoding_position)? <= 0x20 {
        encoding_position += 1;
    }

    // Step 10. Let quoteMark be the byte at encodingPosition.
    let quote_mark = *input.get(encoding_position)?;

    // Step 11. If quoteMark is not either 0x22 (") or 0x27 ('), then return failure.
    if !matches!(quote_mark, b'"' | b'\'') {
        return None;
    }

    // Step 12. Advance encodingPosition to the next byte.
    encoding_position += 1;

    // Step 13. Let encodingEndPosition be the position of the next occurrence of quoteMark at or after
    // encodingPosition. If quoteMark does not occur again, then return failure.
    let encoding_end_position = input[encoding_position..]
        .iter()
        .position(|byte| *byte == quote_mark)?;

    // Step 14. Let potentialEncoding be the sequence of the bytes between encodingPosition
    // (inclusive) and encodingEndPosition (exclusive).
    let potential_encoding = &input[encoding_position..][..encoding_end_position];

    // Step 15. If potentialEncoding contains one or more bytes whose byte value is 0x20 or below,
    // then return failure.
    if potential_encoding.iter().any(|byte| *byte <= 0x20) {
        return None;
    }

    // Step 16. Let encoding be the result of getting an encoding given potentialEncoding isomorphic decoded.
    let encoding = Encoding::for_label(potential_encoding)?;

    // Step 17. If the encoding is UTF-16BE/LE, then change it to UTF-8.
    // Step 18. Return encoding.
    if encoding == UTF_16BE || encoding == UTF_16LE {
        Some(UTF_8)
    } else {
        Some(encoding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_encoding_with_xml_declaration() {
        assert_eq!(
            prescan_the_byte_stream_to_determine_the_encoding(&[
                0x3C, 0x0, 0x3F, 0x0, 0x78, 0x0, 0x42
            ]),
            Some(UTF_16LE)
        );
        assert_eq!(
            prescan_the_byte_stream_to_determine_the_encoding(&[
                0x0, 0x3C, 0x0, 0x3F, 0x0, 0x78, 0x42
            ]),
            Some(UTF_16BE)
        );
    }

    #[test]
    fn meta_charset_within_comment() {
        assert_eq!(
            prescan_the_byte_stream_to_determine_the_encoding(b"<!-- <meta charset='utf8'> -->"),
            None
        );
    }

    #[test]
    fn meta_charset_with_preceding_comment() {
        assert_eq!(
            prescan_the_byte_stream_to_determine_the_encoding(b"<!-- --> <meta charset='utf8'>"),
            Some(UTF_8)
        );
        assert_eq!(
            prescan_the_byte_stream_to_determine_the_encoding(b"<!--> <meta charset='utf8'>"),
            Some(UTF_8)
        );
    }

    #[test]
    fn xml_encoding_invalid_start() {
        assert_eq!(get_xml_encoding(b"<?xmX encoding='UTF8'>"), None);
    }

    #[test]
    fn xml_encoding_outside_of_declaration() {
        assert_eq!(get_xml_encoding(b"<?xml> encoding='UTF8'"), None);
    }

    #[test]
    fn xml_encoding_missing_quotes() {
        // Missing opening quote
        assert_eq!(get_xml_encoding(b"<?xml encoding=UTF8'>"), None);

        // Missing closing quote
        assert_eq!(get_xml_encoding(b"<?xml encoding='UTF8>"), None);
    }

    #[test]
    fn xml_encoding_containing_whitespace_within_quotes() {
        assert_eq!(get_xml_encoding(b"<?xml encoding=' UTF8'>"), None);
    }

    #[test]
    fn xml_encoding_single_quotes() {
        assert_eq!(get_xml_encoding(b"<?xml encoding='UTF8'>"), Some(UTF_8));
    }

    #[test]
    fn xml_encoding_double_quotes() {
        assert_eq!(get_xml_encoding(b"<?xml encoding=\"UTF8\">"), Some(UTF_8));
    }

    #[test]
    fn xml_encoding_with_whitespace_around_equal_sign() {
        assert_eq!(
            get_xml_encoding(b"<?xml encoding \x00 =  \x00 \"UTF8\">"),
            Some(UTF_8)
        );
    }
}
