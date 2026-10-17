//! The cast of the startup show, drawn in Braille. Each grid is dots, `#`
//! lit and `.` empty: four dot rows make one terminal line, two dot columns
//! one cell.

use ratatui::buffer::Buffer;
use ratatui::style::Style;

use super::fx::put;

pub const RUN_A: &str = r"
..................##..........
...................###........
...............########.......
................#..#####......
.................##..#####....
.......##............######...
......#########.....########..
.......#####################..
.......#####################..
.....#.###################....
...#.###################......
...#####################......
..#########..##############...
###.#...............#########.
##.##................#####....
........................###...
";

pub const RUN_B: &str = r"
..........#.......
.......#####......
.......#..###.....
...........##.....
...........####...
...######..#####..
..###############.
################..
#############.....
.############.....
..##########......
....###.##........
....######........
.....####.........
.....####.........
........##........
";

// Where each dot of a cell sits, and its bit in the Braille block.
const DOTS: [(usize, usize, u32); 8] = [
    (0, 0, 0x01),
    (0, 1, 0x02),
    (0, 2, 0x04),
    (1, 0, 0x08),
    (1, 1, 0x10),
    (1, 2, 0x20),
    (0, 3, 0x40),
    (1, 3, 0x80),
];

fn rows(grid: &str) -> Vec<&[u8]> {
    grid.lines()
        .filter(|line| !line.is_empty())
        .map(str::as_bytes)
        .collect()
}

/// Width in cells.
pub fn width(grid: &str) -> i32 {
    rows(grid)
        .iter()
        .map(|row| row.len())
        .max()
        .unwrap_or(0)
        .div_ceil(2) as i32
}

/// Draws the grid with its top left cell at `x`, `y`. Empty cells keep
/// what is behind them.
pub fn draw(buf: &mut Buffer, x: i32, y: i32, grid: &str, style: Style) {
    let rows = rows(grid);
    let lit = |col: usize, row: usize| {
        rows.get(row)
            .and_then(|line| line.get(col))
            .is_some_and(|&dot| dot != b'.')
    };
    for cy in 0..rows.len().div_ceil(4) {
        for cx in 0..width(grid) as usize {
            let bits = DOTS
                .iter()
                .filter(|&&(dx, dy, _)| lit(cx * 2 + dx, cy * 4 + dy))
                .fold(0, |bits, &(_, _, bit)| bits | bit);
            if let Some(ch) = char::from_u32(0x2800 + bits).filter(|_| bits != 0) {
                put(buf, x + cx as i32, y + cy as i32, &ch.to_string(), style);
            }
        }
    }
}
