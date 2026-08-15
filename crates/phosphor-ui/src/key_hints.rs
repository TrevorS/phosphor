//! `KeymapFooter` / `HelpGrid` (`T034`, `T086`) — one keymap surface, three
//! densities.
//!
//! Draws [`phosphor_core::view::Node::KeyHints`]: the same [`KeyHint`] rows at [`Density::Footer`],
//! [`Density::Grid`] (the `SPC` leader popup, screen `3c`) and
//! [`Density::Help`] (the `:help` float body, screen `6d`). Design Language
//! §12 gives the widget its brief in one line — *"`KeymapFooter`: verb-labeled
//! hints; also renders the which-key grid — same data, two densities"* — and
//! `T086` is the third.
//!
//! **Nothing here knows what a keymap is.** A [`KeyHint`] is a key and a verb;
//! the list arrives from `runtime/keymaps.scm` through
//! `phosphor_steel::keymap::entries` and is read *live*, never cached, so a
//! `(keymap-set! …)` typed at the REPL is in the next frame with no wiring at
//! all (`keymaps.scm`'s own read-side header). That is `T034`'s whole liveness
//! claim, and this file is the end of it that draws.
//!
//! # Three densities, one datum
//!
//! | density | surface | shape |
//! |---|---|---|
//! | [`Footer`] | a float's footer strip (`3d`, `7a`, `8d`) | one row, `key verb · key verb`, all meta |
//! | [`Grid`] | the `SPC` leader popup (`3c`) | its own ground and top rule, a `SPC ·` title, entries packed into columns |
//! | [`Help`] | the `:help` float body (`6d`) | one entry per row, keys aligned in a column |
//!
//! [`Footer`]: Density::Footer
//! [`Grid`]: Density::Grid
//! [`Help`]: Density::Help
//!
//! [`Density::Footer`] is drawn *inside* a float by
//! [`crate::float::FloatFooter`], which landed with the chrome primitive
//! (`T084`); this module draws the same row for a footer that is **not** in a
//! float — a statusline slot, a docked strip — and produces the identical
//! string, so the two forms cannot drift into two spellings of one fact.
//!
//! # Two display rules, both read off the drawings
//!
//! **`<space>` is spelled `SPC`.** The canonical spelling is the layer's
//! (`keymaps.scm`: *"`SPC` is `<space>`, and a space is a separator rather than
//! a key"*), and it is what `keymap-entries` answers; `SPC` is what `3c` and
//! the Design Language draw. Canonical is for lookup, spelled is for reading,
//! and [`spell`] is the one place the second is made from the first.
//!
//! **A group's verb carries its own detail.** `keymaps.scm` writes `SPC c`'s
//! verb as `+claude · prompt · steer · interrupt`, and `3c` draws `+claude` in
//! text and `prompt · steer · interrupt` in meta-grey after it. So at
//! [`Density::Grid`] the verb splits at its **first** midline dot — §6 puts the
//! dot *"only inside a fact"*, and a group's label plus what is under it is one
//! fact. A verb with no dot is all label, which is what `3c`'s three leaves
//! (`transcript`, `jj timeline`, `files`) draw. Nothing splits at
//! [`Density::Help`]: §6 keeps the **em dash for cause** (`":interrupt — pause
//! at the next tool boundary"`), so `6d` draws each verb whole, em dash and
//! all.
//!
//! # The alignment is the mockups' own
//!
//! Both drawings pad the same way, and the arithmetic checks out against them
//! cell for cell:
//!
//! * `3c`, column one: `+claude` (7) and `+disk` (5) are followed by 2 and 4
//!   spaces before their details — a label padded to the column's widest, plus
//!   [`GAP`].
//! * `6d`: every verb starts at column 11, and the widest key is `:g/TODO/c`
//!   (9). 9 + [`GAP`] = 11.
//!
//! # Where the height comes from
//!
//! [`KeyHints::desired_height`] takes a width because [`Density::Grid`] packs
//! into as many columns as fit — *"narrow terminals drop, never squeeze"*
//! (§11), and a grid gives up a column rather than a row. That makes it the
//! first surface in this crate whose row count depends on its width, and
//! `crate::interpret`'s `height` deliberately takes none (*"§11 is 'nothing
//! ever wraps', so no node's row count depends on how wide it is"*). The seam
//! is [`KeyHints::natural_height`], the height at the widest packing, and it is
//! honest for every caller that has one: a `Grid` is composed into a sized slot
//! (`3c` is a strip above the statusline, not a float body) and a `Help` body's
//! height does not depend on width at all. **Flagged rather than folded in** —
//! a width parameter on that function is `spine`'s call.
//!
//! # A help body that does not fit — §11, argued (`T086`)
//!
//! A `Density::Help` body is taller than its float whenever the topic is large,
//! and until this pass it simply **stopped**: the loop below drew rows until it
//! ran out of area and the reader had no way to know there were more. That is
//! the defect, and the question it raises is what a body should do about scale.
//!
//! **Design Language §11 rules it: *"scale is grouping, not scrolling"*.** Both
//! of its examples group by something the datum carries — review blocks by
//! directory, transcripts by turn — so the test for this surface is whether a
//! [`KeyHint`] carries anything to group by. **It does not.** A `KeyHint` is a
//! key and a verb and nothing else ([`phosphor_core::view::KeyHint`]), which
//! leaves a widget exactly two derivable groupings, and neither is worth having:
//!
//! * **By shared leading key token** — the mechanism `KeyHints::common_prefix`
//!   already uses at [`Density::Grid`]. Measured against the table it would run
//!   on: `runtime/keymaps.scm` carries 131 `(list "…" (key/…))` forms over
//!   **87** distinct first tokens, so folding by prefix takes about a third of
//!   the rows off a page that is several times too long. Keys are flat; that
//!   is what a modal keymap is.
//! * **Packing into columns** the way [`Density::Grid`] does. That is squeezing,
//!   which §11 forbids in as many words, and the arithmetic does not save it
//!   either. The host's own note on `help_float` sizes the problem —
//!   *"a table of 200-odd bindings that a float can show 25 of"* — and
//!   [`GRID_COLUMNS`] is three, so the best a packing can do is a third of a
//!   number that is an order of magnitude too big.
//!
//! So the grouping §11 asks for is real and it is **already built — one level
//! up**. `:help` with no topic draws an *index* of topics with a count against
//! each, and `:help <topic>` is that index's expansion; the host owns both
//! (`crates/phosphor/src/main.rs`'s `index` and `TOPICS`). What is missing is
//! the second fold, *inside* a topic, and the datum cannot express it.
//!
//! **What lands here is therefore the half a widget can honestly own: the body
//! stops lying.** When the entries outrun the area the last row says how many
//! did not fit and where to narrow — `87 more — :help <topic>`, §6's
//! state-then-remedy em dash, the same shape as *"session lost — :reattach"*.
//! §11's *"narrow terminals drop, never squeeze"* permits the drop; it was the
//! **silence** that was wrong. Nothing here scrolls, and nothing moves that was
//! not asked for: `KeyHints::help` is still a pure function of its hints and
//! its area, so Invariant 3 is untouched — the same list at the same size draws
//! the same cells, and a rebind at the REPL changes the frame only by changing
//! the list.
//!
//! **The other half is owed, and it is `spine`'s**: a group label on `KeyHint`
//! — the layer already knows a binding's scope and role family, and
//! `keymap-entries` already emits both. With one, this density folds by it, a
//! folded group draws as its label plus its members inline, and `6d`'s own
//! `nouns  u unseen region  h hunk …` row — today a composed `Node::Spans` row
//! outside this widget (`crates/phosphor/tests/screen_6d.rs`'s notes) — becomes
//! the thing this widget draws. Raised as a contract request, not half-done
//! here.
//!
//! Owned by `surface`.

