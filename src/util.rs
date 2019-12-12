
pub fn modify_message(message: &mut String, n: usize) {
    const SUFFIX: [char; 8] = [
        '\u{e0000}', '\u{e0002}', '\u{e0003}', '\u{e0004}',
        '\u{e0005}', '\u{e0006}', '\u{e0007}', '\u{e0008}',
    ];

    if n < SUFFIX.len() {
        message.push(SUFFIX[n]);
    }

}
