const CHARACTERS_TO_ESCAPE: [char; 18] = [
    '_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.', '!',
];

pub fn telegram_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());

    for c in text.chars() {
        if CHARACTERS_TO_ESCAPE.contains(&c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }

    escaped
}