use phosphor_core::view::{Density, KeyHint};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::Style;
use ratatui_core::symbols::line;
use ratatui_core::widgets::Widget;

use crate::interpret::cells;
use crate::theme::Theme;

/// Cells between a key and its label, and between a label and its detail.
///
/// `3c` and `6d` both draw two (see the module docs' alignment note).
pub const GAP: u16 = 2;

/// Cells between two columns of the leader grid.
///
/// `3c` sets `column-gap: 24px` at an ~6.6px cell — three columns and a bit.
pub const COLUMN_GAP: u16 = 3;

/// The leader strip's own left inset, matching a float's
/// (`crate::float::PAD_COLS`). `3c` draws `padding: 8px 16px`.
pub const PAD_COLS: u16 = 2;

/// Columns the `SPC` grid packs into at most. `3c` draws
/// `grid-template-columns: repeat(3, 1fr)`.
pub const GRID_COLUMNS: u16 = 3;

/// §6's midline dot, which is also how a group's verb separates its label from
/// what is under it (see the module docs).
const DOT: &str = " · ";

/// The canonical spelling of the leader, as `keymap-entries` answers it.
const LEADER_CANONICAL: &str = "<space>";

/// The drawn spelling of the leader (`3c`, Design Language).
const LEADER_SPELLED: &str = "SPC";

