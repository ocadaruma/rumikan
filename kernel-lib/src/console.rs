use core::fmt::{Arguments, Write};

use crate::graphics::fonts::Font;
use crate::graphics::{CharVec, FrameBuffer, PixelColor};

static mut CONSOLE: Option<Console> = None;

pub fn init_global_console(console: Console) {
    unsafe { CONSOLE = Some(console) };
}

pub struct Console {
    buffer: FrameBuffer,
    bg_color: PixelColor,
    fg_color: PixelColor,
    history: [[char; Console::COLS]; Console::ROWS],
    cursor_row: usize,
    cursor_col: usize,
    row_head: usize,
}

impl Console {
    const ROWS: usize = 25;
    const COLS: usize = 80;

    pub fn new(buffer: FrameBuffer, bg_color: PixelColor, fg_color: PixelColor) -> Console {
        let mut console = Console {
            buffer,
            bg_color,
            fg_color,
            history: [[0 as char; Console::COLS]; Console::ROWS],
            cursor_row: 0,
            cursor_col: 0,
            row_head: 0,
        };
        console.fill_bg();
        console
    }

    fn fill_bg(&mut self) {
        for x in 0..(Console::COLS * Font::WIDTH) {
            for y in 0..(Console::ROWS * Font::HEIGHT) {
                self.buffer.write_pixel(x, y, self.bg_color);
            }
        }
    }

    /// Calculate the current row cursor from row_head based on mod ROWS
    fn logical_row(&self) -> usize {
        (self.cursor_row + Console::ROWS - self.row_head) % Console::ROWS
    }

    fn new_line(&mut self) {
        // reached last row. need scroll
        if self.logical_row() == Console::ROWS - 1 {
            self.fill_bg();
            self.row_head = (self.row_head + 1) % Console::ROWS;
            self.cursor_row = (self.cursor_row + 1) % Console::ROWS;

            for (console_row, history_row) in
                (self.row_head..(self.row_head + Console::ROWS - 1)).enumerate()
            {
                let history_row_mod = history_row % Console::ROWS;
                for (col, &c) in self.history[history_row_mod].iter().enumerate() {
                    self.buffer.write_char(
                        col * Font::WIDTH,
                        console_row * Font::HEIGHT,
                        c,
                        self.fg_color,
                    );
                }
            }
        } else {
            self.cursor_row = (self.cursor_row + 1) % Console::ROWS;
        }

        self.cursor_col = 0;
    }

    pub fn print(&mut self, args: Arguments) {
        let mut v = CharVec::new();
        let truncated_message = if v.write_fmt(args).is_ok() {
            ""
        } else {
            "...(truncated)"
        };
        for c in v
            .as_slice()
            .iter()
            .copied()
            .chain(truncated_message.chars())
        {
            if c == '\n' {
                self.new_line();
                continue;
            }
            if self.cursor_col == Console::COLS {
                self.new_line();
            }
            self.history[self.cursor_row][self.cursor_col] = c;
            self.buffer.write_char(
                self.cursor_col * Font::WIDTH,
                self.logical_row() * Font::HEIGHT,
                c,
                self.fg_color,
            );
            self.cursor_col += 1;
        }
    }
}

pub fn _print(args: Arguments) {
    unsafe {
        CONSOLE.as_mut().unwrap().print(args);
    }
}
