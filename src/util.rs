
pub fn modify_message(message: &mut String, n: usize) {
    const SUFFIX: [char; 4] = ['\u{e0000}', '\u{e0002}', '\u{e0003}', '\u{e0004}'];

    if n < SUFFIX.len() {
        message.push(SUFFIX[n]);
    } else {
        // in this case, we could use the power of combinatorics to append several
        // chars to message. 4^4 possible combinations should have us covered.
    }
}