/// What a [`Density::Help`] body says instead of ending in silence, after the
/// count of the rows that did not fit.
///
/// §6's em dash — state, then the remedy, exactly as *"session lost —
/// :reattach"* is written — and the command spelled whole, never `:h`. The
/// module docs argue why this is a drop rather than a scroll.
const HELP_OVERFLOW: &str = " more — :help <topic>";

/// One keymap surface, at one density.
///
/// Built per frame from the live keymap and thrown away — it borrows the hints
/// and the theme and owns nothing.
#[derive(Debug, Clone, Copy)]
pub struct KeyHints<'a> {
    hints: &'a [KeyHint],
    density: Density,
    theme: &'a Theme,
}

impl<'a> KeyHints<'a> {
    /// A surface over `hints`, drawn at `density`.
    #[must_use]
    pub const fn new(hints: &'a [KeyHint], density: Density, theme: &'a Theme) -> Self {
        Self {
            hints,
            density,
            theme,
        }
    }

    /// Rows this surface wants at `width`.
    ///
    /// Zero for an empty hint list at every density — a layer with no table
    /// draws *nothing*, which is the honest rendering of "nothing is bound"
    /// (`keymap.rs`: *"Empty for a layer with no table"*).
    #[must_use]
    pub fn desired_height(&self, width: u16) -> u16 {
        if self.hints.is_empty() {
            return 0;
        }
        match self.density {
            Density::Footer => 1,
            Density::Help => u16::try_from(self.hints.len()).unwrap_or(u16::MAX),
            Density::Grid => {
                let entries = self.entries();
                let columns = pack(&entries, width.saturating_sub(2 * PAD_COLS));
                let rows = rows_for(entries.len(), columns);
                // The top rule, the `SPC ·` title if there is one, the entries.
                1 + u16::from(self.title(&entries).is_some()) + rows
            }
        }
    }

    /// The height at the widest packing — what a caller with no width to offer
    /// gets. See the module docs.
    #[must_use]
    pub fn natural_height(&self) -> u16 {
        self.desired_height(u16::MAX)
    }

