use std::{
	collections::VecDeque,
	io::{self, Stdout},
	time::Duration,
};

use anyhow::{Result, anyhow};
use crossterm::{
	event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
	execute,
	terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use libp2p::{PeerId, identity};
use ratatui::{
	Frame, Terminal,
	backend::CrosstermBackend,
	layout::{Constraint, Direction, Layout},
	style::{Color, Modifier, Style},
	text::{Line, Span},
	widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use puppyagent_core::p2p::{
	AuthManager, AuthMethod, ControlPlaneRequest, ControlPlaneResponse, FileAccess,
	PermissionGrant, SessionInfo, TokenInfo, UserSummary,
};

const MAX_LOG_LINES: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
	Menu,
	Form,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
	AuthenticateCredentials,
	AuthenticateToken,
	CreateUser,
	CreateToken,
	GrantAccess,
	ListUsers,
	ListTokens,
	RevokeToken,
	RevokeUser,
	Logout,
	Quit,
}

struct MenuItem {
	label: &'static str,
	action: Action,
}

#[derive(Clone)]
struct FormField {
	label: &'static str,
	placeholder: Option<&'static str>,
	value: String,
}

struct FormState {
	action: Action,
	fields: Vec<FormField>,
	active: usize,
}

struct App {
	auth_manager: AuthManager,
	peer_id: PeerId,
	session: Option<SessionInfo>,
	menu: Vec<MenuItem>,
	selected: usize,
	focus: Focus,
	log: VecDeque<String>,
	current_form: Option<FormState>,
	should_exit: bool,
}

impl App {
	fn new() -> Self {
		let keypair = identity::Keypair::generate_ed25519();
		let peer_id = PeerId::from(keypair.public());
		Self {
			auth_manager: AuthManager::default(),
			peer_id,
			session: None,
			menu: vec![
				MenuItem {
					label: "Authenticate (credentials)",
					action: Action::AuthenticateCredentials,
				},
				MenuItem {
					label: "Authenticate (token)",
					action: Action::AuthenticateToken,
				},
				MenuItem {
					label: "Create user",
					action: Action::CreateUser,
				},
				MenuItem {
					label: "Create token",
					action: Action::CreateToken,
				},
				MenuItem {
					label: "Grant access",
					action: Action::GrantAccess,
				},
				MenuItem {
					label: "List users",
					action: Action::ListUsers,
				},
				MenuItem {
					label: "List tokens",
					action: Action::ListTokens,
				},
				MenuItem {
					label: "Revoke token",
					action: Action::RevokeToken,
				},
				MenuItem {
					label: "Revoke user",
					action: Action::RevokeUser,
				},
				MenuItem {
					label: "Logout",
					action: Action::Logout,
				},
				MenuItem {
					label: "Quit",
					action: Action::Quit,
				},
			],
			selected: 0,
			focus: Focus::Menu,
			log: VecDeque::new(),
			current_form: None,
			should_exit: false,
		}
	}

	fn log_line(&mut self, line: impl AsRef<str>) {
		for entry in line.as_ref().split('\n') {
			if self.log.len() == MAX_LOG_LINES {
				self.log.pop_front();
			}
			self.log.push_back(entry.to_string());
		}
	}

	fn clear_form(&mut self) {
		self.current_form = None;
		self.focus = Focus::Menu;
	}

	fn start_form(&mut self, action: Action) {
		let fields = match action {
			Action::AuthenticateCredentials => vec![
				FormField {
					label: "Username",
					placeholder: None,
					value: String::new(),
				},
				FormField {
					label: "Password",
					placeholder: None,
					value: String::new(),
				},
			],
			Action::AuthenticateToken => vec![FormField {
				label: "Token",
				placeholder: None,
				value: String::new(),
			}],
			Action::CreateUser => vec![
				FormField {
					label: "Username",
					placeholder: None,
					value: String::new(),
				},
				FormField {
					label: "Password",
					placeholder: None,
					value: String::new(),
				},
				FormField {
					label: "Roles",
					placeholder: Some("comma separated, optional"),
					value: String::new(),
				},
				FormField {
					label: "Permissions",
					placeholder: Some("comma separated, optional"),
					value: String::new(),
				},
			],
			Action::CreateToken => vec![
				FormField {
					label: "Username",
					placeholder: None,
					value: String::new(),
				},
				FormField {
					label: "Label",
					placeholder: Some("optional"),
					value: String::new(),
				},
				FormField {
					label: "Expires in seconds",
					placeholder: Some("optional"),
					value: String::new(),
				},
				FormField {
					label: "Permissions",
					placeholder: Some("comma separated, optional"),
					value: String::new(),
				},
			],
			Action::GrantAccess => vec![
				FormField {
					label: "Username",
					placeholder: None,
					value: String::new(),
				},
				FormField {
					label: "Permissions",
					placeholder: Some("comma separated, optional"),
					value: String::new(),
				},
				FormField {
					label: "Merge",
					placeholder: Some("true/false, default true"),
					value: String::new(),
				},
			],
			Action::ListTokens => vec![FormField {
				label: "Username",
				placeholder: Some("optional"),
				value: String::new(),
			}],
			Action::RevokeToken => vec![FormField {
				label: "Token id",
				placeholder: None,
				value: String::new(),
			}],
			Action::RevokeUser => vec![FormField {
				label: "Username",
				placeholder: None,
				value: String::new(),
			}],
			Action::Logout | Action::Quit | Action::ListUsers => Vec::new(),
		};
		if fields.is_empty() {
			if let Err(err) = self.execute_action(action, &[]) {
				self.log_line(err);
			}
		} else {
			self.current_form = Some(FormState {
				action,
				fields,
				active: 0,
			});
			self.focus = Focus::Form;
		}
	}

	fn execute_action(&mut self, action: Action, fields: &[FormField]) -> Result<(), String> {
		match action {
			Action::AuthenticateCredentials => {
				let username = fields
					.first()
					.map(|f| f.value.trim().to_string())
					.unwrap_or_default();
				let password = fields.get(1).map(|f| f.value.clone()).unwrap_or_default();
				if username.is_empty() {
					return Err(String::from("Username is required"));
				}
				let request = ControlPlaneRequest::Authenticate {
					method: AuthMethod::Credentials { username, password },
				};
				self.send_request(request);
			}
			Action::AuthenticateToken => {
				let token = fields
					.first()
					.map(|f| f.value.trim().to_string())
					.unwrap_or_default();
				if token.is_empty() {
					return Err(String::from("Token is required"));
				}
				let request = ControlPlaneRequest::Authenticate {
					method: AuthMethod::Token { token },
				};
				self.send_request(request);
			}
			Action::CreateUser => {
				let username = fields
					.first()
					.map(|f| f.value.trim().to_string())
					.unwrap_or_default();
				let password = fields.get(1).map(|f| f.value.clone()).unwrap_or_default();
				if username.is_empty() {
					return Err(String::from("Username is required"));
				}
				if password.is_empty() {
					return Err(String::from("Password is required"));
				}
				let roles = parse_roles(fields.get(2));
				let permissions =
					parse_permissions(fields.get(3)).map_err(|err| err.to_string())?;
				let request = ControlPlaneRequest::CreateUser {
					username,
					password,
					roles,
					permissions,
				};
				self.send_request(request);
			}
			Action::CreateToken => {
				let username = fields
					.first()
					.map(|f| f.value.trim().to_string())
					.unwrap_or_default();
				if username.is_empty() {
					return Err(String::from("Username is required"));
				}
				let label = fields.get(1).and_then(|f| {
					let value = f.value.trim();
					if value.is_empty() {
						None
					} else {
						Some(value.to_string())
					}
				});
				let expires_in = match fields.get(2) {
					Some(field) if !field.value.trim().is_empty() => field
						.value
						.trim()
						.parse::<u64>()
						.map(Some)
						.map_err(|_| String::from("Expires in must be a positive integer"))?,
					_ => None,
				};
				let permissions =
					parse_permissions(fields.get(3)).map_err(|err| err.to_string())?;
				let request = ControlPlaneRequest::CreateToken {
					username,
					label,
					expires_in,
					permissions,
				};
				self.send_request(request);
			}
			Action::GrantAccess => {
				let username = fields
					.first()
					.map(|f| f.value.trim().to_string())
					.unwrap_or_default();
				if username.is_empty() {
					return Err(String::from("Username is required"));
				}
				let permissions =
					parse_permissions(fields.get(1)).map_err(|err| err.to_string())?;
				let merge = match fields.get(2) {
					Some(field) if !field.value.trim().is_empty() => {
						parse_bool(field.value.trim())?
					}
					_ => true,
				};
				let request = ControlPlaneRequest::GrantAccess {
					username,
					permissions,
					merge,
				};
				self.send_request(request);
			}
			Action::ListUsers => {
				self.send_request(ControlPlaneRequest::ListUsers);
			}
			Action::ListTokens => {
				let username = fields.first().and_then(|f| {
					let value = f.value.trim();
					if value.is_empty() {
						None
					} else {
						Some(value.to_string())
					}
				});
				let request = ControlPlaneRequest::ListTokens { username };
				self.send_request(request);
			}
			Action::RevokeToken => {
				let token_id = fields
					.first()
					.map(|f| f.value.trim().to_string())
					.unwrap_or_default();
				if token_id.is_empty() {
					return Err(String::from("Token id is required"));
				}
				let request = ControlPlaneRequest::RevokeToken { token_id };
				self.send_request(request);
			}
			Action::RevokeUser => {
				let username = fields
					.first()
					.map(|f| f.value.trim().to_string())
					.unwrap_or_default();
				if username.is_empty() {
					return Err(String::from("Username is required"));
				}
				let request = ControlPlaneRequest::RevokeUser { username };
				self.send_request(request);
			}
			Action::Logout => {
				self.auth_manager.logout(&self.peer_id);
				self.session = None;
				self.log_line("Logged out");
			}
			Action::Quit => {
				self.should_exit = true;
			}
		}
		Ok(())
	}

	fn send_request(&mut self, request: ControlPlaneRequest) {
		let response = self
			.auth_manager
			.handle_control_request(&self.peer_id, request);
		self.handle_response(response);
	}

	fn handle_response(&mut self, response: ControlPlaneResponse) {
		match response {
			ControlPlaneResponse::AuthSuccess { session } => {
				self.session = Some(session.clone());
				self.log_line(format!(
					"Authenticated as {} (roles: {})",
					session.username,
					session.roles.join(", ")
				));
				if let Some(expires) = session.expires_at {
					self.log_line(format!("Session expires at epoch {}", expires));
				}
			}
			ControlPlaneResponse::AuthFailure { reason } => {
				self.log_line(format!("Authentication failed: {}", reason));
				if reason.contains("expired") {
					self.session = None;
				}
			}
			ControlPlaneResponse::UserCreated { username } => {
				self.log_line(format!("User created: {}", username));
			}
			ControlPlaneResponse::UserRemoved { username } => {
				self.log_line(format!("User removed: {}", username));
			}
			ControlPlaneResponse::TokenIssued {
				token,
				token_id,
				username,
				permissions,
				expires_at,
				..
			} => {
				self.log_line(format!("Token issued for {} (id: {})", username, token_id));
				self.log_line(format!("Secret: {}", token));
				self.log_line(format!("Permissions: {}", format_permissions(&permissions)));
				if let Some(exp) = expires_at {
					self.log_line(format!("Expires at epoch {}", exp));
				}
			}
			ControlPlaneResponse::TokenRevoked { token_id } => {
				self.log_line(format!("Token revoked: {}", token_id));
			}
			ControlPlaneResponse::AccessGranted {
				username,
				permissions,
			} => {
				self.log_line(format!("Updated permissions for {}", username));
				self.log_line(format!("Permissions: {}", format_permissions(&permissions)));
			}
			ControlPlaneResponse::Users(users) => {
				self.render_users(users);
			}
			ControlPlaneResponse::Tokens(tokens) => {
				self.render_tokens(tokens);
			}
			ControlPlaneResponse::Error(message) => {
				self.log_line(format!("Error: {}", message));
			}
		}
	}

	fn render_users(&mut self, users: Vec<UserSummary>) {
		self.log_line(format!("Users: {}", users.len()));
		for user in users {
			self.log_line(format!(
				"- {} | roles: {} | permissions: {}",
				user.username,
				if user.roles.is_empty() {
					String::from("(none)")
				} else {
					user.roles.join(", ")
				},
				format_permissions(&user.permissions)
			));
		}
	}

	fn render_tokens(&mut self, tokens: Vec<TokenInfo>) {
		self.log_line(format!("Tokens: {}", tokens.len()));
		for token in tokens {
			self.log_line(format!(
				"- {} | user: {} | revoked: {} | expires: {}",
				token.id,
				token.username,
				if token.revoked { "yes" } else { "no" },
				match token.expires_at {
					Some(exp) => exp.to_string(),
					None => String::from("never"),
				}
			));
			if let Some(label) = token.label.as_ref() {
				self.log_line(format!("  label: {}", label));
			}
			self.log_line(format!(
				"  permissions: {}",
				format_permissions(&token.permissions)
			));
		}
	}
}

pub fn run() -> Result<()> {
	enable_raw_mode()?;
	let mut stdout = io::stdout();
	execute!(stdout, EnterAlternateScreen)?;
	let backend = CrosstermBackend::new(stdout);
	let mut terminal = Terminal::new(backend)?;
	terminal.hide_cursor()?;
	terminal.clear()?;
	let mut app = App::new();
	let result = run_loop(&mut terminal, &mut app);
	disable_raw_mode()?;
	execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
	terminal.show_cursor()?;
	result
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
	while !app.should_exit {
		terminal.draw(|frame| draw_ui(frame, app))?;
		if event::poll(Duration::from_millis(250))? {
			match event::read()? {
				Event::Key(key) if key.kind == KeyEventKind::Press => handle_key(app, key),
				Event::Resize(_, _) => {}
				_ => {}
			}
		}
	}
	Ok(())
}

fn handle_key(app: &mut App, key: KeyEvent) {
	match app.focus {
		Focus::Menu => match key.code {
			KeyCode::Char('q') | KeyCode::Esc => app.should_exit = true,
			KeyCode::Up => {
				if app.selected == 0 {
					app.selected = app.menu.len().saturating_sub(1);
				} else {
					app.selected -= 1;
				}
			}
			KeyCode::Down => {
				app.selected = (app.selected + 1) % app.menu.len();
			}
			KeyCode::Enter => {
				let action = app.menu[app.selected].action;
				app.start_form(action);
			}
			_ => {}
		},
		Focus::Form => {
			if let Some(form) = app.current_form.as_mut() {
				match key.code {
					KeyCode::Esc => app.clear_form(),
					KeyCode::Tab => move_field(form, 1),
					KeyCode::BackTab => move_field(form, -1),
					KeyCode::Up => move_field(form, -1),
					KeyCode::Down => move_field(form, 1),
					KeyCode::Enter => {
						let action = form.action;
						let fields = form.fields.clone();
						match app.execute_action(action, &fields) {
							Ok(_) => app.clear_form(),
							Err(err) => app.log_line(err),
						}
					}
					KeyCode::Backspace => {
						if let Some(field) = form.fields.get_mut(form.active) {
							field.value.pop();
						}
					}
					KeyCode::Char(c) => {
						if let Some(field) = form.fields.get_mut(form.active) {
							field.value.push(c);
						}
					}
					_ => {}
				}
			}
		}
	}
}

fn move_field(form: &mut FormState, delta: isize) {
	if form.fields.is_empty() {
		return;
	}
	let len = form.fields.len() as isize;
	let current = form.active as isize;
	let mut next = (current + delta) % len;
	if next < 0 {
		next += len;
	}
	form.active = next as usize;
}

fn draw_ui(frame: &mut Frame<'_>, app: &App) {
	let size = frame.size();
	let layout = Layout::default()
		.direction(Direction::Vertical)
		.constraints([
			Constraint::Length(3),
			Constraint::Min(10),
			Constraint::Length(8),
		])
		.split(size);

	draw_header(frame, layout[0], app);
	draw_main(frame, layout[1], app);
	draw_log(frame, layout[2], app);
}

fn draw_header(frame: &mut Frame<'_>, area: ratatui::layout::Rect, app: &App) {
	let mut lines = Vec::new();
	match app.session.as_ref() {
		Some(session) => {
			lines.push(Line::from(vec![Span::styled(
				format!("Session: {}", session.username),
				Style::default()
					.fg(Color::Green)
					.add_modifier(Modifier::BOLD),
			)]));
			lines.push(Line::from(format!("Roles: {}", session.roles.join(", "))));
			let expires = session
				.expires_at
				.map(|exp| format!("expires at epoch {exp}"))
				.unwrap_or_else(|| String::from("no expiry"));
			lines.push(Line::from(expires));
		}
		None => {
			lines.push(Line::from(Span::styled(
				"Not authenticated",
				Style::default().fg(Color::Yellow),
			)));
			lines.push(Line::from("Use Authenticate to start a session"));
		}
	}
	let paragraph =
		Paragraph::new(lines).block(Block::default().title("Session").borders(Borders::ALL));
	frame.render_widget(paragraph, area);
}

fn draw_main(frame: &mut Frame<'_>, area: ratatui::layout::Rect, app: &App) {
	let chunks = Layout::default()
		.direction(Direction::Horizontal)
		.constraints([Constraint::Length(30), Constraint::Min(20)])
		.split(area);
	draw_menu(frame, chunks[0], app);
	draw_details(frame, chunks[1], app);
}

fn draw_menu(frame: &mut Frame<'_>, area: ratatui::layout::Rect, app: &App) {
	let items: Vec<ListItem> = app
		.menu
		.iter()
		.enumerate()
		.map(|(idx, item)| {
			let style = if idx == app.selected {
				Style::default()
					.fg(Color::Cyan)
					.add_modifier(Modifier::BOLD)
			} else {
				Style::default()
			};
			ListItem::new(Line::from(Span::styled(item.label, style)))
		})
		.collect();
	let block = Block::default()
		.title("Actions")
		.borders(Borders::ALL)
		.border_style(if app.focus == Focus::Menu {
			Style::default().fg(Color::Cyan)
		} else {
			Style::default()
		});
	let list = List::new(items).block(block);
	frame.render_widget(list, area);
}

fn draw_details(frame: &mut Frame<'_>, area: ratatui::layout::Rect, app: &App) {
	let mut lines = Vec::new();
	if let Some(form) = app.current_form.as_ref() {
		lines.push(Line::from(Span::styled(
			"Enter values, Enter to submit, Esc to cancel",
			Style::default().fg(Color::Yellow),
		)));
		for (idx, field) in form.fields.iter().enumerate() {
			let mut text = String::new();
			text.push_str(field.label);
			text.push_str(": ");
			text.push_str(&field.value);
			if field.value.is_empty()
				&& let Some(placeholder) = field.placeholder
			{
				text.push(' ');
				text.push_str(placeholder);
			}
			let style = if idx == form.active {
				Style::default()
					.fg(Color::Cyan)
					.add_modifier(Modifier::BOLD)
			} else {
				Style::default()
			};
			lines.push(Line::from(Span::styled(text, style)));
		}
	} else {
		lines.push(Line::from("Select an action and press Enter"));
		lines.push(Line::from("Use arrow keys to navigate"));
		lines.push(Line::from("q or Esc to quit"));
	}
	let block = Block::default()
		.title("Details")
		.borders(Borders::ALL)
		.border_style(if app.focus == Focus::Form {
			Style::default().fg(Color::Cyan)
		} else {
			Style::default()
		});
	let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
	frame.render_widget(paragraph, area);
}

fn draw_log(frame: &mut Frame<'_>, area: ratatui::layout::Rect, app: &App) {
	let lines: Vec<Line> = app
		.log
		.iter()
		.map(|entry| Line::from(entry.clone()))
		.collect();
	let paragraph = Paragraph::new(lines)
		.block(Block::default().title("Log").borders(Borders::ALL))
		.wrap(Wrap { trim: true });
	frame.render_widget(paragraph, area);
}

fn parse_roles(field: Option<&FormField>) -> Vec<String> {
	field
		.map(|f| {
			f.value
				.split(',')
				.filter_map(|item| {
					let trimmed = item.trim();
					if trimmed.is_empty() {
						None
					} else {
						Some(trimmed.to_string())
					}
				})
				.collect()
		})
		.unwrap_or_default()
}

fn parse_permissions(
	field: Option<&FormField>,
) -> std::result::Result<Vec<PermissionGrant>, anyhow::Error> {
	let Some(field) = field else {
		return Ok(Vec::new());
	};
	let mut result = Vec::new();
	for token in field.value.split(',') {
		let token = token.trim();
		if token.is_empty() {
			continue;
		}
		let perm = parse_permission_token(token)?;
		result.push(perm);
	}
	Ok(result)
}

fn parse_permission_token(token: &str) -> std::result::Result<PermissionGrant, anyhow::Error> {
	let lower = token.to_lowercase();
	match lower.as_str() {
		"owner" => Ok(PermissionGrant::Owner),
		"viewer" => Ok(PermissionGrant::Viewer),
		"systeminfo" | "system" => Ok(PermissionGrant::SystemInfo),
		"diskinfo" | "disk" => Ok(PermissionGrant::DiskInfo),
		"networkinfo" | "network" => Ok(PermissionGrant::NetworkInfo),
		value if value.starts_with("files:") => {
			let parts: Vec<&str> = token.splitn(3, ':').collect();
			if parts.len() < 3 {
				return Err(anyhow!("Files permission must be files:/path:access"));
			}
			let path = parts[1].trim();
			if path.is_empty() {
				return Err(anyhow!("Files permission requires a path"));
			}
			let access = match parts[2].trim().to_lowercase().as_str() {
				"read" | "r" => FileAccess::Read,
				"readwrite" | "rw" | "write" | "w" => FileAccess::ReadWrite,
				other => return Err(anyhow!("Unknown file access mode {other}")),
			};
			Ok(PermissionGrant::Files {
				path: path.to_string(),
				access,
			})
		}
		_ => Err(anyhow!("Unknown permission {token}")),
	}
}

fn parse_bool(input: &str) -> Result<bool, String> {
	match input.trim().to_lowercase().as_str() {
		"true" | "t" | "yes" | "y" | "1" => Ok(true),
		"false" | "f" | "no" | "n" | "0" => Ok(false),
		other => Err(format!("Invalid boolean value: {}", other)),
	}
}

fn format_permissions(perms: &[PermissionGrant]) -> String {
	if perms.is_empty() {
		return String::from("(none)");
	}
	let mut parts = Vec::with_capacity(perms.len());
	for perm in perms {
		let text = match perm {
			PermissionGrant::Owner => String::from("Owner"),
			PermissionGrant::Viewer => String::from("Viewer"),
			PermissionGrant::SystemInfo => String::from("SystemInfo"),
			PermissionGrant::DiskInfo => String::from("DiskInfo"),
			PermissionGrant::NetworkInfo => String::from("NetworkInfo"),
			PermissionGrant::Files { path, access } => format!(
				"Files:{}:{}",
				path,
				match access {
					FileAccess::Read => "read",
					FileAccess::ReadWrite => "readwrite",
				}
			),
		};
		parts.push(text);
	}
	parts.join(", ")
}
