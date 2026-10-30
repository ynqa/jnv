//! A small, self-contained vi-style modal editing engine for the query editor.
//!
//! The engine is intentionally decoupled from the promkit text editor: every
//! operation is expressed as a pure transformation over the current query text
//! (a `&str`) and a cursor index (a `char` offset), returning an [`Outcome`]
//! that the caller applies to the underlying widget. This keeps the modal logic
//! free of widget/rendering concerns and trivially unit-testable.
//!
//! Only NORMAL-mode keys flow through [`Editor::handle_normal`]; INSERT mode is
//! handled by the caller's existing text-insertion path so that completion and
//! the configured emacs-style keybinds keep working unchanged. The single
//! INSERT-mode concern the engine owns is the transition back to NORMAL on
//! <kbd>Esc</kbd>, exposed via [`Editor::leave_insert`].
//!
//! The buffer is single-line (the query editor has no newlines), so linewise
//! operators (`dd`, `cc`, `yy`) act on the whole line and `o`/`O`/`.`/multi-line
//! motions are intentionally unsupported.

use promkit_widgets::core::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// The current editing mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mode {
    /// Keys are interpreted as motions, operators, and edit commands.
    #[default]
    Normal,
    /// Keys insert text, exactly like the non-vi editor.
    Insert,
}

/// The effect a NORMAL-mode key has on the editor buffer.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// Move the cursor to the given absolute `char` index. The text is
    /// unchanged. Also used for pure mode changes (`i`, `a`, ...), where the
    /// new [`Mode`] is read from [`Editor::mode`] afterwards.
    Move(usize),
    /// Replace the whole buffer with `text` and place the cursor at `cursor`.
    Replace { text: String, cursor: usize },
    /// The key was consumed but nothing should change (a partial command such
    /// as a pending count/operator, or an unhandled key).
    Noop,
}

/// Pending operator (`d`, `c`, `y`) awaiting a motion, with its own count.
#[derive(Clone, Copy)]
struct Pending {
    op: Operator,
    count: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Operator {
    Delete,
    Change,
    Yank,
}

/// A key that consumes the following keypress as its argument.
#[derive(Clone, Copy)]
enum AwaitChar {
    /// `f`/`F`/`t`/`T` (optionally operator-pending).
    Find { forward: bool, till: bool },
    /// `r` — replace the char(s) under the cursor.
    Replace,
}

/// A resolved motion target plus whether it is inclusive of the target char.
struct Motion {
    pos: usize,
    inclusive: bool,
}

/// The modal editing state machine.
pub struct Editor {
    pub mode: Mode,
    /// Numeric count prefix being accumulated (e.g. the `3` in `3w`).
    count: Option<usize>,
    /// Operator awaiting a motion.
    pending: Option<Pending>,
    /// A key awaiting its character argument.
    await_char: Option<AwaitChar>,
    /// Whether the previous key was `g` (awaiting a second `g` for `gg`).
    pending_g: bool,
    /// The unnamed register, filled by yanks and deletes, pasted by `p`/`P`.
    register: String,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            mode: Mode::Normal,
            count: None,
            pending: None,
            await_char: None,
            pending_g: false,
            register: String::new(),
        }
    }
}