    /// The hints as drawable entries, in the order they were given.
    ///
    /// Order is data: `keymaps.scm` keeps the table as *"one ordered list, not
    /// a hash of hashes … `3c` draws the leader groups in the order they are
    /// declared"*, and a rebind keeps its place, so which-key does not
    /// reshuffle because you changed what a key does.
    fn entries(&self) -> Vec<Entry<'a>> {
        let split = matches!(self.density, Density::Grid);
        let trim = self.common_prefix();
        self.hints
            .iter()
            .map(|hint| {
                let tokens = tokens(&hint.key.0);
                let key = spell(&tokens[trim.min(tokens.len())..]);
                let (label, detail) = if split {
                    match hint.verb.split_once(DOT) {
                        Some((label, detail)) => (label, Some(detail)),
                        None => (hint.verb.as_str(), None),
                    }
                } else {
                    (hint.verb.as_str(), None)
                };
                Entry { key, label, detail }
            })
            .collect()
    }

    /// How many leading key tokens every hint shares — the prefix the grid
    /// puts in its title instead of repeating on every row.
    ///
    /// Zero at every density but [`Density::Grid`], and zero unless *every*
    /// hint keeps at least one token of its own: a list where one entry is
    /// exactly the prefix has no prefix to lift.
    fn common_prefix(&self) -> usize {
        if !matches!(self.density, Density::Grid) {
            return 0;
        }
        let keys: Vec<Vec<&str>> = self.hints.iter().map(|hint| tokens(&hint.key.0)).collect();
        let Some(first) = keys.first() else {
            return 0;
        };
        let mut shared = 0;
        while shared + 1 < first.len()
            && keys
                .iter()
                .all(|key| key.len() > shared + 1 && key[shared] == first[shared])
        {
            shared += 1;
        }
        shared
    }

    /// The grid's title row — the shared prefix, spelled, with §6's dot after
    /// it. `3c` draws `SPC ·`.
    fn title(&self, entries: &[Entry<'a>]) -> Option<String> {
        let shared = self.common_prefix();
        if shared == 0 || entries.is_empty() {
            return None;
        }
        let first = tokens(&self.hints.first()?.key.0);
        Some(format!("{} ·", spell(&first[..shared])))
    }

    // -- drawing ------------------------------------------------------------

    /// `↵ open · s mark seen · esc` — the same row
    /// [`crate::float::FloatFooter`] draws, for a footer that is not in a
    /// float.
    fn footer(self, area: Rect, buf: &mut Buffer) {
        let mut text = String::new();
        for (index, hint) in self.hints.iter().enumerate() {
            if index > 0 {
                text.push_str(DOT);
            }
            text.push_str(&spell(&tokens(&hint.key.0)));
            if !hint.verb.is_empty() {
                text.push(' ');
                text.push_str(&hint.verb);
            }
        }
        write(
            buf,
            area,
            area.x,
            area.y,
            &text,
            Style::new().fg(self.theme.neutrals.meta),
        );
    }

    /// `6d`: one entry per row, keys aligned into a column.
    ///
    /// A list taller than `area` spends its last row on [`HELP_OVERFLOW`]
    /// rather than stopping mid-table — §11's drop, made visible. See the
    /// module docs for why it is a drop and not a scroll.
    fn help(self, area: Rect, buf: &mut Buffer) {
        let entries = self.entries();
        let key_width = entries.iter().map(|entry| entry.key_width()).max();
        let Some(key_width) = key_width else { return };
        let key_style = Style::new().fg(self.theme.actors.claude);
        let verb_style = Style::new().fg(self.theme.neutrals.meta);

        // One row of the budget buys the truth about the rest of the table, and
        // is only spent when there is a rest: a body that fits draws exactly
        // what it drew before this arm learned to count.
        let height = area.height as usize;
        let shown = if entries.len() > height {
            height.saturating_sub(1)
        } else {
            entries.len()
        };

        for (index, entry) in entries.iter().take(shown).enumerate() {
            let Ok(dy) = u16::try_from(index) else { break };
            let y = area.y + dy;
            write(buf, area, area.x, y, &entry.key, key_style);
            let verb_x = area.x.saturating_add(key_width).saturating_add(GAP);
            write(buf, area, verb_x, y, entry.label, verb_style);
        }

        let dropped = entries.len() - shown;
        if dropped > 0 {
            // §6: *"a number beats an adjective"* — the count, not "and more".
            // At the left edge rather than in the verb column, because it is a
            // statement about the list and not a row of it.
            let y = area.y + u16::try_from(shown).unwrap_or(u16::MAX);
            write(
                buf,
                area,
                area.x,
                y,
                &format!("{dropped}{HELP_OVERFLOW}"),
                verb_style,
            );
        }
    }

    /// `3c`: the which-key strip — its own ground, a mood rule on top, the
    /// `SPC ·` title, then the entries packed into columns.
    ///
    /// The ground and the rule are this widget's because the surface *is* the
    /// strip: `3c` paints the float body colour behind it and an informational
    /// rule above it, and a [`phosphor_core::view::Node`] cannot say what
    /// ground it is painted on (`interpret.rs`'s flagged gap).
    fn grid(self, area: Rect, buf: &mut Buffer) {
        let ground = Style::new()
            .fg(self.theme.neutrals.text)
            .bg(self.theme.float.body);
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                buf[(x, y)].set_symbol(" ").set_style(ground);
            }
        }

        let rule = Style::new()
            .fg(self.theme.float.informational)
            .bg(self.theme.float.body);
        for x in area.x..area.right() {
            buf[(x, area.y)]
                .set_symbol(line::HORIZONTAL)
                .set_style(rule);
        }

        let entries = self.entries();
        if entries.is_empty() {
            return;
        }
        let meta = Style::new()
            .fg(self.theme.neutrals.meta)
            .bg(self.theme.float.body);
        let text = Style::new()
            .fg(self.theme.neutrals.text)
            .bg(self.theme.float.body);
        let key_style = Style::new()
            .fg(self.theme.actors.claude)
            .bg(self.theme.float.body);

        let left = area.x.saturating_add(PAD_COLS);
        let inner = area.width.saturating_sub(2 * PAD_COLS);
        let mut y = area.y + 1;
        if let Some(title) = self.title(&entries) {
            if y >= area.bottom() {
                return;
            }
            write(buf, area, left, y, &title, meta);
            y += 1;
        }

        let columns = pack(&entries, inner);
        let widths = column_widths(&entries, columns);
        let rows = rows_for(entries.len(), columns);
        for row in 0..rows {
            if y >= area.bottom() {
                break;
            }
            let mut x = left;
            for (column, width) in widths.iter().enumerate() {
                let index = row as usize * widths.len() + column;
                let Some(entry) = entries.get(index) else {
                    break;
                };
                write(buf, area, x, y, &entry.key, key_style);
                write(
                    buf,
                    area,
                    x.saturating_add(width.label_at()),
                    y,
                    entry.label,
                    text,
                );
                if let Some(detail) = entry.detail {
                    write(
                        buf,
                        area,
                        x.saturating_add(width.detail_at()),
                        y,
                        detail,
                        meta,
                    );
                }
                x = x.saturating_add(width.width()).saturating_add(COLUMN_GAP);
            }
            y += 1;
        }
    }
}

