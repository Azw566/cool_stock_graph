use crate::app::{App, FetchStatus, InputMode, OrderField, TABS};
use crate::data::{OrderStatus, Side};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Axis, Block, Borders, Chart, Dataset, GraphType, List, ListItem, Paragraph, Row,
        Sparkline, Table, Tabs,
    },
};

const GREEN:  Color = Color::Green;
const RED:    Color = Color::Red;
const YELLOW: Color = Color::Yellow;
const CYAN:   Color = Color::Cyan;
const GRAY:   Color = Color::DarkGray;
const WHITE:  Color = Color::White;
const MAGENTA:Color = Color::Magenta;

pub fn draw(f: &mut Frame, app: &App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(f.area());

    draw_header(f, app, root[0]);
    draw_tabs(f, app, root[1]);
    draw_body(f, app, root[2]);
    draw_order_bar(f, app, root[3]);
    draw_footer(f, app, root[4]);
}

// ─── header ──────────────────────────────────────────────────────────────────

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    // Symbol + status
    let (status_str, status_color) = match &app.fetch_status {
        FetchStatus::Loading        => ("● fetching…", YELLOW),
        FetchStatus::Ok             => ("● live",      GREEN),
        FetchStatus::Error(_)       => ("● error",     RED),
    };
    let sym_widget = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {} ", app.symbol),
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        ),
        Span::styled(status_str, Style::default().fg(status_color)),
    ]))
    .block(Block::default().borders(Borders::ALL).title(" Symbol  / "));
    f.render_widget(sym_widget, cols[0]);

    // Price + 24h change
    if app.market.last_price > 1.0 {
        let chg_color = if app.market.change_24h_pct >= 0.0 { GREEN } else { RED };
        let price_w = Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {:.2}", app.market.last_price),
                Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{:+.2}%", app.market.change_24h_pct),
                Style::default().fg(chg_color),
            ),
        ]))
        .block(Block::default().borders(Borders::ALL).title("Price"));
        f.render_widget(price_w, cols[1]);

        let ba_w = Paragraph::new(Line::from(vec![
            Span::styled(format!(" B {:.2}", app.market.bid), Style::default().fg(GREEN)),
            Span::raw("  /  "),
            Span::styled(format!("A {:.2}", app.market.ask), Style::default().fg(RED)),
        ]))
        .block(Block::default().borders(Borders::ALL).title("Bid / Ask"));
        f.render_widget(ba_w, cols[2]);

        let hi_lo = Paragraph::new(Line::from(vec![
            Span::styled(format!(" H {:.2}", app.market.high_24h), Style::default().fg(GREEN)),
            Span::raw("  "),
            Span::styled(format!("L {:.2}", app.market.low_24h), Style::default().fg(RED)),
        ]))
        .block(Block::default().borders(Borders::ALL).title("24h Hi / Lo"));
        f.render_widget(hi_lo, cols[3]);
    } else {
        // Still loading — show placeholder
        let loading = Paragraph::new(" Fetching market data…")
            .style(Style::default().fg(GRAY))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(loading, cols[1]);
        let empty = Block::default().borders(Borders::ALL);
        f.render_widget(empty.clone(), cols[2]);
        f.render_widget(empty, cols[3]);
    }
}

// ─── tabs ─────────────────────────────────────────────────────────────────────

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = TABS
        .iter()
        .enumerate()
        .map(|(i, &t)| {
            if i == 4 && !app.open_orders.is_empty() {
                Line::from(format!("{} ({})", t, app.open_orders.len()))
            } else {
                Line::from(t)
            }
        })
        .collect();

    let tabs = Tabs::new(titles)
        .select(app.tab_index)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .fg(CYAN)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )
        .divider("|");
    f.render_widget(tabs, area);
}

// ─── body dispatch ────────────────────────────────────────────────────────────

fn draw_body(f: &mut Frame, app: &App, area: Rect) {
    if app.candles.is_empty() {
        let msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Fetching data from Yahoo Finance…",
                Style::default().fg(YELLOW),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Press / to change symbol  (e.g. AAPL  TSLA  BTC-USD  NVDA)",
                Style::default().fg(GRAY),
            )),
        ])
        .block(Block::default().borders(Borders::ALL));
        f.render_widget(msg, area);
        return;
    }

    match app.tab_index {
        0 => draw_chart_tab(f, app, area),
        1 => draw_order_book_tab(f, app, area),
        2 => draw_positions_tab(f, app, area),
        3 => draw_trade_log_tab(f, app, area),
        4 => draw_orders_tab(f, app, area),
        _ => {}
    }
}

