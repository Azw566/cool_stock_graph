# trading_tui

A personal terminal trading dashboard written in Rust, built as the visualization layer for my quant research stack.
Real market data is pulled from Yahoo Finance; the interface is designed to eventually display signals, backtests, and live feeds from other projects in this workspace.

---

## Why this exists

Most of my quant work produces data — price series, signals, P&L curves, order flow — that lives in CSVs, databases, or Python notebooks.
This TUI is the front-end that ties it all together: one terminal window that can show live prices, the output of a running strategy, or a replay of a backtest without leaving the keyboard.

---

## Quick start

```powershell
# 1. Install Rust (skip if already installed)
winget install Rustlang.Rustup

# 2. Open a new terminal so cargo is on PATH, then:
cd C:\Users\telem\quant\cool_stock_graph
cargo run --release
```

---

## Launch

**Pre-built binary (after first `cargo build --release`)**
```
cd C:\Users\telem\quant\cool_stock_graph
target\release\trading_tui.exe
```

**Rebuild and run**
```
cd C:\Users\telem\quant\cool_stock_graph
cargo run --release
```

`cargo run` (no `--release`) compiles faster but runs slower — fine for development.

> If `cargo` is not found, open a new terminal after installing Rust; the PATH update only takes effect in fresh sessions.
> Install Rust: `winget install Rustlang.Rustup`

---

## Controls

| Key | Action |
|---|---|
| `/` | Change symbol (type e.g. `AAPL`, `BTC-USD`, `NVDA`, then `Enter`) |
| `Tab` / `Shift+Tab` | Switch between tabs |
| `↑` / `↓` or `k` / `j` | Scroll / select |
| `b` | Open buy order form |
| `s` | Open sell order form |
| `x` | Cancel selected order (Orders tab) |
| `r` | Reset scroll position |
| `q` or `Ctrl+C` | Quit |

**In order / symbol forms:** `Tab` = next field — `Enter` = confirm — `Esc` = cancel

---

## Tabs

| Tab | Content |
|---|---|
| **Chart** | 5-min price line + Volume Profile sidebar (yellow bar = Point of Control) + volume sparkline |
| **Order Book** | Simulated L2 bids & asks around the live mid price |
| **Positions** | Open positions with live mark-to-market P&L |
| **Trade Log** | History of filled orders |
| **Orders** | Active open orders with real-time fill progress bars |

Data refreshes from Yahoo Finance every 5 minutes in a background thread.
Between fetches, the last price micro-fluctuates to keep the display lively.

---

## Project structure

```
src/
├── main.rs   — terminal setup, event loop, key dispatch
├── app.rs    — all state, background fetch thread, tick logic
├── data.rs   — Yahoo Finance HTTP fetch, volume profile, domain types
└── ui.rs     — pure render functions (one per tab + order bar + header)
```

See `ARCHITECTURE.md` for a detailed linking guide and instructions on wiring in a real data feed.

---

## Linking to other quant projects

This section will grow as the workspace expands.
The integration point is always `app.rs` — swap or augment `on_tick()` and the fetch thread.

### Planned connections

<!-- Add links here as projects are created -->

| Project | Data it will provide | Integration point |
|---|---|---|
| _backtester_ | equity curve, trade log replay | replace `equity_curve` vec in `App` |
| _signal engine_ | buy/sell signals with confidence scores | overlay on price chart as markers |
| _order manager_ | live fills from a broker API | replace simulated fill loop in `on_tick()` |
| _market data feed_ | real-time tick stream (WebSocket) | replace `fetch_yahoo` background thread |

### How to feed external data in

The simplest bridge is an `mpsc` channel — the same pattern already used for the Yahoo Finance fetch:

```rust
// Anywhere in your other project:
let (tx, rx) = std::sync::mpsc::channel::<MySignal>();

// Pass rx into App::new(), store it, check in on_tick():
if let Ok(signal) = self.signal_rx.try_recv() {
    // update app state
}
```

For file-based feeds (CSV, SQLite), a background thread that tails the file and sends rows over a channel works without any network code.

---

## Dependencies

| Crate | Role |
|---|---|
| `ratatui` | TUI framework — layout, widgets, double-buffered rendering |
| `crossterm` | Cross-platform terminal backend (raw mode, key events) |
| `ureq` | Blocking HTTP client for Yahoo Finance API calls |
| `serde_json` | JSON parsing of API responses |
| `chrono` | Timestamps on candles and trades |
| `rand` | Order book simulation and micro-fluctuation between fetches |
| `serde` | Derive macros on domain types for future serialisation |
| `anyhow` | Error propagation in `main` |
