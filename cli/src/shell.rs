use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use crossterm::{
	event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
	execute,
	terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use puppyagent_core::{PuppyPeer, State};
use ratatui::{
	Terminal,
	backend::CrosstermBackend,
	layout::{Constraint, Direction, Layout},
	style::{Color, Modifier, Style},
	widgets::{
		Block, Borders, List, ListItem, ListState, Paragraph, Wrap,
		canvas::{Canvas, Line, Points},
	},
};

enum Mode {
	Menu,
	Peers(PeersView),
	CreateUser(CreateUserForm),
	PeersGraph(GraphView),
}

struct GraphView {
	peers: Vec<PeerNode>,
	selected: usize,
}

struct PeerNode {
	id: String,
	// Precomputed polar angle for layout (radians)
	angle: f64,
}

impl GraphView {
	fn new() -> Self {
		let base = sample_peers();
		let count = base.len().max(1);
		let peers = base
			.into_iter()
			.enumerate()
			.map(|(i, p)| PeerNode {
				id: p.id,
				angle: (i as f64) * (std::f64::consts::TAU / count as f64),
			})
			.collect();
		Self { peers, selected: 0 }
	}
	fn next(&mut self) {
		if self.peers.is_empty() {
			return;
		}
		self.selected = (self.selected + 1) % self.peers.len();
	}
	fn previous(&mut self) {
		if self.peers.is_empty() {
			return;
		}
		if self.selected == 0 {
			self.selected = self.peers.len() - 1;
		} else {
			self.selected -= 1;
		}
	}
	fn refresh(&mut self) {
		// Placeholder: recompute angles (could reflect dynamic additions)
		let base_ids: Vec<String> = self.peers.iter().map(|n| n.id.clone()).collect();
		let count = base_ids.len().max(1);
		self.peers = base_ids
			.into_iter()
			.enumerate()
			.map(|(i, id)| PeerNode {
				id,
				angle: (i as f64) * (std::f64::consts::TAU / count as f64),
			})
			.collect();
		if self.selected >= self.peers.len() {
			self.selected = 0;
		}
	}
}

struct PeersView {
	peers: Vec<PeerRow>,
	selected: usize,
}

impl PeersView {
	fn new() -> Self {
		Self {
			peers: sample_peers(),
			selected: 0,
		}
	}

	fn next(&mut self) {
		if self.peers.is_empty() {
			return;
		}
		self.selected = (self.selected + 1) % self.peers.len();
	}

	fn previous(&mut self) {
		if self.peers.is_empty() {
			return;
		}
		if self.selected == 0 {
			self.selected = self.peers.len() - 1;
		} else {
			self.selected -= 1;
		}
	}

	fn refresh(&mut self) {
		// Placeholder: refresh logic would query live peer set
		self.peers = sample_peers();
		if self.selected >= self.peers.len() {
			self.selected = 0;
		}
	}
}

struct PeerRow {
	id: String,
	address: String,
	status: String,
}

fn sample_peers() -> Vec<PeerRow> {
	vec![
		PeerRow {
			id: "12D3KooWAbc...".into(),
			address: "/ip4/127.0.0.1/tcp/4001".into(),
			status: "connected".into(),
		},
		PeerRow {
			id: "12D3KooXDef...".into(),
			address: "/ip4/192.168.1.10/tcp/4001".into(),
			status: "discovered".into(),
		},
		PeerRow {
			id: "12D3KooYZ12...".into(),
			address: "/dns4/example.com/tcp/4001".into(),
			status: "dialing".into(),
		},
	]
}

struct CreateUserForm {
	username: String,
	password: String,
	field: ActiveField,
	submitted: bool,
}

impl CreateUserForm {
	fn new() -> Self {
		Self {
			username: String::new(),
			password: String::new(),
			field: ActiveField::Username,
			submitted: false,
		}
	}
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum ActiveField {
	Username,
	Password,
}

struct ShellApp {
	should_quit: bool,
	menu_items: Vec<&'static str>,
	menu_state: ListState,
	status_line: String,
	mode: Mode,
	_peer: PuppyPeer,
	last_refresh: Instant,
	refresh_interval: Duration,
	refresh_count: u64,
	latest_state: Option<State>,
}

impl ShellApp {
	fn new() -> Self {
		let mut state = ListState::default();
		state.select(Some(0));
		Self {
			should_quit: false,
			menu_items: vec![
				"peers",
				"peers graph",
				"create token",
				"create user",
				"quit",
			],
			menu_state: state,
			status_line: "Use ↑/↓ to navigate, Enter to select, q to quit".to_string(),
			mode: Mode::Menu,
			_peer: PuppyPeer::new(),
			last_refresh: Instant::now(),
			refresh_interval: Duration::from_secs(5),
			refresh_count: 0,
			latest_state: None,
		}
	}

	fn next(&mut self) {
		let i = match self.menu_state.selected() {
			Some(i) => {
				if i >= self.menu_items.len() - 1 {
					0
				} else {
					i + 1
				}
			}
			None => 0,
		};
		self.menu_state.select(Some(i));
	}

	fn previous(&mut self) {
		let i = match self.menu_state.selected() {
			Some(i) => {
				if i == 0 {
					self.menu_items.len() - 1
				} else {
					i - 1
				}
			}
			None => 0,
		};
		self.menu_state.select(Some(i));
	}

	fn activate(&mut self) {
		if let Mode::Menu = self.mode {
			if let Some(i) = self.menu_state.selected() {
				match self.menu_items[i] {
					"quit" => self.should_quit = true,
					"peers" => {
						self.mode = Mode::Peers(PeersView::new());
						self.status_line =
							"Peers loaded (placeholder). r: refresh, ↑/↓: navigate, Esc: back"
								.into();
					}
					"create token" => {
						self.status_line = "Token created (placeholder)".into();
					}
					"create user" => {
						self.mode = Mode::CreateUser(CreateUserForm::new());
						self.status_line = "Enter username/password, Tab to switch field, Enter to submit, Esc to cancel".into();
					}
					"peers graph" => {
						self.mode = Mode::PeersGraph(GraphView::new());
						self.status_line = "Graph view. r: refresh, ←/→: select, Esc: back".into();
					}
					_ => {}
				}
			}
		}
	}

	fn handle_event(&mut self, event: Event) {
		if let Event::Key(key) = event {
			match &mut self.mode {
				Mode::Menu => match key.code {
					KeyCode::Char('q') => self.should_quit = true,
					KeyCode::Down => self.next(),
					KeyCode::Up => self.previous(),
					KeyCode::Enter => self.activate(),
					_ => {}
				},
				Mode::Peers(view) => match key.code {
					KeyCode::Esc => {
						self.mode = Mode::Menu;
						self.status_line = "Back to menu".into();
					}
					KeyCode::Down => view.next(),
					KeyCode::Up => view.previous(),
					KeyCode::Char('r') => {
						view.refresh();
						self.status_line = "Refreshed peers (placeholder)".into();
					}
					KeyCode::Char('q') => {
						/* allow quit shortcut */
						self.should_quit = true;
					}
					_ => {}
				},
				Mode::PeersGraph(graph) => match key.code {
					KeyCode::Esc => {
						self.mode = Mode::Menu;
						self.status_line = "Back to menu".into();
					}
					KeyCode::Left => graph.previous(),
					KeyCode::Right => graph.next(),
					KeyCode::Char('r') => {
						graph.refresh();
						self.status_line = "Refreshed graph (placeholder)".into();
					}
					KeyCode::Char('q') => {
						self.should_quit = true;
					}
					_ => {}
				},
				Mode::CreateUser(form) => {
					match key.code {
						KeyCode::Esc => {
							self.mode = Mode::Menu;
							self.status_line = "Cancelled create user".into();
						}
						KeyCode::Tab | KeyCode::BackTab => {
							form.field = match form.field {
								ActiveField::Username => ActiveField::Password,
								ActiveField::Password => ActiveField::Username,
							};
						}
						KeyCode::Enter => {
							if !form.username.is_empty() && !form.password.is_empty() {
								form.submitted = true;
								self.status_line =
									format!("Created user '{}' (placeholder)", form.username);
								self.mode = Mode::Menu;
							} else {
								self.status_line = "Both fields required".into();
							}
						}
						KeyCode::Char(c) => match form.field {
							ActiveField::Username => form.username.push(c),
							ActiveField::Password => form.password.push(c),
						},
						KeyCode::Backspace => match form.field {
							ActiveField::Username => {
								form.username.pop();
							}
							ActiveField::Password => {
								form.password.pop();
							}
						},
						KeyCode::Left | KeyCode::Right => {} // ignore for now
						_ => {}
					}
				}
			}
		}
	}

	fn periodic_refresh(&mut self) {
		if self.last_refresh.elapsed() >= self.refresh_interval {
			// Pull latest core state
			let state = PuppyPeer::get_state();
			self.latest_state = Some(state.clone());
			// Update active views from state (if open)
			match &mut self.mode {
				Mode::Peers(view) => {
					if !state.peers.is_empty() {
						view.peers = state
							.peers
							.iter()
							.enumerate()
							.map(|(_i, p)| PeerRow {
								id: format!("{}", p.id),
								address: "".into(), // address not yet in State, placeholder
								status: "".into(),  // could derive from connections later
							})
							.collect();
						if view.selected >= view.peers.len() { view.selected = 0; }
						self.status_line = format!("Auto-refreshed peers ({} entries)", view.peers.len());
					} else {
						view.peers.clear();
						self.status_line = "Auto-refreshed peers (none)".into();
					}
				}
				Mode::PeersGraph(graph) => {
					if !state.peers.is_empty() {
						let count = state.peers.len();
						graph.peers = state
							.peers
							.iter()
							.enumerate()
							.map(|(i, p)| PeerNode { id: format!("{}", p.id), angle: (i as f64) * (std::f64::consts::TAU / count as f64) })
							.collect();
						if graph.selected >= graph.peers.len() { graph.selected = 0; }
						self.status_line = format!("Auto-refreshed graph ({} nodes)", graph.peers.len());
					} else {
						graph.peers.clear();
						self.status_line = "Auto-refreshed graph (none)".into();
					}
				}
				_ => {}
			}
			// legacy post-refresh per-mode adjustments removed (state-based updates already applied)
			self.refresh_count += 1;
			self.last_refresh = Instant::now();
		}
	}
}

pub fn run() -> io::Result<()> {
	enable_raw_mode()?;
	let mut stdout = io::stdout();
	execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
	let backend = CrosstermBackend::new(stdout);
	let mut terminal = Terminal::new(backend)?;

	let result = run_app(&mut terminal);

	restore_terminal(&mut terminal)?;

	result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
	let mut app = ShellApp::new();

	while !app.should_quit {
		// Periodic refresh hook
		app.periodic_refresh();
		terminal.draw(|f| {
			let size = f.size();

			match &app.mode {
				Mode::Menu => {
					let chunks = Layout::default()
						.direction(Direction::Vertical)
						.margin(1)
						.constraints(
							[
								Constraint::Length(3), // title / help
								Constraint::Min(5),    // menu list
								Constraint::Length(1), // status line
							]
							.as_ref(),
						)
						.split(size);

					let header = Paragraph::new("PuppyPeer")
						.style(Style::default().fg(Color::Yellow))
						.block(Block::default().borders(Borders::ALL).title("Header"));
					f.render_widget(header, chunks[0]);

					let items: Vec<ListItem> =
						app.menu_items.iter().map(|m| ListItem::new(*m)).collect();
					let list = List::new(items)
						.block(Block::default().borders(Borders::ALL).title("Menu"))
						.highlight_style(
							Style::default()
								.fg(Color::Cyan)
								.add_modifier(Modifier::BOLD | Modifier::REVERSED),
						)
						.highlight_symbol("▶ ");
					f.render_stateful_widget(list, chunks[1], &mut app.menu_state);

					let status = Paragraph::new(app.status_line.as_str())
						.block(Block::default().borders(Borders::ALL).title("Status"));
					f.render_widget(status, chunks[2]);
				}
				Mode::Peers(view) => {
					use ratatui::widgets::{Row, Table};
					let chunks = Layout::default()
						.direction(Direction::Vertical)
						.margin(1)
						.constraints(
							[
								Constraint::Length(3), // title
								Constraint::Min(5),    // table
								Constraint::Length(1), // status
							]
							.as_ref(),
						)
						.split(size);

					let header = Paragraph::new("Peers")
						.style(Style::default().fg(Color::Green))
						.block(Block::default().borders(Borders::ALL).title("Header"));
					f.render_widget(header, chunks[0]);

					let header_row = Row::new(vec!["Idx", "Peer ID", "Address", "Status"])
						.style(Style::default().add_modifier(Modifier::BOLD));
					let rows: Vec<Row> = view
						.peers
						.iter()
						.enumerate()
						.map(|(i, p)| {
							let style = if i == view.selected {
								Style::default().fg(Color::Cyan)
							} else {
								Style::default()
							};
							Row::new(vec![
								format!("{}", i),
								p.id.clone(),
								p.address.clone(),
								p.status.clone(),
							])
							.style(style)
						})
						.collect();

					let widths = [
						Constraint::Length(4),
						Constraint::Length(16),
						Constraint::Percentage(50),
						Constraint::Length(12),
					];
					let table = Table::new(rows, &widths)
						.header(header_row)
						.block(
							Block::default()
								.borders(Borders::ALL)
								.title("Peers (r=refresh, Esc=back)"),
						)
						.highlight_style(Style::default().add_modifier(Modifier::REVERSED));
					f.render_widget(table, chunks[1]);

					let status = Paragraph::new(app.status_line.as_str())
						.block(Block::default().borders(Borders::ALL).title("Status"));
					f.render_widget(status, chunks[2]);
				}
				Mode::CreateUser(form) => {
					let chunks = Layout::default()
						.direction(Direction::Vertical)
						.margin(1)
						.constraints(
							[
								Constraint::Length(3), // title
								Constraint::Min(5),    // form
								Constraint::Length(1), // status
							]
							.as_ref(),
						)
						.split(size);

					let header = Paragraph::new("Create User")
						.style(Style::default().fg(Color::Magenta))
						.block(Block::default().borders(Borders::ALL).title("Header"));
					f.render_widget(header, chunks[0]);

					let form_chunks = Layout::default()
						.direction(Direction::Vertical)
						.margin(1)
						.constraints(
							[
								Constraint::Length(3),
								Constraint::Length(3),
								Constraint::Min(1),
							]
							.as_ref(),
						)
						.split(chunks[1]);

					let username_label = format!("Username: {}", form.username);
					let password_mask: String = "*".repeat(form.password.len());
					let password_label = format!("Password: {}", password_mask);

					let username_para = Paragraph::new(username_label)
						.block(
							Block::default()
								.borders(Borders::ALL)
								.title(match form.field {
									ActiveField::Username => "[Username]*",
									ActiveField::Password => "Username",
								}),
						)
						.style(match form.field {
							ActiveField::Username => Style::default().fg(Color::Cyan),
							_ => Style::default(),
						})
						.wrap(Wrap { trim: true });

					let password_para = Paragraph::new(password_label)
						.block(
							Block::default()
								.borders(Borders::ALL)
								.title(match form.field {
									ActiveField::Password => "[Password]*",
									ActiveField::Username => "Password",
								}),
						)
						.style(match form.field {
							ActiveField::Password => Style::default().fg(Color::Cyan),
							_ => Style::default(),
						})
						.wrap(Wrap { trim: true });

					let help = Paragraph::new("Tab: switch field | Enter: submit | Esc: cancel")
						.block(Block::default().borders(Borders::ALL).title("Help"));

					f.render_widget(username_para, form_chunks[0]);
					f.render_widget(password_para, form_chunks[1]);
					f.render_widget(help, form_chunks[2]);

					let status = Paragraph::new(app.status_line.as_str())
						.block(Block::default().borders(Borders::ALL).title("Status"));
					f.render_widget(status, chunks[2]);
				}
				Mode::PeersGraph(graph) => {
					let chunks = Layout::default()
						.direction(Direction::Vertical)
						.margin(1)
						.constraints(
							[
								Constraint::Length(3), // title
								Constraint::Min(5),    // canvas
								Constraint::Length(1), // status
							]
							.as_ref(),
						)
						.split(size);

					let header = Paragraph::new("Peers Graph")
						.style(Style::default().fg(Color::Blue))
						.block(Block::default().borders(Borders::ALL).title("Header"));
					f.render_widget(header, chunks[0]);

					// Canvas coordinate system: we'll use (-1.2,-1.0) to (1.2,1.0) to leave some margin
					let peers_clone = graph
						.peers
						.iter()
						.enumerate()
						.map(|(i, n)| (i, n.id.clone(), n.angle))
						.collect::<Vec<_>>();
					let selected = graph.selected;
					let canvas = Canvas::default()
						.block(
							Block::default()
								.borders(Borders::ALL)
								.title("Graph (r=refresh, ←/→ select, Esc back)"),
						)
						.x_bounds([-1.3, 1.3])
						.y_bounds([-1.1, 1.1])
						.paint(move |ctx| {
							// Draw connecting lines (complete graph for placeholder)
							for (i1, _id1, a1) in &peers_clone {
								let x1 = a1.cos();
								let y1 = a1.sin();
								for (i2, _id2, a2) in &peers_clone {
									if i1 < i2 {
										// avoid duplicates
										let x2 = a2.cos();
										let y2 = a2.sin();
										ctx.draw(&Line {
											x1,
											y1,
											x2,
											y2,
											color: Color::DarkGray,
										});
									}
								}
							}
							// Draw nodes
							for (i, id, a) in &peers_clone {
								let x = a.cos();
								let y = a.sin();
								let color = if *i == selected {
									Color::Cyan
								} else {
									Color::White
								};
								ctx.draw(&Points {
									coords: &[(x, y)],
									color,
								});
								// Simple label: first 5 chars radial outward
								let label: String = id.chars().take(5).collect();
								let lx = x * 1.1;
								let ly = y * 1.1;
								ctx.print(lx, ly, label);
							}
						});
					f.render_widget(canvas, chunks[1]);

					let status = Paragraph::new(app.status_line.as_str())
						.block(Block::default().borders(Borders::ALL).title("Status"));
					f.render_widget(status, chunks[2]);
				}
			}
		})?;

		if event::poll(Duration::from_millis(200))? {
			let event = event::read()?;
			app.handle_event(event);
		}
	}

	Ok(())
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
	disable_raw_mode()?;
	execute!(
		terminal.backend_mut(),
		LeaveAlternateScreen,
		DisableMouseCapture
	)?;
	terminal.show_cursor()?;
	Ok(())
}
