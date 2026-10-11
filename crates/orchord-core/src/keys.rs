//! Typing into the Mac from outside: which keys of a US keyboard make a character, and
//! sending whole lines of text one key at a time.

use std::sync::mpsc::Sender;
use std::time::Duration;

use crate::supervisor::VmCommand;

/// The pause after asking for a key, which itself takes about a tenth of a second to press.
/// Asking faster would only queue keys up in the supervisor.
const KEY_GAP: Duration = Duration::from_millis(150);

/// Presses a chord of keys, given as QEMU key names, and waits for it to be taken.
pub fn press(commands: &Sender<VmCommand>, keys: &[&str]) {
    let _ = commands.send(VmCommand::SendKeys(keys.iter().map(|key| key.to_string()).collect()));
    std::thread::sleep(KEY_GAP);
}

/// Types `text` on the Mac's keyboard. Blocks for about a fifth of a second per character;
/// characters a US keyboard cannot type are left out.
pub fn type_text(commands: &Sender<VmCommand>, text: &str) {
    for character in text.chars() {
        if let Some(keys) = keys_for(character) {
            press(commands, &keys);
        }
    }
}

/// The keys of a US keyboard that type `character`, as QEMU key names.
pub fn keys_for(character: char) -> Option<Vec<&'static str>> {
    const LETTERS: [&str; 26] = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z"];
    const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
    let plain = |key: &'static str| Some(vec![key]);
    let shifted = |key: &'static str| Some(vec!["shift", key]);
    match character {
        'a'..='z' => plain(LETTERS[character as usize - 'a' as usize]),
        'A'..='Z' => shifted(LETTERS[character as usize - 'A' as usize]),
        '0'..='9' => plain(DIGITS[character as usize - '0' as usize]),
        ' ' => plain("spc"),
        '-' => plain("minus"),
        '=' => plain("equal"),
        '[' => plain("bracket_left"),
        ']' => plain("bracket_right"),
        '\\' => plain("backslash"),
        ';' => plain("semicolon"),
        '\'' => plain("apostrophe"),
        ',' => plain("comma"),
        '.' => plain("dot"),
        '/' => plain("slash"),
        '`' => plain("grave_accent"),
        '!' => shifted("1"),
        '@' => shifted("2"),
        '#' => shifted("3"),
        '$' => shifted("4"),
        '%' => shifted("5"),
        '^' => shifted("6"),
        '&' => shifted("7"),
        '*' => shifted("8"),
        '(' => shifted("9"),
        ')' => shifted("0"),
        '_' => shifted("minus"),
        '+' => shifted("equal"),
        '{' => shifted("bracket_left"),
        '}' => shifted("bracket_right"),
        '|' => shifted("backslash"),
        ':' => shifted("semicolon"),
        '"' => shifted("apostrophe"),
        '<' => shifted("comma"),
        '>' => shifted("dot"),
        '?' => shifted("slash"),
        '~' => shifted("grave_accent"),
        _ => None,
    }
}
