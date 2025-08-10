use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::{SinkExt, StreamExt};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame, Terminal,
};
use std::{
    env,
    error::Error,
    io,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

#[derive(Debug, Clone, PartialEq)]
enum AppState {
    Welcome,
    Connecting,
    Connected,
    Disconnected,
}

#[derive(Debug)]
pub enum AppEvent {
    SendWord(String),
    Connect,
    Disconnect,
    Quit,
}

#[derive(Debug)]
struct App {
    state: AppState,
    current_input: String,
    sentence: String,
    connection_status: String,
    users_count: usize,
    typing_speed: f64,
    start_time: Option<Instant>,
    chars_typed: usize,
    error_message: Option<String>,
    server_url: String,
    should_quit: bool,
    show_help: bool,
}

impl Default for App {
    fn default() -> App {
        App {
            state: AppState::Welcome,
            current_input: String::new(),
            sentence: String::new(),
            connection_status: "Not connected".to_string(),
            users_count: 1,
            typing_speed: 0.0,
            start_time: None,
            chars_typed: 0,
            error_message: None,
            server_url: "ws://127.0.0.1:9001".to_string(),
            should_quit: false,
            show_help: false,
        }
    }
}

impl App {
    fn new(server_url: String) -> App {
        App {
            state: AppState::Welcome,
            current_input: String::new(),
            sentence: String::new(),
            connection_status: "Not connected".to_string(),
            users_count: 1,
            typing_speed: 0.0,
            start_time: None,
            chars_typed: 0,
            error_message: None,
            server_url,
            should_quit: false,
            show_help: false,
        }
    }
}

impl App {
    fn connect(&mut self) {
        self.state = AppState::Connecting;
        self.connection_status = "Connecting...".to_string();
        self.error_message = None;
    }

    fn set_connected(&mut self) {
        self.state = AppState::Connected;
        self.connection_status = "Connected".to_string();
        self.start_time = Some(Instant::now());
    }

    fn set_disconnected(&mut self, error: Option<String>) {
        self.state = AppState::Disconnected;
        self.connection_status = "Disconnected".to_string();
        if let Some(err) = error {
            self.error_message = Some(err);
        }
    }

    fn send_word(&mut self) -> Option<String> {
        if !self.current_input.trim().is_empty() {
            let word = self.current_input.trim().to_string();
            self.current_input.clear();
            self.chars_typed += word.len() + 1;
            self.update_typing_speed();
            Some(word)
        } else {
            None
        }
    }

    fn update_typing_speed(&mut self) {
        if let Some(start) = self.start_time {
            let elapsed = start.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                let words = self.chars_typed as f64 / 5.0;
                self.typing_speed = (words / elapsed) * 60.0;
            }
        }
    }

    fn update_sentence(&mut self, new_sentence: String) {
        self.sentence = new_sentence;
    }

    fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    let server_ip = if args.len() > 1 {
        &args[1]
    } else {
        "127.0.0.1"
    };

    // Validate IP format (basic check)
    if !is_valid_ip_or_hostname(server_ip) {
        eprintln!("Error: Invalid IP address or hostname: {}", server_ip);
        eprintln!("Usage: {} [IP_ADDRESS|HOSTNAME]", args[0]);
        eprintln!("Example: {} 192.168.1.100", args[0]);
        std::process::exit(1);
    }

    let server_url = format!("ws://{}:9001", server_ip);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = Arc::new(Mutex::new(App::new(server_url)));
    let (event_tx, event_rx) = mpsc::unbounded_channel::<AppEvent>();

    // Spawn WebSocket client task
    let app_clone = Arc::clone(&app);
    let ws_handle = tokio::spawn(async move {
        run_websocket_client(app_clone, event_rx).await;
    });

    let res = run_app(&mut terminal, app, event_tx).await;

    // Cleanup
    ws_handle.abort();
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

async fn run_websocket_client(
    app: Arc<Mutex<App>>,
    mut event_rx: mpsc::UnboundedReceiver<AppEvent>,
) {
    loop {
        // Wait for connect event
        while let Some(event) = event_rx.recv().await {
            match event {
                AppEvent::Connect => {
                    let url = {
                        let app_lock = app.lock().unwrap();
                        app_lock.server_url.clone()
                    };

                    // Attempt connection
                    match connect_async(&url).await {
                        Ok((ws_stream, _)) => {
                            {
                                let mut app_lock = app.lock().unwrap();
                                app_lock.set_connected();
                            }

                            let (mut write, mut read) = ws_stream.split();

                            // Handle the WebSocket connection
                            loop {
                                tokio::select! {
                                    // Handle incoming WebSocket messages
                                    msg_result = read.next() => {
                                        match msg_result {
                                            Some(Ok(msg)) => {
                                                if let Ok(text) = msg.to_text() {
                                                    let mut app_lock = app.lock().unwrap();
                                                    app_lock.update_sentence(text.to_string());
                                                }
                                            }
                                            Some(Err(_)) | None => {
                                                let mut app_lock = app.lock().unwrap();
                                                app_lock.set_disconnected(Some("Connection lost".to_string()));
                                                break;
                                            }
                                        }
                                    }

                                    // Handle outgoing events
                                    event = event_rx.recv() => {
                                        match event {
                                            Some(AppEvent::SendWord(word)) => {
                                                if write.send(Message::Text(word)).await.is_err() {
                                                    let mut app_lock = app.lock().unwrap();
                                                    app_lock.set_disconnected(Some("Failed to send message".to_string()));
                                                    break;
                                                }
                                            }
                                            Some(AppEvent::Disconnect) => {
                                                break;
                                            }
                                            Some(AppEvent::Quit) => {
                                                return;
                                            }
                                            Some(AppEvent::Connect) => {
                                                // Already connected, ignore
                                            }
                                            None => break,
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            let mut app_lock = app.lock().unwrap();
                            app_lock.set_disconnected(Some(format!("Connection failed: {}", e)));
                        }
                    }
                }
                AppEvent::Quit => return,
                _ => {} // Ignore other events when not connected
            }
        }
    }
}

async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: Arc<Mutex<App>>,
    event_tx: mpsc::UnboundedSender<AppEvent>,
) -> io::Result<()> {
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(250);

    loop {
        let should_quit = {
            let app_lock = app.lock().unwrap();
            app_lock.should_quit
        };

        if should_quit {
            let _ = event_tx.send(AppEvent::Quit);
            break;
        }

        terminal.draw(|f| ui(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key_event(key.code, &app, &event_tx).await;
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            let mut app_lock = app.lock().unwrap();
            app_lock.update_typing_speed();
            drop(app_lock);
            last_tick = Instant::now();
        }
    }

    Ok(())
}

async fn handle_key_event(
    key: KeyCode,
    app: &Arc<Mutex<App>>,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
) {
    let mut app_lock = app.lock().unwrap();

    match app_lock.state {
        AppState::Welcome => match key {
            KeyCode::Enter => {
                app_lock.connect();
                drop(app_lock);
                let _ = event_tx.send(AppEvent::Connect);
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                app_lock.should_quit = true;
            }
            KeyCode::Char('h') | KeyCode::Char('H') => {
                app_lock.toggle_help();
            }
            _ => {}
        },
        AppState::Connected => match key {
            KeyCode::Char(c) => {
                app_lock.current_input.push(c);
            }
            KeyCode::Backspace => {
                app_lock.current_input.pop();
            }
            KeyCode::Enter => {
                if let Some(word) = app_lock.send_word() {
                    drop(app_lock);
                    let _ = event_tx.send(AppEvent::SendWord(word));
                }
            }
            KeyCode::Esc => {
                app_lock.state = AppState::Welcome;
                app_lock.current_input.clear();
                drop(app_lock);
                let _ = event_tx.send(AppEvent::Disconnect);
            }
            KeyCode::F(1) => {
                app_lock.toggle_help();
            }
            _ => {}
        },
        AppState::Connecting => match key {
            KeyCode::Esc => {
                app_lock.state = AppState::Welcome;
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                app_lock.should_quit = true;
            }
            _ => {}
        },
        AppState::Disconnected => match key {
            KeyCode::Enter => {
                app_lock.connect();
                drop(app_lock);
                let _ = event_tx.send(AppEvent::Connect);
            }
            KeyCode::Esc => {
                app_lock.state = AppState::Welcome;
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                app_lock.should_quit = true;
            }
            _ => {}
        },
    }
}

fn ui(f: &mut Frame, app: &Arc<Mutex<App>>) {
    let app_lock = app.lock().unwrap();

    match app_lock.state {
        AppState::Welcome => draw_welcome_screen(f, &app_lock),
        AppState::Connecting => draw_connecting_screen(f, &app_lock),
        AppState::Connected => draw_game_screen(f, &app_lock),
        AppState::Disconnected => draw_disconnected_screen(f, &app_lock),
    }

    if app_lock.show_help {
        draw_help_popup(f);
    }
}

fn draw_welcome_screen(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(f.size());

    // Title
    let title = Paragraph::new("Chaos Type")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Main content
    let welcome_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Welcome to ", Style::default().fg(Color::White)),
            Span::styled(
                "Chaos Type",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("!", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from("Connect with friends and build sentences together in real-time!"),
        Line::from(""),
        Line::from(vec![
            Span::styled("📝 ", Style::default().fg(Color::Green)),
            Span::styled(
                "Type words and watch as others contribute",
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(vec![
            Span::styled("⚡ ", Style::default().fg(Color::Yellow)),
            Span::styled(
                "See your typing speed in real-time",
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Server: ", Style::default().fg(Color::White)),
            Span::styled(
                &app.server_url,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::White)),
            Span::styled(
                "ENTER",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " to connect to the server",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::White)),
            Span::styled(
                "H",
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" for help", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::White)),
            Span::styled(
                "Q",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to quit", Style::default().fg(Color::White)),
        ]),
    ];

    let welcome = Paragraph::new(welcome_text)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        );
    f.render_widget(welcome, chunks[1]);

    // Footer
    let footer = Paragraph::new(format!("Server: {}", app.server_url))
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

fn draw_connecting_screen(f: &mut Frame, _app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0)])
        .split(f.size());

    let connecting = Paragraph::new(vec![
        Line::from(""),
        Line::from("🔄 Connecting to server..."),
        Line::from(""),
        Line::from("Please wait while we establish the connection."),
        Line::from(""),
        Line::from("Press ESC to cancel"),
    ])
    .style(Style::default().fg(Color::Yellow))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL).title("Connecting"));

    f.render_widget(connecting, chunks[0]);
}

fn draw_game_screen(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(5),    // Sentence display
            Constraint::Length(3), // Input
            Constraint::Length(4), // Stats
        ])
        .split(f.size());

    // Header with connection status
    let header = Paragraph::new(format!(
        "🎮 Collaborative Typing Game | Status: {} | Speed: {:.1} WPM",
        app.connection_status, app.typing_speed
    ))
    .style(
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Sentence display
    let sentence_text = if app.sentence.is_empty() {
        "Start typing to begin the collaborative sentence...".to_string()
    } else {
        app.sentence.clone()
    };

    let sentence = Paragraph::new(sentence_text)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("📝 Current Sentence")
                .border_style(Style::default().fg(Color::Blue)),
        );
    f.render_widget(sentence, chunks[1]);

    // Input field
    let input = Paragraph::new(app.current_input.clone())
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("✍️  Your Word (Press ENTER to send)")
                .border_style(Style::default().fg(Color::Green)),
        );
    f.render_widget(input, chunks[2]);

    // Stats
    let stats_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[3]);

    let stats_left = Paragraph::new(vec![
        Line::from(format!("Characters typed: {}", app.chars_typed)),
        Line::from(format!("Active users: {}", app.users_count)),
    ])
    .style(Style::default().fg(Color::Cyan))
    .block(Block::default().borders(Borders::ALL).title("📊 Stats"));
    f.render_widget(stats_left, stats_chunks[0]);

    let help_text = vec![
        Line::from("ESC: Back to menu"),
        Line::from("F1: Toggle help"),
    ];
    let controls = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL).title("🎮 Controls"));
    f.render_widget(controls, stats_chunks[1]);
}