impl Editor {
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            ..Default::default()
        }
    }

    /// Leave INSERT mode and return to NORMAL, mirroring vi's "step left on
    /// Esc" behaviour. `cursor` is the current cursor index; the returned index
    /// is where the cursor should land.
    pub fn leave_insert(&mut self, cursor: usize) -> usize {
        self.mode = Mode::Normal;
        self.reset_pending();
        cursor.saturating_sub(1)
    }

    /// Whether the engine is mid-command (operator/count/await pending). Used by
    /// the caller to decide whether a key is part of a vi command.
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
            || self.count.is_some()
            || self.await_char.is_some()
            || self.pending_g
    }

    fn reset_pending(&mut self) {
        self.count = None;
        self.pending = None;
        self.await_char = None;
        self.pending_g = false;
    }

    /// Handle a key while in NORMAL mode.
    ///
    /// `text` is the query text without the cursor, `cursor` is the cursor's
    /// `char` index (`0..=text.chars().count()`).
    pub fn handle_normal(&mut self, key: &KeyEvent, text: &str, cursor: usize) -> Outcome {
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let cursor = cursor.min(len);

        // Resolve a key that was awaiting a character argument (f/F/t/T/r).
        if let Some(awaiting) = self.await_char.take() {
            return self.resolve_await(awaiting, key, &chars, cursor, len);
        }

        let Some(ch) = vi_char(key) else {
            // Non-character keys: map the common navigation keys, ignore the
            // rest. Any of these also cancels a pending command.
            let outcome = match key.code {
                KeyCode::Left | KeyCode::Backspace => Outcome::Move(cursor.saturating_sub(1)),
                KeyCode::Right => Outcome::Move(clamp_normal(cursor + 1, len)),
                KeyCode::Home => Outcome::Move(0),
                KeyCode::End => Outcome::Move(clamp_normal(len, len)),
                KeyCode::Esc => Outcome::Noop,
                _ => Outcome::Noop,
            };
            self.reset_pending();
            return outcome;
        };

        // `gg` — the only two-key prefix.
        if self.pending_g {
            self.pending_g = false;
            if ch == 'g' {
                return self.run_motion_or_operator(Motion::excl(0), &chars, cursor, len, None);
            }
            // Fall through: treat as a fresh key (vi ignores most `g{x}`).
        }

        // Count accumulation: digits 1-9 always, 0 only to extend an existing
        // count (a leading 0 is the "line start" motion).
        if ch.is_ascii_digit() && !(ch == '0' && self.count.is_none()) {
            let digit = ch as usize - '0' as usize;
            self.count = Some(self.count.unwrap_or(0) * 10 + digit);
            return Outcome::Noop;
        }

        // Operators: d / c / y. A repeated operator (dd/cc/yy) is linewise.
        if let Some(op) = operator_of(ch) {
            if let Some(p) = self.pending {
                if p.op == op {
                    // dd / cc / yy — whole line (counts are a no-op here).
                    return self.apply_linewise(op, &chars);
                }
            }
            self.pending = Some(Pending {
                op,
                count: self.count.take(),
            });
            return Outcome::Noop;
        }

        // `g` prefix.
        if ch == 'g' {
            self.pending_g = true;
            return Outcome::Noop;
        }

        // Motions that can also be operator targets.
        if let Some(motion) = self.resolve_motion(ch, &chars, cursor, len) {
            let word_forward = if is_word_forward(ch) {
                Some(ch == 'W')
            } else {
                None
            };
            return self.run_motion_or_operator(motion, &chars, cursor, len, word_forward);
        }

        // Keys that await a character argument.
        match ch {
            'f' => {
                self.await_char = Some(AwaitChar::Find {
                    forward: true,
                    till: false,
                });
                return Outcome::Noop;
            }
            'F' => {
                self.await_char = Some(AwaitChar::Find {
                    forward: false,
                    till: false,
                });
                return Outcome::Noop;
            }
            't' => {
                self.await_char = Some(AwaitChar::Find {
                    forward: true,
                    till: true,
                });
                return Outcome::Noop;
            }
            'T' => {
                self.await_char = Some(AwaitChar::Find {
                    forward: false,
                    till: true,
                });
                return Outcome::Noop;
            }
            'r' => {
                self.await_char = Some(AwaitChar::Replace);
                return Outcome::Noop;
            }
            _ => {}
        }

        // Mode-entry and standalone edit commands. These cancel any pending
        // operator (none of them combine with one).
        let count = mul_counts(self.pending.take().and_then(|p| p.count), self.count.take());
        self.run_command(ch, &chars, cursor, len, count)
    }

    /// Resolve a key that follows `f`/`F`/`t`/`T`/`r`.
    fn resolve_await(
        &mut self,
        awaiting: AwaitChar,
        key: &KeyEvent,
        chars: &[char],
        cursor: usize,
        len: usize,
    ) -> Outcome {
        let Some(target) = vi_char(key) else {
            self.reset_pending();
            return Outcome::Noop;
        };
        match awaiting {
            AwaitChar::Find { forward, till } => {
                let count = mul_counts(self.pending.and_then(|p| p.count), self.count.take());
                match find_char(chars, cursor, target, forward, till, count) {
                    Some(motion) => self.run_motion_or_operator(motion, chars, cursor, len, None),
                    None => {
                        self.reset_pending();
                        Outcome::Noop
                    }
                }
            }
            AwaitChar::Replace => {
                let count = self.count.take().unwrap_or(1);
                self.reset_pending();
                if cursor >= len || cursor + count > len {
                    return Outcome::Noop;
                }
                let mut out: Vec<char> = chars.to_vec();
                for c in out.iter_mut().skip(cursor).take(count) {
                    *c = target;
                }
                Outcome::Replace {
                    text: out.into_iter().collect(),
                    cursor: cursor + count - 1,
                }
            }
        }
    }

    /// Apply a resolved motion, either as a cursor move (no operator) or as the
    /// target of a pending operator.
    ///
    /// `word_forward` is `Some(big)` when the motion was `w`/`W` (so the
    /// `cw`-as-`ce` quirk can be applied), or `None` otherwise.
    fn run_motion_or_operator(
        &mut self,
        motion: Motion,
        chars: &[char],
        cursor: usize,
        len: usize,
        word_forward: Option<bool>,
    ) -> Outcome {
        if let Some(p) = self.pending.take() {
            self.count = None;
            // `cw` behaves like `ce` on a non-blank char (a vi quirk).
            let motion = match word_forward {
                Some(big)
                    if p.op == Operator::Change
                        && cursor < len
                        && !chars[cursor].is_whitespace() =>
                {
                    Motion::incl(next_word_end(chars, cursor, big))
                }
                _ => motion,
            };
            self.apply_operator(p.op, chars, cursor, motion, len)
        } else {
            self.count = None;
            Outcome::Move(clamp_normal(motion.pos, len))
        }
    }

    /// Resolve a single-key motion to a [`Motion`], or `None` if `ch` is not a
    /// motion. `count` (from `self.count` / pending operator) is applied here.
    fn resolve_motion(
        &self,
        ch: char,
        chars: &[char],
        cursor: usize,
        len: usize,
    ) -> Option<Motion> {
        let count = mul_counts(self.pending.and_then(|p| p.count), self.count);
        let m = match ch {
            'h' => Motion::excl(repeat(cursor, count, |c| c.saturating_sub(1))),
            'l' | ' ' => Motion::excl(repeat(cursor, count, |c| (c + 1).min(len))),
            '0' => Motion::excl(0),
            '$' => Motion::incl(len.saturating_sub(1)),
            '^' => Motion::excl(first_non_blank(chars)),
            'w' => Motion::excl(repeat(cursor, count, |c| next_word_start(chars, c, false))),
            'W' => Motion::excl(repeat(cursor, count, |c| next_word_start(chars, c, true))),
            'b' => Motion::excl(repeat(cursor, count, |c| prev_word_start(chars, c, false))),
            'B' => Motion::excl(repeat(cursor, count, |c| prev_word_start(chars, c, true))),
            'e' => Motion::incl(repeat(cursor, count, |c| next_word_end(chars, c, false))),
            'E' => Motion::incl(repeat(cursor, count, |c| next_word_end(chars, c, true))),
            'G' => Motion::incl(len.saturating_sub(1)),
            _ => return None,
        };
        Some(m)
    }

    /// Mode-entry keys and standalone edit commands.
    fn run_command(
        &mut self,
        ch: char,
        chars: &[char],
        cursor: usize,
        len: usize,
        count: usize,
    ) -> Outcome {
        self.reset_pending();
        match ch {
            // Mode entry.
            'i' => {
                self.mode = Mode::Insert;
                Outcome::Move(cursor)
            }
            'I' => {
                self.mode = Mode::Insert;
                Outcome::Move(first_non_blank(chars))
            }
            'a' => {
                self.mode = Mode::Insert;
                Outcome::Move((cursor + 1).min(len))
            }
            'A' => {
                self.mode = Mode::Insert;
                Outcome::Move(len)
            }
            // x / X — delete chars under / before the cursor.
            'x' => {
                if cursor >= len {
                    return Outcome::Noop;
                }
                let hi = (cursor + count).min(len);
                self.register = chars[cursor..hi].iter().collect();
                let text = remove_range(chars, cursor, hi);
                Outcome::Replace {
                    cursor: clamp_normal(cursor, text.chars().count()),
                    text,
                }
            }
            'X' => {
                if cursor == 0 {
                    return Outcome::Noop;
                }
                let lo = cursor.saturating_sub(count);
                self.register = chars[lo..cursor].iter().collect();
                let text = remove_range(chars, lo, cursor);
                Outcome::Replace { text, cursor: lo }
            }
            // D / C — to end of line. s / S — substitute.
            'D' => self.apply_operator(
                Operator::Delete,
                chars,
                cursor,
                Motion::incl(len.saturating_sub(1)),
                len,
            ),
            'C' => self.apply_operator(
                Operator::Change,
                chars,
                cursor,
                Motion::incl(len.saturating_sub(1)),
                len,
            ),
            's' => {
                if cursor >= len {
                    self.mode = Mode::Insert;
                    return Outcome::Move(cursor);
                }
                let hi = (cursor + count).min(len);
                self.register = chars[cursor..hi].iter().collect();
                self.mode = Mode::Insert;
                Outcome::Replace {
                    text: remove_range(chars, cursor, hi),
                    cursor,
                }
            }
            'S' => self.apply_linewise(Operator::Change, chars),
            // Paste.
            'p' => self.paste(chars, cursor, len, true),
            'P' => self.paste(chars, cursor, len, false),
            // Toggle case.
            '~' => {
                if cursor >= len {
                    return Outcome::Noop;
                }
                let hi = (cursor + count).min(len);
                let mut out = chars.to_vec();
                for c in out.iter_mut().skip(cursor).take(hi - cursor) {
                    *c = toggle_case(*c);
                }
                Outcome::Replace {
                    text: out.into_iter().collect(),
                    cursor: clamp_normal(hi, len),
                }
            }
            // Y == yy (linewise yank).
            'Y' => self.apply_linewise(Operator::Yank, chars),
            _ => Outcome::Noop,
        }
    }

    fn paste(&mut self, chars: &[char], cursor: usize, len: usize, after: bool) -> Outcome {
        if self.register.is_empty() {
            return Outcome::Noop;
        }
        let reg: Vec<char> = self.register.chars().collect();
        let at = if after && len > 0 { cursor + 1 } else { cursor };
        let at = at.min(len);
        let mut out: Vec<char> = chars[..at].to_vec();
        out.extend(reg.iter());
        out.extend(chars[at..].iter());
        Outcome::Replace {
            cursor: at + reg.len() - 1,
            text: out.into_iter().collect(),
        }
    }

    /// Apply an operator over the range between `start` and `motion`.
    fn apply_operator(
        &mut self,
        op: Operator,
        chars: &[char],
        start: usize,
        motion: Motion,
        len: usize,
    ) -> Outcome {
        let (lo, hi) = if motion.pos >= start {
            (start, (motion.pos + usize::from(motion.inclusive)).min(len))
        } else {
            (motion.pos, start)
        };
        if lo == hi {
            // Empty range: yank/delete nothing. Change still enters insert.
            return match op {
                Operator::Change => {
                    self.mode = Mode::Insert;
                    Outcome::Move(lo)
                }
                _ => Outcome::Move(clamp_normal(start, len)),
            };
        }
        match op {
            Operator::Yank => {
                self.register = chars[lo..hi].iter().collect();
                Outcome::Move(clamp_normal(lo, len))
            }
            Operator::Delete => {
                self.register = chars[lo..hi].iter().collect();
                let text = remove_range(chars, lo, hi);
                Outcome::Replace {
                    cursor: clamp_normal(lo, text.chars().count()),
                    text,
                }
            }
            Operator::Change => {
                self.register = chars[lo..hi].iter().collect();
                self.mode = Mode::Insert;
                Outcome::Replace {
                    text: remove_range(chars, lo, hi),
                    cursor: lo,
                }
            }
        }
    }

    /// Linewise operator (`dd`/`cc`/`yy`) on the single-line buffer.
    fn apply_linewise(&mut self, op: Operator, chars: &[char]) -> Outcome {
        self.reset_pending();
        match op {
            Operator::Yank => {
                self.register = chars.iter().collect();
                Outcome::Move(0)
            }
            Operator::Delete => {
                self.register = chars.iter().collect();
                Outcome::Replace {
                    text: String::new(),
                    cursor: 0,
                }
            }
            Operator::Change => {
                self.register = chars.iter().collect();
                self.mode = Mode::Insert;
                Outcome::Replace {
                    text: String::new(),
                    cursor: 0,
                }
            }
        }
    }
}

