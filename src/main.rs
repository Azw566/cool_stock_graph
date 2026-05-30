mod app;
mod data;
mod ui;

use anyhow::Result;
use app::{App, InputMode};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use data::Side;
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    io,
    time::{Duration, Instant},
};

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let tick_rate = Duration::from_millis(250);
    let result = run(&mut terminal, &mut app, tick_rate);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    tick_rate: Duration,
) -> Result<()> {
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match app.input_mode {
                    InputMode::OrderEntry => match key.code {
                        KeyCode::Esc               => app.cancel_order_entry(),
                        KeyCode::Enter             => app.submit_order(),
                        KeyCode::Tab | KeyCode::BackTab => app.toggle_order_field(),
                        KeyCode::Backspace         => app.handle_backspace(),
                        KeyCode::Char(c)           => app.handle_char(c),
                        _ => {}
                    },

                    InputMode::SymbolInput => match key.code {
                        KeyCode::Esc       => app.cancel_symbol_input(),
                        KeyCode::Enter     => app.submit_symbol(),
                        KeyCode::Backspace => app.symbol_backspace(),
                        KeyCode::Char(c)   => app.symbol_char(c),
                        _ => {}
                    },

                    InputMode::Normal => match (key.code, key.modifiers) {
                        (KeyCode::Char('q'), _)
                        | (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Ok(()),
                        (KeyCode::Tab,    _) => app.next_tab(),
                        (KeyCode::BackTab,_) => app.prev_tab(),
                        (KeyCode::Up   | KeyCode::Char('k'), _) => app.scroll_up(),
                        (KeyCode::Down | KeyCode::Char('j'), _) => app.scroll_down(),
                        (KeyCode::Char('r'), _) => app.reset_view(),
                        (KeyCode::Char('b'), _) => app.start_order(Side::Long),
                        (KeyCode::Char('s'), _) => app.start_order(Side::Short),
                        (KeyCode::Char('x'), _) => app.cancel_selected_order(),
                        (KeyCode::Char('/'), _) => app.start_symbol_input(),
                        _ => {}
                    },
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }
    }
}
