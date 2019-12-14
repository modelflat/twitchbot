/// Modifies message in such way that it is visually remains the same,
/// but Twitch IRC does not consider message a duplicate.
///
/// The implementation relies on
/// [unicode block named Tags](https://en.wikipedia.org/wiki/Tags_(Unicode_block)).
///
/// Tags are not classified as Whitespace, and therefore they are not
/// stripped from the message by the Twitch IRC server. Only reserved characters from
/// this block are used.
///
/// This function guarantees that modification will result in exactly
/// 1 character and no more than 4 bytes increase in message size per call.
///
/// To allow user to use the power of Language tags more efficiently, this
/// function accepts the second parameter which determines the character
/// to be used for message modification.
pub fn modify_message(message: &mut String, salt: usize) {
    const SUFFIX: [char; 31] = [
        '\u{e0000}' /*   e0001*/, '\u{e0002}', '\u{e0003}',
        '\u{e0004}', '\u{e0005}', '\u{e0006}', '\u{e0007}',
        '\u{e0008}', '\u{e0009}', '\u{e000a}', '\u{e000b}',
        '\u{e000c}', '\u{e000d}', '\u{e000e}', '\u{e000f}',
        '\u{e0010}', '\u{e0011}', '\u{e0012}', '\u{e0013}',
        '\u{e0014}', '\u{e0015}', '\u{e0016}', '\u{e0017}',
        '\u{e0018}', '\u{e0019}', '\u{e001a}', '\u{e001b}',
        '\u{e001c}', '\u{e001d}', '\u{e001e}', '\u{e001f}',
    ];

    message.push(SUFFIX[salt % SUFFIX.len()]);
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_modify_message_modifies_message_by_exactly_1_char() {
        let mut message = "message".to_string();
        let original_len = message.chars().count();

        modify_message(&mut message, 0);
        assert_eq!(message.chars().count(), original_len + 1);
        modify_message(&mut message, 1);
        assert_eq!(message.chars().count(), original_len + 2);
        modify_message(&mut message, 123123123);
        assert_eq!(message.chars().count(), original_len + 3);
    }

    #[test]
    fn test_modify_message_modifies_message_by_no_more_than_4_bytes() {
        let mut message = "message".to_string();
        let original_len = message.len();

        modify_message(&mut message, 0);
        assert!(message.len() <= original_len + 4);
        modify_message(&mut message, 1);
        assert!(message.len() <= original_len + 8);
        modify_message(&mut message, 123123123);
        assert!(message.len() <= original_len + 12);
    }

}
