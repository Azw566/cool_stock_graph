# Trading TUI — Architecture & Linking Guide

## Overview

This project is a terminal trading dashboard built with **Ratatui** (a Rust TUI framework).
It renders live-updating market data, order books, positions, and trade history entirely
in the terminal — no browser or GPU required.

---

## Module Map

```
src/
├── main.rs      — Entry point, event loop, terminal setup/teardown
├── app.rs       — Central application state; owns all data + tick logic
├── data.rs      — Domain types + simulation helpers (candles, order book, …)
└── ui.rs        — Pure render functions; reads App, writes to Frame
```

### How the pieces link together

```
main.rs
  │
  ├─ creates  App  (app.rs)
  │     ├─ owns  Vec<Candle>, MarketData, OrderBook, Vec<Position>, …  (data.rs types)
  │     └─ mutates itself on every tick via  on_tick()
  │
  └─ drives the render loop
        └─ calls  ui::draw(frame, &app)  (ui.rs)
              └─ reads App fields, renders Ratatui widgets
```

**Data never flows from `ui.rs` back to `app.rs`** — the UI is purely a view.
All mutations live in `app.rs`.

---

## Key Ratatui Concepts Used

| Concept | Where | Purpose |
|---|---|---|
| `Terminal::draw(closure)` | `main.rs` | Enters a double-buffered render pass |
| `Frame` | `ui.rs` | Holds the back-buffer; widgets are rendered into it |
| `Layout` | `ui.rs` | Splits a `Rect` into sub-areas (horizontal / vertical) |
| `Block` | `ui.rs` | Decorates any widget with a border and title |
| `Chart` / `Dataset` | `ui.rs` — chart tab | Plots the price line and equity curve |
| `Table` | `ui.rs` — order book & positions | Grid of labelled rows |
| `List` | `ui.rs` — trade log | Scrollable list of styled lines |
| `Tabs` | `ui.rs` | Top navigation bar |
| `Paragraph` | `ui.rs` | Header stats and footer hint bar |

---

## Adding a Real Data Feed

The `App` struct in `app.rs` is the only place you need to change.

1. **Replace `simulate_tick`** in `data.rs` with a call to your exchange WebSocket or REST API.
   A good pattern is an `mpsc` channel fed by a background Tokio task:

   ```rust
   // In main.rs, before the render loop:
   let (tx, rx) = std::sync::mpsc::channel::<MarketData>();
   std::thread::spawn(move || { /* fetch and tx.send(…) */ });

   // In App:
   pub rx: std::sync::mpsc::Receiver<MarketData>,
   // In on_tick():
   if let Ok(md) = self.rx.try_recv() { self.market = md; }
   ```

2. **Replace `generate_order_book`** with a parsed Level-2 snapshot from your feed.

3. **Replace the static `trade_log`** with real fill events pushed from your order manager.

---

## Adding a New Tab

1. Add a label to the `TABS` constant in `app.rs`.
2. Add any new state fields to `App`.
3. Add a `draw_<name>_tab(f, app, area)` function in `ui.rs`.
4. Match on the new index in `draw_body()` in `ui.rs`.

That's the entire extension surface — no registration, no trait objects required.

---

## Running

```bash
# Install Rust if needed:
# https://rustup.rs  (one-liner: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh)

cargo run            # debug build, live reload-friendly
cargo run --release  # optimised, recommended for real trading use
```

Press **q** to quit cleanly (raw-mode terminal is always restored).

---

## Dependency Rationale

| Crate | Role |
|---|---|
| `ratatui` | TUI framework — layout, widgets, double-buffered rendering |
| `crossterm` | Cross-platform terminal backend for ratatui (raw mode, events) |
| `anyhow` | Ergonomic error propagation in `main` |
| `rand` | Price simulation (replace with real feed in production) |
| `chrono` | Timestamps on candles and trades |
| `serde` | Derives on domain types for future JSON serialisation / persistence |