// ─── Chart tab ───────────────────────────────────────────────────────────────

fn draw_chart_tab(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
        .split(area);

    // Top row: price chart | volume profile
    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
        .split(rows[0]);

    draw_price_chart(f, app, top[0]);
    draw_vol_profile(f, app, top[1]);
    draw_volume_bars(f, app, rows[1]);
}

fn draw_price_chart(f: &mut Frame, app: &App, area: Rect) {
    let data: Vec<(f64, f64)> = app
        .candles
        .iter()
        .enumerate()
        .map(|(i, c)| (i as f64, c.close))
        .collect();

    let prices: Vec<f64> = app.candles.iter().map(|c| c.close).collect();
    let min_p = prices.iter().cloned().fold(f64::INFINITY,     f64::min);
    let max_p = prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    // Colour the line green if price trended up, red if down
    let trend_color = if prices.last().unwrap_or(&0.0) >= prices.first().unwrap_or(&0.0) {
        GREEN
    } else {
        RED
    };

    let datasets = vec![Dataset::default()
        .name("Close")
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(trend_color))
        .data(&data)];

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .title(format!(
                    " {} — 5m bars ({} candles) ",
                    app.symbol,
                    app.candles.len()
                ))
                .borders(Borders::ALL),
        )
        .x_axis(
            Axis::default()
                .style(Style::default().fg(GRAY))
                .bounds([0.0, data.len() as f64])
                .labels([Line::from("5d ago"), Line::from("now")]),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(GRAY))
                .bounds([min_p * 0.9995, max_p * 1.0005])
                .labels([
                    Line::from(format!("{:.2}", min_p)),
                    Line::from(format!("{:.2}", (min_p + max_p) / 2.0)),
                    Line::from(format!("{:.2}", max_p)),
                ]),
        );
    f.render_widget(chart, area);
}

fn draw_vol_profile(f: &mut Frame, app: &App, area: Rect) {
    if app.volume_profile.is_empty() {
        f.render_widget(
            Block::default().borders(Borders::ALL).title(" Volume Profile "),
            area,
        );
        return;
    }

    let max_vol = app
        .volume_profile
        .iter()
        .map(|b| b.volume)
        .fold(0.0_f64, f64::max);

    let poc_price = app
        .volume_profile
        .iter()
        .max_by(|a, b| a.volume.partial_cmp(&b.volume).unwrap())
        .map(|b| b.price)
        .unwrap_or(0.0);

    // bar width = terminal width minus the 8-char price label
    let bar_w = (area.width.saturating_sub(10) as usize).max(1);

    // buckets are low→high; we render top→bottom so reverse
    let lines: Vec<Line> = app
        .volume_profile
        .iter()
        .rev()
        .map(|bucket| {
            let filled = if max_vol > 0.0 {
                ((bucket.volume / max_vol) * bar_w as f64).round() as usize
            } else {
                0
            };
            let empty = bar_w.saturating_sub(filled);
            let is_poc = (bucket.price - poc_price).abs() < (poc_price * 0.0001);
            let bar_color = if is_poc { YELLOW } else { CYAN };

            Line::from(vec![
                Span::styled(
                    format!("{:7.2} ", bucket.price),
                    Style::default().fg(GRAY),
                ),
                Span::styled(
                    "█".repeat(filled),
                    Style::default().fg(bar_color),
                ),
                Span::styled(
                    "░".repeat(empty),
                    Style::default().fg(Color::DarkGray),
                ),
            ])
        })
        .collect();

    let para = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Volume Profile ")
            .title_style(Style::default().fg(CYAN)),
    );
    f.render_widget(para, area);
}