fn draw_disconnected_screen(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0)])
        .split(f.size());

    let mut lines = vec![
        Line::from(""),
        Line::from("❌ Connection Lost"),
        Line::from(""),
    ];

    if let Some(ref error) = app.error_message {
        lines.push(Line::from(format!("Error: {}", error)));
        lines.push(Line::from(""));
    }

    lines.extend([
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::White)),
            Span::styled(
                "ENTER",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to retry connection", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::White)),
            Span::styled(
                "ESC",
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to return to menu", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::White)),
            Span::styled(
                "Q",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to quit", Style::default().fg(Color::White)),
        ]),
    ]);

    let disconnected = Paragraph::new(lines)
        .style(Style::default().fg(Color::Red))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("Disconnected"));

    f.render_widget(disconnected, chunks[0]);
}

fn draw_help_popup(f: &mut Frame) {
    let popup_area = centered_rect(60, 70, f.size());
    f.render_widget(Clear, popup_area);

    let help_text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "Chaos Type - Help",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("🎮 Game Controls:"),
        Line::from("  • Type words and press ENTER to send"),
        Line::from("  • ESC: Return to welcome screen"),
        Line::from("  • Q: Quit application"),
        Line::from("  • H: Toggle this help"),
        Line::from(""),
        Line::from("📝 How to Play:"),
        Line::from("  • Connect to the server"),
        Line::from("  • Type words to contribute to the sentence"),
        Line::from("  • Watch as other players add their words"),
        Line::from("  • Build creative sentences together!"),
        Line::from(""),
        Line::from("📊 Features:"),
        Line::from("  • Real-time collaboration"),
        Line::from("  • Typing speed tracking (WPM)"),
        Line::from("  • Live sentence updates"),
        Line::from("  • Multi-user support"),
        Line::from(""),
        Line::from("🔗 Connection:"),
        Line::from("  • Run with: ./client [IP_ADDRESS]"),
        Line::from("  • Default: 127.0.0.1 (localhost)"),
        Line::from("  • Example: ./client 192.168.1.100"),
        Line::from(""),
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::White)),
            Span::styled(
                "H",
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " again to close this help",
                Style::default().fg(Color::White),
            ),
        ]),
    ];

    let help = Paragraph::new(help_text)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Help")
                .border_style(Style::default().fg(Color::Yellow)),
        );

    f.render_widget(help, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn is_valid_ip_or_hostname(addr: &str) -> bool {
    // Check if it's a valid IPv4 address
    if addr.parse::<std::net::Ipv4Addr>().is_ok() {
        return true;
    }

    // Check if it's a valid IPv6 address
    if addr.parse::<std::net::Ipv6Addr>().is_ok() {
        return true;
    }

    // Basic hostname validation (allows letters, numbers, dots, and hyphens)
    if addr.is_empty() || addr.len() > 253 {
        return false;
    }

    addr.chars()
        .all(|c| c.is_alphanumeric() || c == '.' || c == '-')
        && !addr.starts_with('-')
        && !addr.ends_with('-')
        && !addr.starts_with('.')
        && !addr.ends_with('.')
}
