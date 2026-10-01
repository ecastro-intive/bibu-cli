//! The one place that draws human tables, so every list command looks and shrinks the same way.

use comfy_table::modifiers::{UTF8_ROUND_CORNERS, UTF8_SOLID_INNER_BORDERS};
use comfy_table::presets::UTF8_FULL;
use comfy_table::{ColumnConstraint, ContentArrangement, Table, Width};

/// Terminals narrower than this lose their `WHEN` style columns (dates, triggers).
pub const NARROW: u16 = 100;
/// Terminals narrower than this also lose their `WHO` style columns (authors, creators).
pub const TINY: u16 = 80;

/// Cell padding comfy-table adds on each side of a column.
const PADDING: usize = 2;

pub struct Column {
    pub name: &'static str,
    /// Short fields (ids, states, dates, hashes) that must never wrap.
    pub keep: bool,
    /// Hidden when the terminal is narrower than this many columns.
    pub hide_below: Option<u16>,
}

impl Column {
    /// A column that wraps and is never dropped (titles, branches, paths).
    pub const fn flex(name: &'static str) -> Self {
        Self {
            name,
            keep: false,
            hide_below: None,
        }
    }

    /// A short column that never wraps and is never dropped.
    pub const fn keep(name: &'static str) -> Self {
        Self {
            name,
            keep: true,
            hide_below: None,
        }
    }

    /// Wrapping column dropped on terminals narrower than `width`.
    pub const fn flex_below(name: &'static str, width: u16) -> Self {
        Self {
            name,
            keep: false,
            hide_below: Some(width),
        }
    }

    /// Non-wrapping column dropped on terminals narrower than `width`.
    pub const fn keep_below(name: &'static str, width: u16) -> Self {
        Self {
            name,
            keep: true,
            hide_below: Some(width),
        }
    }
}

/// Draws `rows` for the current terminal: bordered, word wrapped to its width, and with the
/// optional columns dropped when it is narrow. Outside a terminal it uses the natural width.
pub fn render(columns: &[Column], rows: Vec<Vec<String>>) -> String {
    let width = Table::new().width();
    render_at(columns, rows, width)
}

pub(crate) fn render_at(columns: &[Column], rows: Vec<Vec<String>>, width: Option<u16>) -> String {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.apply_modifier(UTF8_ROUND_CORNERS);
    table.apply_modifier(UTF8_SOLID_INNER_BORDERS);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    if let Some(width) = width {
        table.set_width(width);
    }
    table.set_header(columns.iter().map(|c| c.name));

    let widest: Vec<usize> = columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            rows.iter()
                .filter_map(|row| row.get(i))
                .flat_map(|cell| cell.lines())
                .map(|line| line.chars().count())
                .chain([c.name.chars().count()])
                .max()
                .unwrap_or(0)
        })
        .collect();
    for row in rows {
        table.add_row(row);
    }

    for (i, column) in columns.iter().enumerate() {
        let Some(col) = table.column_mut(i) else {
            continue;
        };
        if matches!((width, column.hide_below), (Some(w), Some(min)) if w < min) {
            col.set_constraint(ColumnConstraint::Hidden);
        } else if column.keep {
            let fixed = u16::try_from(widest[i] + PADDING).unwrap_or(u16::MAX);
            col.set_constraint(ColumnConstraint::LowerBoundary(Width::Fixed(fixed)));
        }
    }
    table.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLUMNS: [Column; 5] = [
        Column::keep("ID"),
        Column::keep("STATE"),
        Column::flex("TITLE"),
        Column::flex_below("AUTHOR", TINY),
        Column::keep_below("UPDATED", NARROW),
    ];

    fn rows() -> Vec<Vec<String>> {
        vec![
            vec![
                "#294".into(),
                "OPEN".into(),
                "SCRP-36690 / Align HTTP timeouts with the ALB idle timeout".into(),
                "Gerardo Gomez".into(),
                "2026-09-25".into(),
            ],
            vec![
                "#1".into(),
                "MERGED".into(),
                "short".into(),
                "Dani Castro".into(),
                "2025-08-22".into(),
            ],
        ]
    }

    fn widest_line(text: &str) -> usize {
        text.lines().map(|l| l.chars().count()).max().unwrap_or(0)
    }

    #[test]
    fn draws_rounded_borders() {
        let text = render_at(&COLUMNS, rows(), Some(140));
        assert!(
            text.starts_with('╭') && text.trim_end().ends_with('╯'),
            "{text}"
        );
        assert!(text.contains('│') && text.contains('├'), "{text}");
    }

    #[test]
    fn wraps_long_cells_to_the_terminal_width() {
        let text = render_at(&COLUMNS, rows(), Some(60));
        assert!(widest_line(&text) <= 60, "{text}");
        assert!(
            text.contains("Align HTTP") && text.contains("idle timeout"),
            "{text}"
        );
    }

    #[test]
    fn short_columns_are_never_split() {
        let text = render_at(&COLUMNS, rows(), Some(50));
        assert!(widest_line(&text) <= 50, "{text}");
        for needle in ["#294", "MERGED"] {
            assert!(text.contains(needle), "{needle} was split:\n{text}");
        }
    }

    #[test]
    fn drops_columns_on_narrow_terminals() {
        let wide = render_at(&COLUMNS, rows(), Some(120));
        assert!(
            wide.contains("AUTHOR") && wide.contains("UPDATED"),
            "{wide}"
        );

        let narrow = render_at(&COLUMNS, rows(), Some(99));
        assert!(
            narrow.contains("AUTHOR") && !narrow.contains("UPDATED"),
            "{narrow}"
        );

        let tiny = render_at(&COLUMNS, rows(), Some(79));
        assert!(
            !tiny.contains("AUTHOR") && !tiny.contains("UPDATED"),
            "{tiny}"
        );
        assert!(tiny.contains("TITLE") && tiny.contains("STATE"), "{tiny}");
    }

    #[test]
    fn never_overflows_the_terminal_at_any_width() {
        let mut rows = rows();
        rows.push(vec![
            "#123".into(),
            "OPEN".into(),
            "feature/SCRP-17446-update-labels-for-expire-on-date".into(),
            "Mariano Araoz".into(),
            "2025-12-03".into(),
        ]);
        for width in 50..=140u16 {
            let text = render_at(&COLUMNS, rows.clone(), Some(width));
            assert!(
                widest_line(&text) <= usize::from(width),
                "overflow at {width}:\n{text}"
            );
        }
    }

    #[test]
    fn keeps_every_column_without_a_terminal() {
        let text = render_at(&COLUMNS, rows(), None);
        assert!(
            text.contains("AUTHOR") && text.contains("UPDATED"),
            "{text}"
        );
        assert!(text.contains("SCRP-36690 / Align HTTP timeouts with the ALB idle timeout"));
    }
}