fn draw_volume_bars(f: &mut Frame, app: &App, area: Rect) {
    let vols: Vec<u64> = app
        .candles
        .iter()
        .map(|c| c.volume as u64)
        .collect();

    let max_v = vols.iter().cloned().max().unwrap_or(1);

    let sparkline = Sparkline::default()
        .data(&vols)
        .max(max_v)
        .style(Style::default().fg(MAGENTA))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(
                    " Volume per bar  (max {:.0})",
                    max_v as f64
                )),
        );
    f.render_widget(sparkline, area);
}

// ─── Order Book tab ───────────────────────────────────────────────────────────

fn draw_order_book_tab(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let hdr = Row::new(["Price", "Qty"])
        .style(Style::default().fg(YELLOW).add_modifier(Modifier::BOLD));

    let asks: Vec<Row> = app
        .order_book
        .asks
        .iter()
        .map(|l| {
            Row::new(vec![format!("{:.4}", l.price), format!("{:.4}", l.quantity)])
                .style(Style::default().fg(RED))
        })
        .collect();
    f.render_widget(
        Table::new(asks, [Constraint::Percentage(55), Constraint::Percentage(45)])
            .header(hdr.clone())
            .block(Block::default().borders(Borders::ALL).title(" Asks ")),
        cols[0],
    );

    let bids: Vec<Row> = app
        .order_book
        .bids
        .iter()
        .map(|l| {
            Row::new(vec![format!("{:.4}", l.price), format!("{:.4}", l.quantity)])
                .style(Style::default().fg(GREEN))
        })
        .collect();
    f.render_widget(
        Table::new(bids, [Constraint::Percentage(55), Constraint::Percentage(45)])
            .header(hdr)
            .block(Block::default().borders(Borders::ALL).title(" Bids ")),
        cols[1],
    );
}

// ─── Positions tab ────────────────────────────────────────────────────────────

fn draw_positions_tab(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(5)])
        .split(area);

    let hdr = Row::new(["Symbol", "Side", "Entry", "Mark", "Qty", "UPnL"])
        .style(Style::default().fg(YELLOW).add_modifier(Modifier::BOLD));

    let table_rows: Vec<Row> = app
        .positions
        .iter()
        .map(|p| {
            let c = if p.unrealized_pnl >= 0.0 { GREEN } else { RED };
            Row::new(vec![
                p.symbol.clone(),
                match p.side { Side::Long => "LONG", Side::Short => "SHORT" }.to_string(),
                format!("{:.2}", p.entry_price),
                format!("{:.2}", p.current_price),
                format!("{:.4}", p.quantity),
                format!("{:+.2}", p.unrealized_pnl),
            ])
            .style(Style::default().fg(c))
        })
        .collect();

    let note = if app.positions.is_empty() {
        "No positions. Use b/s to place orders."
    } else {
        ""
    };

    f.render_widget(
        Table::new(
            table_rows,
            [
                Constraint::Percentage(20),
                Constraint::Percentage(10),
                Constraint::Percentage(15),
                Constraint::Percentage(15),
                Constraint::Percentage(15),
                Constraint::Percentage(25),
            ],
        )
        .header(hdr)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Open Positions  {}", note)),
        ),
        rows[0],
    );

    let upnl = app.total_unrealized_pnl();
    let rpnl = app.total_realized_pnl();
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("  Unrealized: "),
            Span::styled(
                format!("{:+.2}", upnl),
                Style::default()
                    .fg(if upnl >= 0.0 { GREEN } else { RED })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("    Realized: "),
            Span::styled(
                format!("{:+.2}", rpnl),
                Style::default()
                    .fg(if rpnl >= 0.0 { GREEN } else { RED })
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .block(Block::default().borders(Borders::ALL).title(" Summary ")),
        rows[1],
    );
}

// ─── Trade Log tab ────────────────────────────────────────────────────────────