impl Motion {
    fn incl(pos: usize) -> Self {
        Motion {
            pos,
            inclusive: true,
        }
    }
    fn excl(pos: usize) -> Self {
        Motion {
            pos,
            inclusive: false,
        }
    }
}

fn is_word_forward(ch: char) -> bool {
    matches!(ch, 'w' | 'W')
}

fn operator_of(ch: char) -> Option<Operator> {
    match ch {
        'd' => Some(Operator::Delete),
        'c' => Some(Operator::Change),
        'y' => Some(Operator::Yank),
        _ => None,
    }
}

/// Extract a vi command character: a plain or shifted `Char` key (Ctrl/Alt
/// combinations are not vi command chars).
fn vi_char(key: &KeyEvent) -> Option<char> {
    match key.code {
        KeyCode::Char(c)
            if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT =>
        {
            Some(c)
        }
        _ => None,
    }
}

fn mul_counts(a: Option<usize>, b: Option<usize>) -> usize {
    a.unwrap_or(1) * b.unwrap_or(1)
}

/// Apply `f` `count` times starting from `start`, stopping early if it reaches
/// a fixed point (so word motions don't spin past the ends).
fn repeat(start: usize, count: usize, f: impl Fn(usize) -> usize) -> usize {
    let mut pos = start;
    for _ in 0..count.max(1) {
        let next = f(pos);
        if next == pos {
            break;
        }
        pos = next;
    }
    pos
}

