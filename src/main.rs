use std::io::{self, Stdout};
use std::panic;
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;

use orbitpixel::app::{App, AppCommand, AppEvent, RenderMode};
use orbitpixel::html::Document;
use orbitpixel::network::HttpClient;
use orbitpixel::ui;

#[derive(Parser, Debug)]
#[command(
    name = "orbitpixel",
    version,
    about = "OrbitPixel Browser - Terminal-native web browser"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<ModeCommand>,

    /// Initial URL to open
    url: Option<String>,
}

#[derive(Subcommand, Debug, Clone)]
enum ModeCommand {
    /// Traditional plain text terminal browsing
    Text { url: Option<String> },
    /// Rich ANSI colors, borders, and styled layout
    Color { url: Option<String> },
    /// Graphics-capable pixel rendering mode
    Pixel { url: Option<String> },
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) {
    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    );
    let _ = terminal.show_cursor();
}

fn install_panic_hook() {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
        original_hook(panic_info);
    }));
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let (mode, initial_url) = match cli.command {
        Some(ModeCommand::Text { url }) => (RenderMode::Text, url.or(cli.url)),
        Some(ModeCommand::Color { url }) => (RenderMode::Color, url.or(cli.url)),
        Some(ModeCommand::Pixel { url }) => (RenderMode::Pixel, url.or(cli.url)),
        None => (RenderMode::Color, cli.url),
    };

    install_panic_hook();
    let mut terminal = setup_terminal()?;

    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<AppCommand>();
    let (evt_tx, mut evt_rx) = mpsc::unbounded_channel::<AppEvent>();
    let client = HttpClient::new()?;

    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                AppCommand::FetchUrl {
                    tab_id,
                    request_id,
                    raw_url,
                    push_history,
                } => {
                    let client_clone = client.clone();
                    let evt_tx_clone = evt_tx.clone();
                    tokio::spawn(async move {
                        match client_clone.fetch_html(&raw_url).await {
                            Ok(page) => {
                                let doc = Document::parse(&page.html, &page.final_url);
                                let _ = evt_tx_clone.send(AppEvent::PageLoaded {
                                    tab_id,
                                    request_id,
                                    final_url: page.final_url,
                                    status_code: page.status_code,
                                    document: doc,
                                    push_history,
                                });
                            }
                            Err(err) => {
                                let _ = evt_tx_clone.send(AppEvent::PageError {
                                    tab_id,
                                    request_id,
                                    url: raw_url,
                                    error: err.to_string(),
                                });
                            }
                        }
                    });
                }
            }
        }
    });

    let mut app = App::new(mode, cmd_tx);
    if let Some(url) = initial_url {
        app.navigate_active(&url, true);
    }

    let run_result = run_event_loop(&mut terminal, &mut app, &mut evt_rx);
    restore_terminal(&mut terminal);
    run_result
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    evt_rx: &mut mpsc::UnboundedReceiver<AppEvent>,
) -> Result<()> {
    loop {
        while let Ok(bg_event) = evt_rx.try_recv() {
            app.handle_background_event(bg_event);
        }

        let size = terminal.size()?;
        let viewport_width = size.width.saturating_sub(2).max(20);
        let viewport_height = size.height.saturating_sub(6).max(4);
        app.ensure_layout(viewport_width, viewport_height);

        terminal.draw(|frame| ui::render(frame, app))?;

        if app.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(25))? {
            match event::read()? {
                Event::Key(key) => app.handle_key(key),
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                Event::Resize(_, _) => app.invalidate_all_layouts(),
                _ => {}
            }
        }
    }

    Ok(())
}