fn draw_trade_log_tab(f: &mut Frame, app: &App, area: Rect) {
    let vis = area.height.saturating_sub(2) as usize;
    let off = app.scroll_offset.min(app.trade_log.len().saturating_sub(vis));

    let items: Vec<ListItem> = app
        .trade_log
        .iter()
        .skip(off)
        .take(vis)
        .map(|t| {
            let (s, sc) = match t.side {
                Side::Long  => ("BUY ", GREEN),
                Side::Short => ("SELL", RED),
            };
            let pc = if t.pnl >= 0.0 { GREEN } else { RED };
            ListItem::new(Line::from(vec![
                Span::styled(t.timestamp.format("%m-%d %H:%M").to_string(), Style::default().fg(GRAY)),
                Span::raw("  "),
                Span::styled(format!("{:<10}", t.symbol), Style::default().fg(WHITE)),
                Span::styled(s, Style::default().fg(sc).add_modifier(Modifier::BOLD)),
                Span::raw(format!("  {:.4}  qty {:.4}  pnl ", t.price, t.quantity)),
                Span::styled(format!("{:+.2}", t.pnl), Style::default().fg(pc)),
            ]))
        })
        .collect();

    f.render_widget(
        List::new(items).block(Block::default().borders(Borders::ALL).title(format!(
            " Trade Log ({}/{})  ↑↓ scroll  r reset ",
            off,
            app.trade_log.len()
        ))),
        area,
    );
}

// ─── Orders tab ───────────────────────────────────────────────────────────────

fn draw_orders_tab(f: &mut Frame, app: &App, area: Rect) {
    if app.open_orders.is_empty() {
        f.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled("  No open orders.", Style::default().fg(GRAY))),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  b", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
                    Span::raw(" = buy    "),
                    Span::styled("s", Style::default().fg(RED).add_modifier(Modifier::BOLD)),
                    Span::raw(" = sell"),
                ]),
            ])
            .block(Block::default().borders(Borders::ALL).title(" Open Orders ")),
            area,
        );
        return;
    }

    let sel = app.scroll_offset.min(app.open_orders.len().saturating_sub(1));
    let hdr = Row::new(["#", "Symbol", "Side", "Price", "Qty", "Filled", "Status"])
        .style(Style::default().fg(YELLOW).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .open_orders
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let (side_s, c) = match o.side {
                Side::Long  => ("BUY",  GREEN),
                Side::Short => ("SELL", RED),
            };
            let status_s = match o.status {
                OrderStatus::Pending         => "PENDING",
                OrderStatus::PartiallyFilled => "PARTIAL",
                OrderStatus::Filled          => "FILLED",
                OrderStatus::Cancelled       => "CANCELLED",
            };
            let bar = fill_bar(o.fill_pct(), 8);
            let style = if i == sel {
                Style::default().fg(c).add_modifier(Modifier::REVERSED)
            } else {
                Style::default().fg(c)
            };
            Row::new(vec![
                format!("{}", o.id),
                o.symbol.clone(),
                side_s.to_string(),
                format!("{:.2}", o.price),
                format!("{:.4}", o.quantity),
                format!("{} {:.0}%", bar, o.fill_pct()),
                status_s.to_string(),
            ])
            .style(style)
        })
        .collect();

    f.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(4),
                Constraint::Percentage(14),
                Constraint::Length(5),
                Constraint::Percentage(18),
                Constraint::Percentage(12),
                Constraint::Percentage(22),
                Constraint::Percentage(18),
            ],
        )
        .header(hdr)
        .block(Block::default().borders(Borders::ALL).title(format!(
            " Open Orders ({})  ↑↓ select   x = cancel ",
            app.open_orders.len()
        ))),
        area,
    );
}

// ─── Order entry / symbol bar ─────────────────────────────────────────────────