impl Widget for KeyHints<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.is_empty() || self.hints.is_empty() {
            return;
        }
        match self.density {
            Density::Footer => self.footer(area, buf),
            Density::Grid => self.grid(area, buf),
            Density::Help => self.help(area, buf),
        }
    }
}

// ---------------------------------------------------------------------------
// Entries and packing
// ---------------------------------------------------------------------------

/// One hint, ready to draw: the key as it reads, and the verb in one or two
/// parts.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry<'a> {
    /// The key, spelled (`SPC c`, `gg`, `<C-r>`), with any shared prefix
    /// already lifted into the title.
    key: String,
    /// What it does — the whole verb, or the part before the first midline dot
    /// at [`Density::Grid`].
    label: &'a str,
    /// What is under it, at [`Density::Grid`] only.
    detail: Option<&'a str>,
}

impl Entry<'_> {
    fn key_width(&self) -> u16 {
        cells(&self.key)
    }

    fn label_width(&self) -> u16 {
        cells(self.label)
    }

    fn detail_width(&self) -> u16 {
        self.detail.map_or(0, cells)
    }
}

/// One column of the grid, as wide as its own widest key, label and detail.
///
/// Three widths rather than one because the padding is per column and per
/// part: `3c` lines up two details by padding `+disk` to `+claude`, and a
/// column with no details is only as wide as its labels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Column {
    key: u16,
    label: u16,
    detail: u16,
}

impl Column {
    /// Where this column's label starts, relative to its left edge.
    const fn label_at(self) -> u16 {
        self.key.saturating_add(GAP)
    }

    /// Where this column's detail starts, relative to its left edge.
    const fn detail_at(self) -> u16 {
        self.label_at()
            .saturating_add(self.label)
            .saturating_add(GAP)
    }

    /// Every cell the column needs.
    const fn width(self) -> u16 {
        if self.detail == 0 {
            self.label_at().saturating_add(self.label)
        } else {
            self.detail_at().saturating_add(self.detail)
        }
    }
}

/// Rows `count` entries need in `columns` columns, filling row-major (`3c`'s
/// grid order: `c u t` then `r j f`).
const fn rows_for(count: usize, columns: u16) -> u16 {
    if columns == 0 {
        return 0;
    }
    let columns = columns as usize;
    let rows = count.div_ceil(columns);
    if rows > u16::MAX as usize {
        u16::MAX
    } else {
        rows as u16
    }
}

