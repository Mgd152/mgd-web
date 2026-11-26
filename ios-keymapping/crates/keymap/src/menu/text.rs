//! Single-line text field model (profile name, control label).
//!
//! Editing works on `char`s: a precomposed letter is one char, a letter plus
//! combining marks is several, and backspace removes one char at a time.

#[derive(Clone, PartialEq, Debug)]
pub struct TextEdit {
    text: String,
    /// Char index, 0..=len.
    caret: usize,
    /// Other end of the selection, when there is one.
    anchor: Option<usize>,
    max_chars: usize,
}

fn clean(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

impl TextEdit {
    pub fn new(text: &str, max_chars: usize) -> TextEdit {
        let text: String = clean(text).chars().take(max_chars).collect();
        let caret = text.chars().count();
        TextEdit {
            text,
            caret,
            anchor: None,
            max_chars,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    /// `(start, end)` char indices, start < end.
    pub fn selection(&self) -> Option<(usize, usize)> {
        let a = self.anchor?;
        (a != self.caret).then(|| (a.min(self.caret), a.max(self.caret)))
    }

    fn len(&self) -> usize {
        self.text.chars().count()
    }

    fn byte(&self, char_idx: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_idx)
            .map_or(self.text.len(), |(b, _)| b)
    }

    fn remove_range(&mut self, a: usize, b: usize) {
        let (ba, bb) = (self.byte(a), self.byte(b));
        self.text.replace_range(ba..bb, "");
        self.caret = a;
        self.anchor = None;
    }

    fn delete_selection(&mut self) -> bool {
        match self.selection() {
            Some((a, b)) => {
                self.remove_range(a, b);
                true
            }
            None => {
                self.anchor = None;
                false
            }
        }
    }

    /// Replaces the selection; control characters are dropped and the text
    /// is cut at `max_chars`.
    pub fn insert(&mut self, s: &str) {
        self.delete_selection();
        let room = self.max_chars.saturating_sub(self.len());
        let add: String = clean(s).chars().take(room).collect();
        let at = self.byte(self.caret);
        self.text.insert_str(at, &add);
        self.caret += add.chars().count();
    }

    pub fn backspace(&mut self) {
        if !self.delete_selection() && self.caret > 0 {
            self.remove_range(self.caret - 1, self.caret);
        }
    }

    pub fn delete(&mut self) {
        if !self.delete_selection() && self.caret < self.len() {
            self.remove_range(self.caret, self.caret + 1);
        }
    }

    fn move_to(&mut self, to: usize, shift: bool) {
        if shift {
            self.anchor.get_or_insert(self.caret);
        } else {
            self.anchor = None;
        }
        self.caret = to;
    }

    pub fn left(&mut self, shift: bool) {
        if !shift {
            if let Some((a, _)) = self.selection() {
                self.move_to(a, false);
                return;
            }
        }
        self.move_to(self.caret.saturating_sub(1), shift);
    }

    pub fn right(&mut self, shift: bool) {
        if !shift {
            if let Some((_, b)) = self.selection() {
                self.move_to(b, false);
                return;
            }
        }
        self.move_to((self.caret + 1).min(self.len()), shift);
    }

    pub fn home(&mut self, shift: bool) {
        self.move_to(0, shift);
    }

    pub fn end(&mut self, shift: bool) {
        self.move_to(self.len(), shift);
    }

    pub fn select_all(&mut self) {
        self.anchor = Some(0);
        self.caret = self.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // test 41
    #[test]
    fn insert_backspace_delete_precomposed() {
        let mut t = TextEdit::new("", 40);
        t.insert("Nạp đạn");
        assert_eq!(t.text(), "Nạp đạn");
        assert_eq!(t.caret(), 7);
        t.backspace();
        assert_eq!(t.text(), "Nạp đạ");
        t.left(false);
        t.left(false);
        t.delete();
        assert_eq!(t.text(), "Nạp ạ");
        assert_eq!(t.caret(), 4);
        t.home(false);
        t.backspace();
        assert_eq!(t.text(), "Nạp ạ", "backspace at start is a no-op");
        t.end(false);
        t.delete();
        assert_eq!(t.text(), "Nạp ạ", "delete at end is a no-op");
    }

    #[test]
    fn combining_marks_are_separate_chars() {
        // "ệ" typed as e + dot below + circumflex (3 chars)
        let mut t = TextEdit::new("", 40);
        t.insert("Nhe\u{0323}\u{0302}");
        assert_eq!(t.caret(), 5);
        t.backspace();
        assert_eq!(t.text(), "Nhe\u{0323}");
        t.backspace();
        assert_eq!(t.text(), "Nhe");
        // inserting in the middle keeps the byte offsets right
        t.left(false);
        t.insert("ễ");
        assert_eq!(t.text(), "Nhễe");
        assert_eq!(t.caret(), 3);
    }

    // test 42
    #[test]
    fn caret_and_selection_movement() {
        let mut t = TextEdit::new("Hồ sơ", 40);
        assert_eq!(t.selection(), None);
        t.home(true);
        assert_eq!(t.selection(), Some((0, 5)));
        t.right(false);
        assert_eq!(t.caret(), 5);
        assert_eq!(t.selection(), None);
        t.left(true);
        t.left(true);
        assert_eq!(t.selection(), Some((3, 5)));
        t.left(false); // collapses to the start of the selection
        assert_eq!(t.caret(), 3);
        assert_eq!(t.selection(), None);
        t.select_all();
        assert_eq!(t.selection(), Some((0, 5)));
        t.right(false);
        assert_eq!(t.caret(), 5);
        t.select_all();
        t.left(false);
        assert_eq!(t.caret(), 0);
        t.right(true);
        t.right(true);
        assert_eq!(t.selection(), Some((0, 2)));
        t.insert("X");
        assert_eq!(t.text(), "X sơ");
        t.select_all();
        t.backspace();
        assert_eq!(t.text(), "");
        assert_eq!(t.caret(), 0);
        t.left(false);
        t.right(false);
        assert_eq!(t.caret(), 0);
    }

    // test 43
    #[test]
    fn max_length_counts_chars() {
        let mut t = TextEdit::new("", 5);
        t.insert("ạạạạạạạ");
        assert_eq!(t.text().chars().count(), 5);
        t.insert("x");
        assert_eq!(t.text(), "ạạạạạ");
        t.select_all();
        t.insert("abcdefg");
        assert_eq!(t.text(), "abcde");
        let t = TextEdit::new("Đấu trường", 4);
        assert_eq!(t.text(), "Đấu ");
    }

    // test 44
    #[test]
    fn control_characters_are_dropped() {
        let mut t = TextEdit::new("a\nb\tc", 40);
        assert_eq!(t.text(), "abc");
        t.insert("\r\n\u{0}x\u{7f}y\u{85}");
        assert_eq!(t.text(), "abcxy");
    }
}