fn draw_order_bar(f: &mut Frame, app: &App, area: Rect) {
    match app.input_mode {
        InputMode::SymbolInput => {
            let line = Line::from(vec![
                Span::raw("  Symbol: "),
                Span::styled(
                    format!("{}│", app.symbol_buf),
                    Style::default()
                        .fg(CYAN)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
                Span::raw("    "),
                Span::styled("Enter", Style::default().fg(GREEN)),
                Span::raw("=fetch   "),
                Span::styled("Esc", Style::default().fg(RED)),
                Span::raw("=cancel"),
            ]);
            f.render_widget(
                Paragraph::new(line).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Change Symbol ")
                        .border_style(Style::default().fg(CYAN)),
                ),
                area,
            );
        }

        InputMode::OrderEntry => {
            if let Some(input) = &app.order_input {
                let (label, color) = match input.side {
                    Side::Long  => (" BUY ORDER ",  GREEN),
                    Side::Short => (" SELL ORDER ", RED),
                };
                let price_active = input.focus == OrderField::Price;
                let line = Line::from(vec![
                    Span::raw("  Price: "),
                    Span::styled(
                        if price_active { format!("{}│", input.price) } else { input.price.clone() },
                        if price_active {
                            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                        } else {
                            Style::default().fg(WHITE)
                        },
                    ),
                    Span::raw("    Qty: "),
                    Span::styled(
                        if !price_active { format!("{}│", input.qty) } else { input.qty.clone() },
                        if !price_active {
                            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                        } else {
                            Style::default().fg(WHITE)
                        },
                    ),
                    Span::raw("    "),
                    Span::styled("Tab", Style::default().fg(GRAY)),
                    Span::raw("=next   "),
                    Span::styled("Enter", Style::default().fg(GREEN)),
                    Span::raw("=place   "),
                    Span::styled("Esc", Style::default().fg(RED)),
                    Span::raw("=cancel"),
                ]);
                f.render_widget(
                    Paragraph::new(line).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(label)
                            .border_style(Style::default().fg(color))
                            .title_style(Style::default().fg(color).add_modifier(Modifier::BOLD)),
                    ),
                    area,
                );
            }
        }

        InputMode::Normal => {
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(35),
                    Constraint::Percentage(30),
                    Constraint::Percentage(35),
                ])
                .split(area);

            let price_ready = app.market.last_price > 1.0;
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        "  [ b ]  BUY ",
                        Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        if price_ready { format!("@ {:.4}  ", app.market.ask) } else { "…  ".into() },
                        Style::default().fg(WHITE),
                    ),
                ]))
                .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(GREEN))),
                cols[0],
            );

            let n = app.open_orders.len();
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    if n == 0 { "  no open orders".to_string() }
                    else { format!("  {} open order{}", n, if n == 1 { "" } else { "s" }) },
                    Style::default().fg(if n == 0 { GRAY } else { YELLOW }),
                )))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL).title(" Orders ")),
                cols[1],
            );

            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        "  [ s ]  SELL ",
                        Style::default().fg(RED).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        if price_ready { format!("@ {:.4}  ", app.market.bid) } else { "…  ".into() },
                        Style::default().fg(WHITE),
                    ),
                ]))
                .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(RED))),
                cols[2],
            );
        }
    }
}

// ─── footer ───────────────────────────────────────────────────────────────────

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let spans: Vec<Span> = match app.input_mode {
        InputMode::SymbolInput => vec![
            Span::styled("Enter ", Style::default().fg(GREEN)), Span::raw("fetch  "),
            Span::styled("Esc ",   Style::default().fg(RED)),   Span::raw("cancel"),
        ],
        InputMode::OrderEntry => vec![
            Span::styled("Tab ",   Style::default().fg(YELLOW)), Span::raw("switch field  "),
            Span::styled("Enter ", Style::default().fg(GREEN)),  Span::raw("place  "),
            Span::styled("Esc ",   Style::default().fg(RED)),    Span::raw("cancel"),
        ],
        InputMode::Normal => vec![
            Span::styled("q ",   Style::default().fg(YELLOW)), Span::raw("quit  "),
            Span::styled("/ ",   Style::default().fg(CYAN)),   Span::raw("symbol  "),
            Span::styled("Tab ", Style::default().fg(YELLOW)), Span::raw("switch tab  "),
            Span::styled("↑↓ ",  Style::default().fg(YELLOW)), Span::raw("scroll  "),
            Span::styled("b ",   Style::default().fg(GREEN)),  Span::raw("buy  "),
            Span::styled("s ",   Style::default().fg(RED)),    Span::raw("sell  "),
            Span::styled("x ",   Style::default().fg(YELLOW)), Span::raw("cancel order"),
        ],
    };

    f.render_widget(
        Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .style(Style::default().fg(GRAY)),
        area,
    );
}

// ─── helpers ──────────────────────────────────────────────────────────────────

fn fill_bar(pct: f64, w: usize) -> String {
    let f = ((pct / 100.0) * w as f64).round() as usize;
    format!("[{}{}]", "█".repeat(f), "░".repeat(w.saturating_sub(f)))
}