/// How many columns fit in `width` — the most, down to one.
///
/// §11: *"narrow terminals drop, never squeeze."* A grid gives up a **column**
/// rather than an entry, so nothing is lost on the way down; the last rung is
/// one column, and below that the rows clip at the right edge.
fn pack(entries: &[Entry<'_>], width: u16) -> u16 {
    let most = GRID_COLUMNS.min(u16::try_from(entries.len()).unwrap_or(GRID_COLUMNS));
    for columns in (1..=most).rev() {
        let total = column_widths(entries, columns)
            .iter()
            .map(|column| column.width())
            .fold(0u16, u16::saturating_add)
            .saturating_add(COLUMN_GAP.saturating_mul(columns.saturating_sub(1)));
        if total <= width {
            return columns;
        }
    }
    1
}

/// Each column's widths, filling row-major.
fn column_widths(entries: &[Entry<'_>], columns: u16) -> Vec<Column> {
    let mut widths = vec![Column::default(); columns.max(1) as usize];
    if columns == 0 {
        return widths;
    }
    for (index, entry) in entries.iter().enumerate() {
        let slot = &mut widths[index % columns as usize];
        slot.key = slot.key.max(entry.key_width());
        slot.label = slot.label.max(entry.label_width());
        slot.detail = slot.detail.max(entry.detail_width());
    }
    widths
}

// ---------------------------------------------------------------------------
// Spelling
// ---------------------------------------------------------------------------

/// A key sequence as its keys: `<space>c` → `["<space>", "c"]`.
///
/// The canonical spelling's own rule, from `keymaps.scm`: *"a bracketed key is
/// copied whole … a run of spaces separates tokens and is never itself a key."*
/// A sequence that is spelled with spaces (`SPC c p`) tokenises the same way as
/// one that is not (`<space>cp`), which is what makes them one binding.
fn tokens(seq: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = seq;
    while !rest.is_empty() {
        if let Some(tail) = rest.strip_prefix(' ') {
            rest = tail;
            continue;
        }
        if rest.starts_with('<') {
            let end = rest.find('>').map_or(rest.len(), |at| at + 1);
            let (token, tail) = rest.split_at(end);
            out.push(token);
            rest = tail;
            continue;
        }
        let end = rest
            .char_indices()
            .nth(1)
            .map_or(rest.len(), |(index, _)| index);
        let (token, tail) = rest.split_at(end);
        out.push(token);
        rest = tail;
    }
    out
}

/// Tokens as they read: `["<space>", "c"]` → `SPC c`, `["g", "g"]` → `gg`.
///
/// Single characters run together the way vim writes them; anything longer —
/// the spelled leader, a bracketed chord — takes a space beside it, which is
/// how `keymaps.scm` writes `SPC c p` and how `3c` draws its title.
fn spell(tokens: &[&str]) -> String {
    let mut out = String::new();
    let mut previous_wide = false;
    for token in tokens {
        let spelled = if *token == LEADER_CANONICAL {
            LEADER_SPELLED
        } else {
            token
        };
        let wide = spelled.chars().nth(1).is_some();
        if !out.is_empty() && (wide || previous_wide) {
            out.push(' ');
        }
        out.push_str(spelled);
        previous_wide = wide;
    }
    out
}

/// Write `text` at `(x, y)`, clipped to `area`. Returns the column after the
/// last cell written.
fn write(buf: &mut Buffer, area: Rect, x: u16, y: u16, text: &str, style: Style) -> u16 {
    if area.is_empty() || x >= area.right() || y >= area.bottom() || y < area.y {
        return x;
    }
    let room = area.right() - x;
    let (next, _) = buf.set_stringn(x, y, text, room as usize, style);
    next.min(area.right())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{KeyHints, spell, tokens};
    use crate::theme::Theme;
    use phosphor_core::request::KeySeq;
    use phosphor_core::view::{Density, KeyHint};
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::Rect;
    use ratatui_core::widgets::Widget;

    fn hint(key: &str, verb: &str) -> KeyHint {
        KeyHint {
            key: KeySeq(key.to_owned()),
            verb: verb.to_owned(),
        }
    }

    /// `3c`'s six rows, verbs exactly as `runtime/keymaps.scm` writes them.
    fn leader() -> Vec<KeyHint> {
        vec![
            hint("<space>c", "+claude · prompt · steer · interrupt"),
            hint("<space>u", "+unseen · next · list · mark seen"),
            hint("<space>t", "transcript"),
            hint("<space>r", "+disk · refresh · diff"),
            hint("<space>j", "jj timeline"),
            hint("<space>f", "files"),
        ]
    }

    fn draw(hints: &[KeyHint], density: Density, width: u16, height: u16) -> Buffer {
        let theme = Theme::phosphor_dark();
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        KeyHints::new(hints, density, &theme).render(area, &mut buf);
        buf
    }

    fn row(buf: &Buffer, y: u16) -> String {
        (buf.area.x..buf.area.right())
            .map(|x| buf[(x, y)].symbol())
            .collect::<String>()
            .trim_end()
            .to_owned()
    }

    // -- spelling ------------------------------------------------------------

    #[test]
    fn a_bracketed_key_is_one_token_and_a_space_is_none() {
        assert_eq!(tokens("<space>c"), vec!["<space>", "c"]);
        assert_eq!(tokens("SPC c p"), vec!["S", "P", "C", "c", "p"]);
        assert_eq!(tokens("<C-r>"), vec!["<C-r>"]);
        assert_eq!(tokens("gg"), vec!["g", "g"]);
        assert_eq!(tokens(""), Vec::<&str>::new());
        // A malformed sequence is drawn, not dropped: the layer canonicalises,
        // and a surface that swallowed a key would hide the mistake.
        assert_eq!(tokens("<oops"), vec!["<oops"]);
    }

    #[test]
    fn the_leader_is_drawn_as_spc_and_never_run_into_its_leaf() {
        assert_eq!(spell(&tokens("<space>")), "SPC");
        assert_eq!(spell(&tokens("<space>cp")), "SPC cp");
        assert_eq!(spell(&tokens("gg")), "gg");
        assert_eq!(spell(&tokens("<C-r>")), "<C-r>");
    }

    // -- the footer ----------------------------------------------------------

    /// The row is the same string `crate::float::FloatFooter` draws, so a
    /// footer outside a float and one inside it cannot become two spellings.
    #[test]
    fn a_footer_is_one_row_of_key_verb_pairs() {
        let hints = vec![hint("↵", "open"), hint("s", "mark seen"), hint("esc", "")];
        let buf = draw(&hints, Density::Footer, 60, 3);
        assert_eq!(row(&buf, 0), "↵ open · s mark seen · esc");
        assert_eq!(row(&buf, 1), "", "a footer is one row");
    }

    // -- the leader grid (3c) ------------------------------------------------

    #[test]
    fn the_leader_grid_lifts_the_shared_prefix_into_its_title() {
        let buf = draw(&leader(), Density::Grid, 120, 6);
        assert_eq!(row(&buf, 1), "  SPC ·");
        // The rows carry the leaf key alone, as `3c` draws them.
        assert!(
            row(&buf, 2).starts_with("  c  +claude"),
            "{:?}",
            row(&buf, 2)
        );
    }

    #[test]
    fn a_group_shows_its_children_in_meta_after_its_label() {
        let theme = Theme::phosphor_dark();
        let buf = draw(&leader(), Density::Grid, 120, 6);
        let drawn = row(&buf, 2);
        assert!(
            drawn.contains("+claude  prompt · steer · interrupt"),
            "{drawn:?}"
        );
        // The label is text, the detail is meta — `3c`'s two greys.
        let label_at = drawn.find('+').expect("a group label") as u16;
        assert_eq!(buf[(label_at, 2)].fg, theme.neutrals.text);
        let detail_at = drawn.find("prompt").expect("the detail") as u16;
        assert_eq!(buf[(detail_at, 2)].fg, theme.neutrals.meta);
        // And the key is claude's, as every drawing of the namespace has it.
        assert_eq!(buf[(2, 2)].fg, theme.actors.claude);
    }

    /// `3c`'s own alignment: `+disk` is padded to `+claude`'s width, so the two
    /// details start in the same column.
    #[test]
    fn labels_pad_to_the_widest_in_their_own_column() {
        let buf = draw(&leader(), Density::Grid, 120, 8);
        let first = row(&buf, 2);
        let second = row(&buf, 3);
        assert_eq!(
            first.find("prompt · steer"),
            second.find("refresh · diff"),
            "{first:?} / {second:?}"
        );
    }

    #[test]
    fn a_narrow_grid_gives_up_a_column_rather_than_an_entry() {
        let wide = draw(&leader(), Density::Grid, 200, 8);
        let narrow = draw(&leader(), Density::Grid, 60, 10);
        let count = |buf: &Buffer, height: u16| {
            (0..height)
                .map(|y| row(buf, y))
                .filter(|line| line.contains("transcript"))
                .count()
        };
        assert_eq!(count(&wide, 8), 1);
        assert_eq!(count(&narrow, 10), 1, "every entry survives the ladder");
        // Fewer columns means more rows, and never a second line of one entry.
        assert!(
            KeyHints::new(&leader(), Density::Grid, &Theme::phosphor_dark()).desired_height(60)
                > KeyHints::new(&leader(), Density::Grid, &Theme::phosphor_dark())
                    .desired_height(200)
        );
    }

    #[test]
    fn the_strip_paints_its_own_ground_and_a_rule_on_top() {
        let theme = Theme::phosphor_dark();
        let buf = draw(&leader(), Density::Grid, 120, 6);
        assert_eq!(buf[(0, 0)].symbol(), "─");
        assert_eq!(buf[(0, 0)].fg, theme.float.informational);
        assert_eq!(buf[(0, 1)].bg, theme.float.body);
        assert_eq!(buf[(119, 5)].bg, theme.float.body);
    }

    // -- the help body (6d) --------------------------------------------------

    #[test]
    fn the_help_body_aligns_every_verb_in_one_column() {
        let hints = vec![
            hint("viu", "select inner unseen region"),
            hint(":g/TODO/c", "global over a pattern"),
            hint("dih", "delete inner hunk"),
        ];
        let buf = draw(&hints, Density::Help, 80, 4);
        // The widest key is 9 cells, so every verb starts at 11 — `6d`'s own
        // column.
        for (y, verb) in [(0, "select"), (1, "global"), (2, "delete")] {
            assert_eq!(row(&buf, y).find(verb), Some(11), "row {y}");
        }
    }

    #[test]
    fn the_help_body_draws_a_verb_whole() {
        // `6d` draws `next / previous unseen · ]b block-wise` in one meta run;
        // only the leader grid splits at the dot.
        let hints = vec![hint("]u", "next / previous unseen · ]b block-wise")];
        let theme = Theme::phosphor_dark();
        let buf = draw(&hints, Density::Help, 80, 2);
        assert_eq!(row(&buf, 0), "]u  next / previous unseen · ]b block-wise");
        assert_eq!(buf[(4, 0)].fg, theme.neutrals.meta);
        assert_eq!(buf[(0, 0)].fg, theme.actors.claude);
    }

    /// `T086`'s limit, closed the way §11 rules rather than with a scrollbar:
    /// the body drops what does not fit and **says so**.
    #[test]
    fn a_help_body_taller_than_its_float_names_what_it_dropped() {
        let hints: Vec<KeyHint> = (0..10)
            .map(|n| hint(&format!("g{n}"), &format!("goto {n}")))
            .collect();
        let buf = draw(&hints, Density::Help, 40, 4);
        assert_eq!(row(&buf, 0), "g0  goto 0");
        assert_eq!(row(&buf, 2), "g2  goto 2");
        // Three rows of table, one of truth: 10 − 3 = 7.
        assert_eq!(row(&buf, 3), "7 more — :help <topic>");
    }

    /// The marker costs a row only when there is something to say. A body that
    /// fits draws what `6d` draws and nothing else.
    #[test]
    fn a_help_body_that_fits_draws_no_marker() {
        let hints = vec![
            hint("viu", "select inner unseen region"),
            hint("dih", "delete inner hunk"),
        ];
        for height in [2, 3, 9] {
            let buf = draw(&hints, Density::Help, 40, height);
            assert_eq!(row(&buf, 0), "viu  select inner unseen region");
            assert_eq!(row(&buf, 1), "dih  delete inner hunk");
            for y in 2..height {
                assert_eq!(row(&buf, y), "", "row {y} at height {height}");
            }
        }
    }

    /// The count is what did *not* fit, at every height — including the one
    /// where the marker is the only row there is room for.
    #[test]
    fn the_dropped_count_is_the_rows_the_reader_cannot_see() {
        let hints: Vec<KeyHint> = (0..6).map(|n| hint(&format!("{n}"), "goto")).collect();
        for (height, expected) in [(1, "6 more"), (2, "5 more"), (5, "2 more")] {
            let buf = draw(&hints, Density::Help, 40, height);
            let last = row(&buf, height - 1);
            assert!(last.starts_with(expected), "{last:?} at height {height}");
        }
        // Six in six is not an overflow, and the sixth row is an entry.
        let exact = draw(&hints, Density::Help, 40, 6);
        assert_eq!(row(&exact, 5), "5  goto");
    }

    /// It reads as a note, not as a binding: meta, at the left edge, with the
    /// key column left to the keys.
    #[test]
    fn the_marker_is_meta_and_not_a_key() {
        let theme = Theme::phosphor_dark();
        let hints: Vec<KeyHint> = (0..8).map(|n| hint(&format!("g{n}"), "goto")).collect();
        let buf = draw(&hints, Density::Help, 40, 3);
        assert_eq!(buf[(0, 2)].fg, theme.neutrals.meta);
        assert_ne!(buf[(0, 2)].fg, theme.actors.claude);
    }

    // -- degenerate cases ----------------------------------------------------

    #[test]
    fn nothing_bound_draws_nothing_at_all() {
        for density in [Density::Footer, Density::Grid, Density::Help] {
            let buf = draw(&[], density, 40, 4);
            assert_eq!(buf, Buffer::empty(Rect::new(0, 0, 40, 4)), "{density:?}");
            assert_eq!(
                KeyHints::new(&[], density, &Theme::phosphor_dark()).desired_height(40),
                0
            );
        }
    }

    #[test]
    fn a_surface_smaller_than_its_content_clips_and_does_not_panic() {
        let theme = Theme::phosphor_dark();
        for density in [Density::Footer, Density::Grid, Density::Help] {
            for area in [
                Rect::new(0, 0, 1, 1),
                Rect::new(0, 0, 4, 2),
                Rect::new(3, 3, 6, 3),
                Rect::new(0, 0, 200, 1),
            ] {
                let mut buf = Buffer::empty(Rect::new(0, 0, 40, 8));
                KeyHints::new(&leader(), density, &theme).render(area, &mut buf);
            }
        }
    }

    #[test]
    fn a_list_with_no_shared_prefix_has_no_title_row() {
        let hints = vec![hint("g", "goto"), hint("d", "delete")];
        let buf = draw(&hints, Density::Grid, 60, 4);
        assert_eq!(row(&buf, 1), "  g  goto   d  delete");
    }
}