fn clamp_normal(pos: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        pos.min(len - 1)
    }
}

fn remove_range(chars: &[char], lo: usize, hi: usize) -> String {
    chars[..lo].iter().chain(chars[hi..].iter()).collect()
}

fn toggle_case(c: char) -> char {
    if c.is_uppercase() {
        c.to_lowercase().next().unwrap_or(c)
    } else if c.is_lowercase() {
        c.to_uppercase().next().unwrap_or(c)
    } else {
        c
    }
}

fn first_non_blank(chars: &[char]) -> usize {
    chars.iter().position(|c| !c.is_whitespace()).unwrap_or(0)
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Class {
    Whitespace,
    Word,
    Punct,
}

fn class(c: char, big: bool) -> Class {
    if c.is_whitespace() {
        Class::Whitespace
    } else if big || c.is_alphanumeric() || c == '_' {
        // In WORD mode (`big`), every non-whitespace char is part of the word.
        Class::Word
    } else {
        Class::Punct
    }
}

/// vi `w`/`W`: start of the next word after `cursor`.
fn next_word_start(chars: &[char], cursor: usize, big: bool) -> usize {
    let len = chars.len();
    let mut i = cursor;
    if i >= len {
        return len;
    }
    let c0 = class(chars[i], big);
    if c0 != Class::Whitespace {
        while i < len && class(chars[i], big) == c0 {
            i += 1;
        }
    }
    while i < len && class(chars[i], big) == Class::Whitespace {
        i += 1;
    }
    i.min(len)
}

/// vi `e`/`E`: end of the next word at or after `cursor`.
fn next_word_end(chars: &[char], cursor: usize, big: bool) -> usize {
    let len = chars.len();
    if len == 0 {
        return 0;
    }
    let mut i = cursor + 1;
    while i < len && class(chars[i], big) == Class::Whitespace {
        i += 1;
    }
    if i >= len {
        return len - 1;
    }
    let c0 = class(chars[i], big);
    while i + 1 < len && class(chars[i + 1], big) == c0 {
        i += 1;
    }
    i
}

/// vi `b`/`B`: start of the word before `cursor`.
fn prev_word_start(chars: &[char], cursor: usize, big: bool) -> usize {
    if cursor == 0 {
        return 0;
    }
    let mut i = cursor - 1;
    while i > 0 && class(chars[i], big) == Class::Whitespace {
        i -= 1;
    }
    let c0 = class(chars[i], big);
    while i > 0 && class(chars[i - 1], big) == c0 {
        i -= 1;
    }
    i
}

/// vi `f`/`F`/`t`/`T`: find the `count`-th occurrence of `target`.
fn find_char(
    chars: &[char],
    cursor: usize,
    target: char,
    forward: bool,
    till: bool,
    count: usize,
) -> Option<Motion> {
    let len = chars.len();
    let count = count.max(1);
    if forward {
        let mut found = 0;
        let mut i = cursor;
        while i + 1 < len {
            i += 1;
            if chars[i] == target {
                found += 1;
                if found == count {
                    let pos = if till { i - 1 } else { i };
                    return Some(Motion::incl(pos));
                }
            }
        }
        None
    } else {
        let mut found = 0;
        let mut i = cursor;
        while i > 0 {
            i -= 1;
            if chars[i] == target {
                found += 1;
                if found == count {
                    let pos = if till { i + 1 } else { i };
                    return Some(Motion::excl(pos));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use promkit_widgets::core::crossterm::event::{KeyEvent, KeyEventKind, KeyEventState};

    fn key(c: char) -> KeyEvent {
        let modifiers = if c.is_uppercase() {
            KeyModifiers::SHIFT
        } else {
            KeyModifiers::NONE
        };
        KeyEvent {
            code: KeyCode::Char(c),
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn special(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    /// Feed a sequence of command chars; returns the final (text, cursor, mode).
    fn run(initial: &str, cursor: usize, keys: &str) -> (String, usize, Mode) {
        let mut ed = Editor::new(Mode::Normal);
        let mut text = initial.to_string();
        let mut cur = cursor;
        for c in keys.chars() {
            match ed.handle_normal(&key(c), &text, cur) {
                Outcome::Move(p) => cur = p,
                Outcome::Replace {
                    text: t,
                    cursor: nc,
                } => {
                    text = t;
                    cur = nc;
                }
                Outcome::Noop => {}
            }
        }
        (text, cur, ed.mode)
    }

    // --- motions -----------------------------------------------------------

    #[test]
    fn h_l_move_within_bounds() {
        assert_eq!(run("abc", 1, "h").1, 0);
        assert_eq!(run("abc", 0, "h").1, 0); // clamp at head
        assert_eq!(run("abc", 1, "l").1, 2);
        assert_eq!(run("abc", 2, "l").1, 2); // clamp at last char
    }

    #[test]
    fn zero_and_dollar() {
        assert_eq!(run("abc def", 5, "0").1, 0);
        assert_eq!(run("abc def", 0, "$").1, 6);
    }

    #[test]
    fn word_forward_backward() {
        // "foo bar baz", cursor on 'f'
        assert_eq!(run("foo bar baz", 0, "w").1, 4); // -> 'b' of bar
        assert_eq!(run("foo bar baz", 0, "ww").1, 8); // -> 'b' of baz
        assert_eq!(run("foo bar baz", 8, "b").1, 4); // back to bar
        assert_eq!(run("foo bar baz", 4, "e").1, 6); // end of bar
    }

    #[test]
    fn word_motion_treats_punctuation_as_word() {
        // ".foo" : 'w' from '.' should land on 'f'
        assert_eq!(run(".foo", 0, "w").1, 1);
        // WORD: ".foo bar" from start -> next WORD start 'b'
        assert_eq!(run(".foo bar", 0, "W").1, 5);
    }

    #[test]
    fn count_prefixed_motion() {
        assert_eq!(run("a b c d e", 0, "3w").1, 6); // -> 'd'
        assert_eq!(run("abcdef", 0, "3l").1, 3);
    }

    #[test]
    fn gg_and_capital_g() {
        assert_eq!(run("abc def", 5, "gg").1, 0);
        assert_eq!(run("abc def", 0, "G").1, 6);
    }

    // --- find --------------------------------------------------------------

    #[test]
    fn find_char_forward_and_till() {
        let mut ed = Editor::new(Mode::Normal);
        // f,
        assert_eq!(ed.handle_normal(&key('f'), "a,b,c", 0), Outcome::Noop);
        assert_eq!(ed.handle_normal(&key(','), "a,b,c", 0), Outcome::Move(1));
        // t,
        let mut ed = Editor::new(Mode::Normal);
        ed.handle_normal(&key('t'), "a,b,c", 0);
        assert_eq!(ed.handle_normal(&key(','), "a,b,c", 0), Outcome::Move(0));
    }

    #[test]
    fn find_char_backward() {
        let mut ed = Editor::new(Mode::Normal);
        ed.handle_normal(&key('F'), "a,b,c", 4);
        assert_eq!(ed.handle_normal(&key(','), "a,b,c", 4), Outcome::Move(3));
    }

    // --- edits -------------------------------------------------------------

    #[test]
    fn x_deletes_under_cursor() {
        let (t, c, _) = run("abc", 1, "x");
        assert_eq!((t.as_str(), c), ("ac", 1));
    }

    #[test]
    fn x_with_count() {
        let (t, c, _) = run("abcdef", 1, "3x");
        assert_eq!((t.as_str(), c), ("aef", 1));
    }

    #[test]
    fn capital_x_deletes_before() {
        let (t, c, _) = run("abc", 2, "X");
        assert_eq!((t.as_str(), c), ("ac", 1));
    }

    #[test]
    fn dw_deletes_word() {
        let (t, c, _) = run("foo bar", 0, "dw");
        assert_eq!((t.as_str(), c), ("bar", 0));
    }

    #[test]
    fn dd_clears_line() {
        let (t, c, m) = run("anything here", 4, "dd");
        assert_eq!((t.as_str(), c, m), ("", 0, Mode::Normal));
    }

    #[test]
    fn d_dollar_deletes_to_end() {
        let (t, c, _) = run("foo bar", 3, "d$");
        assert_eq!((t.as_str(), c), ("foo", 2));
    }

    #[test]
    fn capital_d_deletes_to_end() {
        let (t, c, _) = run("foo bar", 3, "D");
        assert_eq!((t.as_str(), c), ("foo", 2));
    }

    #[test]
    fn de_deletes_to_word_end_inclusive() {
        let (t, c, _) = run("foo bar", 0, "de");
        assert_eq!((t.as_str(), c), (" bar", 0));
    }

    #[test]
    fn cw_acts_like_ce_and_enters_insert() {
        let (t, c, m) = run("foo bar", 0, "cw");
        assert_eq!((t.as_str(), c, m), (" bar", 0, Mode::Insert));
    }

    #[test]
    fn cc_clears_and_inserts() {
        let (t, c, m) = run("foo bar", 3, "cc");
        assert_eq!((t.as_str(), c, m), ("", 0, Mode::Insert));
    }

    #[test]
    fn df_deletes_through_char() {
        let mut ed = Editor::new(Mode::Normal);
        ed.handle_normal(&key('d'), "a,b,c", 0);
        ed.handle_normal(&key('f'), "a,b,c", 0);
        let out = ed.handle_normal(&key(','), "a,b,c", 0);
        assert_eq!(
            out,
            Outcome::Replace {
                text: "b,c".to_string(),
                cursor: 0
            }
        );
    }

    #[test]
    fn count_with_operator() {
        let (t, c, _) = run("a b c d e", 0, "d3w");
        assert_eq!((t.as_str(), c), ("d e", 0));
    }

    // --- insert mode entry -------------------------------------------------

    #[test]
    fn i_a_capital_i_a_enter_insert() {
        assert_eq!(run("abc", 1, "i"), ("abc".to_string(), 1, Mode::Insert));
        assert_eq!(run("abc", 1, "a"), ("abc".to_string(), 2, Mode::Insert));
        assert_eq!(run("  abc", 4, "I"), ("  abc".to_string(), 2, Mode::Insert));
        assert_eq!(run("abc", 0, "A"), ("abc".to_string(), 3, Mode::Insert));
    }

    #[test]
    fn leave_insert_steps_left() {
        let mut ed = Editor::new(Mode::Insert);
        assert_eq!(ed.leave_insert(3), 2);
        assert_eq!(ed.mode, Mode::Normal);
        assert_eq!(ed.leave_insert(0), 0);
    }

    // --- replace / toggle / paste -----------------------------------------

    #[test]
    fn r_replaces_char() {
        let mut ed = Editor::new(Mode::Normal);
        ed.handle_normal(&key('r'), "abc", 1);
        let out = ed.handle_normal(&key('X'), "abc", 1);
        assert_eq!(
            out,
            Outcome::Replace {
                text: "aXc".to_string(),
                cursor: 1
            }
        );
    }

    #[test]
    fn tilde_toggles_case_and_advances() {
        let (t, c, _) = run("abc", 0, "~");
        assert_eq!((t.as_str(), c), ("Abc", 1));
    }

    #[test]
    fn yank_and_paste() {
        // yw yanks "foo ", p pastes after cursor.
        let mut ed = Editor::new(Mode::Normal);
        let mut text = "foo bar".to_string();
        let mut cur = 0;
        for c in "yw".chars() {
            match ed.handle_normal(&key(c), &text, cur) {
                Outcome::Move(p) => cur = p,
                Outcome::Replace {
                    text: t,
                    cursor: nc,
                } => {
                    text = t;
                    cur = nc;
                }
                Outcome::Noop => {}
            }
        }
        // cursor back at 0; paste after.
        let out = ed.handle_normal(&key('p'), &text, cur);
        assert_eq!(
            out,
            Outcome::Replace {
                text: "ffoo oo bar".to_string(),
                cursor: 4
            }
        );
    }

    #[test]
    fn delete_fills_register_for_paste() {
        let mut ed = Editor::new(Mode::Normal);
        // x deletes 'a' into register, then p pastes it after cursor.
        let out = ed.handle_normal(&key('x'), "abc", 0);
        assert_eq!(
            out,
            Outcome::Replace {
                text: "bc".to_string(),
                cursor: 0
            }
        );
        let out = ed.handle_normal(&key('p'), "bc", 0);
        assert_eq!(
            out,
            Outcome::Replace {
                text: "bac".to_string(),
                cursor: 1
            }
        );
    }

    // --- navigation keys / cancel -----------------------------------------

    #[test]
    fn arrow_keys_move() {
        let mut ed = Editor::new(Mode::Normal);
        assert_eq!(
            ed.handle_normal(&special(KeyCode::Left), "abc", 2),
            Outcome::Move(1)
        );
        assert_eq!(
            ed.handle_normal(&special(KeyCode::Right), "abc", 0),
            Outcome::Move(1)
        );
    }

    #[test]
    fn esc_cancels_pending_operator() {
        let mut ed = Editor::new(Mode::Normal);
        ed.handle_normal(&key('d'), "abc", 0);
        assert!(ed.is_pending());
        assert_eq!(
            ed.handle_normal(&special(KeyCode::Esc), "abc", 0),
            Outcome::Noop
        );
        assert!(!ed.is_pending());
    }

    #[test]
    fn empty_buffer_is_safe() {
        assert_eq!(run("", 0, "x"), ("".to_string(), 0, Mode::Normal));
        assert_eq!(run("", 0, "dw"), ("".to_string(), 0, Mode::Normal));
        assert_eq!(run("", 0, "$").1, 0);
        let (_, _, m) = run("", 0, "i");
        assert_eq!(m, Mode::Insert);
    }
}
