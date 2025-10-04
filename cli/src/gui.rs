use std::collections::HashMap;
use std::mem;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use iced::alignment::{Horizontal, Vertical};
use iced::executor;
use iced::theme;
use iced::time;
use iced::widget::{button, container, scrollable, text, text_input, tooltip, pick_list};
use iced::{Application, Command, Element, Length, Settings, Subscription, Theme};
use libp2p::PeerId;
use puppyagent_core::p2p::{CpuInfo, DirEntry};
use puppyagent_core::{FileChunk, PuppyPeer, State};

const LOCAL_LISTEN_MULTIADDR: &str = "/ip4/0.0.0.0:8336";
const REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const FILE_VIEW_CHUNK_SIZE: u64 = 64 * 1024;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MenuItem {
	Peers,
	PeersGraph,
	CreateUser,
	FileSearch,
	Quit,
}

const MENU_ITEMS: [MenuItem; 5] = [
	MenuItem::Peers,
	MenuItem::PeersGraph,
	MenuItem::CreateUser,
	MenuItem::FileSearch,
	MenuItem::Quit,
];

impl MenuItem {
	fn label(self) -> &'static str {
		match self {
			MenuItem::Peers => "Peers",
			MenuItem::PeersGraph => "Peers Graph",
			MenuItem::CreateUser => "Create User",
			MenuItem::FileSearch => "File Search",
			MenuItem::Quit => "Quit",
		}
	}
}

#[derive(Debug, Clone)]
struct PeerRow {
	id: String,
	address: String,
	status: String,
}

#[derive(Debug, Clone)]
struct PeerCpuState {
	peer_id: String,
	cpus: Vec<CpuInfo>,
}

#[derive(Debug, Clone)]
struct FileBrowserState {
	peer_id: String,
	path: String,
	entries: Vec<DirEntry>,
	loading: bool,
	error: Option<String>,
}

impl FileBrowserState {
	fn new(peer_id: String, path: String) -> Self {
		Self {
			peer_id,
			path,
			entries: Vec::new(),
			loading: true,
			error: None,
		}
	}
}

#[derive(Debug, Clone)]
struct FileViewerState {
	browser: FileBrowserState,
	peer_id: String,
	path: String,
	data: Vec<u8>,
	offset: u64,
	eof: bool,
	loading: bool,
	error: Option<String>,
}

impl FileViewerState {
	fn new(browser: FileBrowserState, peer_id: String, path: String) -> Self {
		Self {
			peer_id,
			path,
			browser,
			data: Vec::new(),
			offset: 0,
			eof: false,
			loading: true,
			error: None,
		}
	}

	fn apply_chunk(&mut self, chunk: FileChunk) {
		let offset = chunk.offset;
		let eof = chunk.eof;
		let data = chunk.data;
		if offset != self.offset {
			self.offset = offset;
		}
		if !data.is_empty() {
			self.offset = offset.saturating_add(data.len() as u64);
			self.data.extend_from_slice(&data);
		} else {
			self.offset = offset;
		}
		self.eof = eof;
	}
}

#[derive(Debug, Clone)]
struct GraphView {
	nodes: Vec<PeerNode>,
	selected: usize,
}

impl GraphView {
	fn new() -> Self {
		Self {
			nodes: Vec::new(),
			selected: 0,
		}
	}

	fn set_peers(&mut self, peers: &[PeerRow]) {
		let count = peers.len().max(1);
		self.nodes = peers
			.iter()
			.enumerate()
			.map(|(idx, peer)| PeerNode {
				id: peer.id.clone(),
				angle: (idx as f32) * (std::f32::consts::TAU / count as f32),
			})
			.collect();
		if self.selected >= self.nodes.len() {
			self.selected = 0;
		}
	}

	fn next(&mut self) {
		if !self.nodes.is_empty() {
			self.selected = (self.selected + 1) % self.nodes.len();
		}
	}

	fn previous(&mut self) {
		if !self.nodes.is_empty() {
			if self.selected == 0 {
				self.selected = self.nodes.len() - 1;
			} else {
				self.selected -= 1;
			}
		}
	}

	fn selected_id(&self) -> Option<&str> {
		self.nodes.get(self.selected).map(|node| node.id.as_str())
	}
}

#[derive(Debug, Clone)]
struct PeerNode {
	id: String,
	angle: f32,
}

#[derive(Debug, Clone)]
struct CreateUserForm {
	username: String,
	password: String,
	status: Option<String>,
}

impl CreateUserForm {
	fn new() -> Self {
		Self {
			username: String::new(),
			password: String::new(),
			status: None,
		}
	}
}

#[derive(Debug, Clone)]
struct FileSearchState {
	query: String,
	selected_mime: String,
	mime_filter_input: String,
	available_mime_types: Vec<String>,
	sort_desc: bool,
	results: Vec<FileSearchEntry>,
	loading: bool,
	error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FileSearchEntry {
	hash: String,
	size: u64,
	mime_type: Option<String>,
	first: String,
	latest: String,
}

impl FileSearchState {
	fn new() -> Self {
		Self {
			query: String::new(),
			selected_mime: String::new(),
			mime_filter_input: String::new(),
			available_mime_types: Vec::new(),
			sort_desc: true,
			results: Vec::new(),
			loading: false,
			error: None,
		}
	}
}

pub struct GuiApp {
	peer: Arc<PuppyPeer>,
	latest_state: Option<State>,
	local_peer_id: Option<String>,
	menu: MenuItem,
	mode: Mode,
	peers: Vec<PeerRow>,
	selected_peer_id: Option<String>,
	graph: GraphView,
	status: String,
}

#[derive(Debug, Clone)]
enum Mode {
	Peers,
	PeerActions { peer_id: String },
	PeerCpus(PeerCpuState),
	FileBrowser(FileBrowserState),
	FileViewer(FileViewerState),
	PeersGraph,
	CreateUser(CreateUserForm),
	FileSearch(FileSearchState),
}

#[derive(Debug, Clone)]
pub enum GuiMessage {
	Tick,
	MenuSelected(MenuItem),
	BackToPeers,
	PeerActionsRequested(String),
	CpuRequested(String),
	CpuLoaded(String, Result<Vec<CpuInfo>, String>),
	FileBrowserRequested {
		peer_id: String,
		path: String,
	},
	FileBrowserLoaded {
		peer_id: String,
		path: String,
		entries: Result<Vec<DirEntry>, String>,
	},
	FileEntryActivated(DirEntry),
	FileNavigateUp,
	FileReadLoaded {
		peer_id: String,
		path: String,
		offset: u64,
		result: Result<FileChunk, String>,
	},
	FileReadMore,
	FileViewerBack,
	GraphNext,
	GraphPrev,
	UsernameChanged(String),
	PasswordChanged(String),
	CreateUserSubmit,
	FileSearchQueryChanged(String),
	FileSearchMimeChanged(String),
	FileSearchToggleSort,
	FileSearchExecute,
	FileSearchLoaded(Result<(Vec<FileSearchEntry>, Vec<String>), String>),
}

impl Application for GuiApp {
	type Executor = executor::Default;
	type Message = GuiMessage;
	type Theme = Theme;
	type Flags = ();

	fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
		let peer = Arc::new(PuppyPeer::new());
		let latest_state = peer.state().lock().ok().map(|state| state.clone());
		let peers = latest_state
			.as_ref()
			.map(aggregate_peers)
			.unwrap_or_default();
		let mut graph = GraphView::new();
		graph.set_peers(&peers);
		let app = GuiApp {
			peer,
			latest_state: latest_state.clone(),
			local_peer_id: latest_state.as_ref().map(|state| state.me.to_string()),
			menu: MenuItem::Peers,
			mode: Mode::Peers,
			peers,
			selected_peer_id: None,
			graph,
			status: String::from("Ready"),
		};
		(app, Command::none())
	}

	fn title(&self) -> String {
		String::from("PuppyAgent GUI")
	}

	fn theme(&self) -> Theme {
		Theme::Dark
	}

	fn subscription(&self) -> Subscription<Self::Message> {
		time::every(REFRESH_INTERVAL).map(|_| GuiMessage::Tick)
	}

	fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
		match message {
			GuiMessage::Tick => {
				self.refresh_from_state();
				Command::none()
			}
			GuiMessage::MenuSelected(item) => {
				match item {
					MenuItem::Quit => {
						std::process::exit(0);
					}
					MenuItem::Peers => {
						self.menu = item;
						self.refresh_from_state();
						self.mode = Mode::Peers;
						self.status = if self.peers.is_empty() {
							String::from("Showing peers — none discovered")
						} else {
							format!("Showing peers — {} total", self.peers.len())
						};
					}
					MenuItem::PeersGraph => {
						self.menu = item;
						self.mode = Mode::PeersGraph;
						self.refresh_from_state();
						self.selected_peer_id = self.graph.selected_id().map(|id| id.to_string());
						self.status = match self.selected_peer_id.as_deref() {
							Some(id) => format!("Graph overview — focused on {}", id),
							None => String::from("Graph overview — no peers"),
						};
					}
					MenuItem::CreateUser => {
						self.menu = item;
						self.mode = Mode::CreateUser(CreateUserForm::new());
						self.status = String::from("Create user form");
					}
					MenuItem::FileSearch => {
						self.menu = item;
						self.mode = Mode::FileSearch(FileSearchState::new());
						self.status = String::from("File search");
					}
				}
				Command::none()
			}
			GuiMessage::BackToPeers => {
				self.menu = MenuItem::Peers;
				self.mode = Mode::Peers;
				Command::none()
			}
			GuiMessage::PeerActionsRequested(peer_id) => {
				self.mode = Mode::PeerActions {
					peer_id: peer_id.clone(),
				};
				self.selected_peer_id = Some(peer_id.clone());
				self.status = format!("Peer actions for {}", peer_id);
				Command::none()
			}
			GuiMessage::CpuRequested(peer_id) => {
				self.status = format!("Loading CPU info for {}...", peer_id);
				let peer = self.peer.clone();
				Command::perform(fetch_cpus(peer, peer_id.clone()), move |(id, result)| {
					GuiMessage::CpuLoaded(id, result)
				})
			}
			GuiMessage::CpuLoaded(peer_id, result) => {
				match result {
					Ok(cpus) => {
						self.status = cpu_summary(&cpus);
						self.mode = Mode::PeerCpus(PeerCpuState { peer_id, cpus });
					}
					Err(err) => {
						self.status = format!("Failed to load CPU info: {}", err);
						self.mode = Mode::Peers;
					}
				}
				Command::none()
			}
			GuiMessage::FileBrowserRequested { peer_id, path } => {
				self.status = format!("Listing {} on {}...", path, peer_id);
				self.mode = Mode::FileBrowser(FileBrowserState::new(peer_id.clone(), path.clone()));
				let peer = self.peer.clone();
				let local = self.local_peer_id.clone();
				Command::perform(
					fetch_dir_entries(peer, local, peer_id, path),
					|(peer_id, path, entries)| GuiMessage::FileBrowserLoaded {
						peer_id,
						path,
						entries,
					},
				)
			}
			GuiMessage::FileBrowserLoaded {
				peer_id,
				path,
				entries,
			} => {
				match &mut self.mode {
					Mode::FileBrowser(state) if state.peer_id == peer_id => {
						state.path = path.clone();
						state.loading = false;
						match entries {
							Ok(entries) => {
								state.entries = entries;
								state.error = None;
								self.status = format!("Loaded {} entries", state.entries.len());
							}
							Err(err) => {
								state.entries.clear();
								state.error = Some(err.clone());
								self.status = format!("Failed to load directory: {}", err);
							}
						}
					}
					_ => {}
				}
				Command::none()
			}
			GuiMessage::FileEntryActivated(entry) => {
				if let Mode::FileBrowser(state) = &mut self.mode {
					if entry.is_dir {
						let target = join_child_path(&state.path, &entry.name);
						let peer_id = state.peer_id.clone();
						state.path = target.clone();
						state.entries.clear();
						state.loading = true;
						state.error = None;
						self.status = format!("Opening {}...", target);
						let peer = self.peer.clone();
						let local = self.local_peer_id.clone();
						return Command::perform(
							fetch_dir_entries(peer, local, peer_id, target),
							|(peer_id, path, entries)| GuiMessage::FileBrowserLoaded {
								peer_id,
								path,
								entries,
							},
						);
					}
					let target = join_child_path(&state.path, &entry.name);
					let peer_id = state.peer_id.clone();
					let browser_snapshot = state.clone();
					self.status = format!(
						"Reading {} ({})",
						target,
						format_size(entry.size)
					);
					let peer = self.peer.clone();
					let local = self.local_peer_id.clone();
					let command = Command::perform(
						fetch_file_chunk(
							peer,
							local,
							peer_id.clone(),
							target.clone(),
							0,
							FILE_VIEW_CHUNK_SIZE,
						),
						|(peer_id, path, offset, result)| GuiMessage::FileReadLoaded {
							peer_id,
							path,
							offset,
							result,
						},
					);
					self.mode = Mode::FileViewer(FileViewerState::new(
						browser_snapshot,
						peer_id,
						target,
					));
					return command;
				}
				Command::none()
			}
			GuiMessage::FileNavigateUp => {
				if let Mode::FileBrowser(state) = &mut self.mode {
					let target = parent_path(&state.path);
					if target == state.path {
						self.status = String::from("Already at root");
						return Command::none();
					}
					let peer_id = state.peer_id.clone();
					state.path = target.clone();
					state.entries.clear();
					state.loading = true;
					state.error = None;
					self.status = format!("Opening {}...", target);
					let peer = self.peer.clone();
					let local = self.local_peer_id.clone();
					return Command::perform(
						fetch_dir_entries(peer, local, peer_id, target),
						|(peer_id, path, entries)| GuiMessage::FileBrowserLoaded {
							peer_id,
							path,
							entries,
						},
					);
				}
				Command::none()
			}
			GuiMessage::FileReadLoaded {
				peer_id,
				path,
				offset: _,
				result,
			} => {
				match &mut self.mode {
					Mode::FileViewer(state) if state.peer_id == peer_id && state.path == path => {
						state.loading = false;
						match result {
							Ok(chunk) => {
								state.error = None;
								state.apply_chunk(chunk);
								self.status = format!(
									"Loaded {} bytes{}",
									state.data.len(),
									if state.eof { " (end of file)" } else { "" }
								);
							}
							Err(err) => {
								state.error = Some(err.clone());
								self.status = format!("Failed to load file chunk: {}", err);
							}
						}
					}
					_ => {}
				}
				Command::none()
			}
			GuiMessage::FileReadMore => {
				if let Mode::FileViewer(state) = &mut self.mode {
					if state.loading {
						return Command::none();
					}
					if state.eof {
						self.status = String::from("Already at end of file");
						return Command::none();
					}
					state.loading = true;
					let peer_id = state.peer_id.clone();
					let path = state.path.clone();
					let offset = state.offset;
					self.status = format!("Loading bytes starting at {}...", offset);
					let peer = self.peer.clone();
					let local = self.local_peer_id.clone();
					return Command::perform(
						fetch_file_chunk(
							peer,
							local,
							peer_id,
							path,
							offset,
							FILE_VIEW_CHUNK_SIZE,
						),
						|(peer_id, path, offset, result)| GuiMessage::FileReadLoaded {
							peer_id,
							path,
							offset,
							result,
						},
					);
				}
				Command::none()
			}
			GuiMessage::FileViewerBack => {
				if let Mode::FileViewer(state) = mem::replace(&mut self.mode, Mode::Peers) {
					let browser = state.browser;
					self.status = format!("Browsing {} on {}", browser.path, browser.peer_id);
					self.mode = Mode::FileBrowser(browser);
				}
				Command::none()
			}
			GuiMessage::GraphNext => {
				self.graph.next();
				if let Some(id) = self.graph.selected_id() {
					self.selected_peer_id = Some(id.to_string());
					self.status = format!("Graph focus: {}", id);
				}
				Command::none()
			}
			GuiMessage::GraphPrev => {
				self.graph.previous();
				if let Some(id) = self.graph.selected_id() {
					self.selected_peer_id = Some(id.to_string());
					self.status = format!("Graph focus: {}", id);
				}
				Command::none()
			}
			GuiMessage::UsernameChanged(value) => {
				if let Mode::CreateUser(form) = &mut self.mode {
					form.username = value;
				}
				Command::none()
			}
			GuiMessage::PasswordChanged(value) => {
				if let Mode::CreateUser(form) = &mut self.mode {
					form.password = value;
				}
				Command::none()
			}
			GuiMessage::CreateUserSubmit => {
				if let Mode::CreateUser(form) = &mut self.mode {
					if form.username.trim().is_empty() || form.password.trim().is_empty() {
						form.status = Some(String::from("Both fields are required"));
					} else {
						self.status = format!("Created user '{}' (placeholder)", form.username);
						form.status = Some(self.status.clone());
						form.password.clear();
					}
				}
				Command::none()
			}
			GuiMessage::FileSearchQueryChanged(q) => {
				if let Mode::FileSearch(state) = &mut self.mode { state.query = q; }
				Command::none()
			}
			GuiMessage::FileSearchMimeChanged(m) => {
				if let Mode::FileSearch(state) = &mut self.mode { state.selected_mime = m; }
				Command::none()
			}
			GuiMessage::FileSearchToggleSort => {
				if let Mode::FileSearch(state) = &mut self.mode { state.sort_desc = !state.sort_desc; }
				Command::none()
			}
			GuiMessage::FileSearchExecute => {
				if let Mode::FileSearch(state) = &mut self.mode {
					state.loading = true; state.error = None; state.results.clear();
					let query = state.query.clone();
					let mime = if state.selected_mime.trim().is_empty() { None } else { Some(state.selected_mime.clone()) };
					let sort_desc = state.sort_desc;
					let peer = self.peer.clone();
					return Command::perform(search_files(peer, query, mime, sort_desc), GuiMessage::FileSearchLoaded);
				}
				Command::none()
			}
			GuiMessage::FileSearchLoaded(result) => {
				if let Mode::FileSearch(state) = &mut self.mode {
					state.loading = false;
					match result {
						Ok((entries, mimes)) => { state.results = entries; state.available_mime_types = mimes; self.status = format!("Search loaded: {} files", state.results.len()); }
						Err(err) => { state.error = Some(err.clone()); self.status = format!("Search failed: {}", err); }
					}
				}
				Command::none()
			}
		}
	}

	fn view(&self) -> Element<'_, Self::Message> {
		println!("mode: {:?}", self.mode);
		let mut menu_column = iced::widget::Column::new().spacing(8);
		for item in MENU_ITEMS.iter() {
			let mut label = item.label().to_string();
			if self.menu == *item {
				label = format!("▶ {}", label);
			}
			let button = button(text(label).size(16))
				// .width(Length::Fill)
				.on_press(GuiMessage::MenuSelected(*item));
			menu_column = menu_column.push(button);
		}
		let sidebar = container(menu_column)
			.width(Length::Shrink)
			.padding(16)
			.style(theme::Container::Box);
		let content: Element<_> = match &self.mode {
			Mode::Peers => self.view_peers(),
			Mode::PeerActions { peer_id } => self.view_peer_actions(peer_id),
			Mode::PeerCpus(state) => self.view_peer_cpus(state),
			Mode::FileBrowser(state) => self.view_file_browser(state),
			Mode::FileViewer(state) => self.view_file_viewer(state),
			Mode::PeersGraph => self.view_graph(),
			Mode::CreateUser(form) => self.view_create_user(form),
			Mode::FileSearch(state) => self.view_file_search(state),
		};
		let content_container = container(content)
			.width(Length::Fill)
			.height(Length::Fill)
			.padding(16)
			.style(theme::Container::Box);
		let main = iced::widget::Row::new()
			.spacing(16)
			.push(sidebar)
			.push(content_container)
			.height(Length::Fill);
		let status = container(text(&self.status).size(16))
			.width(Length::Fill)
			.padding(12)
			.style(theme::Container::Box);
		iced::widget::Column::new()
			.spacing(12)
			.padding(12)
			.push(main)
			.push(status)
			.into()
	}
}

impl GuiApp {
	fn refresh_from_state(&mut self) {
		if let Ok(state_guard) = self.peer.state().lock() {
			let snapshot = state_guard.clone();
			self.local_peer_id = Some(snapshot.me.to_string());
			self.peers = aggregate_peers(&snapshot);
			if self
				.selected_peer_id
				.clone()
				.filter(|id| !self.peers.iter().any(|p| p.id == *id))
				.is_some()
			{
				self.selected_peer_id = None;
			}
			let missing_peer = if let Mode::PeerActions { peer_id } = &self.mode {
				if !self.peers.iter().any(|p| p.id == *peer_id) {
					Some(peer_id.clone())
				} else {
					None
				}
			} else {
				None
			};
			if let Some(peer_id) = missing_peer {
				self.mode = Mode::Peers;
				self.status = format!("Peer {} not available", peer_id);
			}
			self.graph.set_peers(&self.peers);
			if let Some(idx) = self.selected_peer_id.as_ref().and_then(|selected| {
				self.graph
					.nodes
					.iter()
					.position(|node| &node.id == selected)
			}) {
				self.graph.selected = idx;
			}
			self.latest_state = Some(snapshot);
		} else {
			self.status = String::from("Waiting for peer state");
		}
	}

	fn view_peers(&self) -> Element<'_, GuiMessage> {
		println!("view_peers");
		let mut layout = iced::widget::Column::new().spacing(12);
		layout = layout.push(text("Discovered Peers").size(24));
		if self.peers.is_empty() {
			layout = layout.push(text("No peers discovered yet.").size(16));
		} else {
			let mut list = iced::widget::Column::new().spacing(4);
			for peer in &self.peers {
				let indicator = if self.selected_peer_id.as_deref() == Some(peer.id.as_str()) {
					"▶"
				} else {
					""
				};
				let id_label = format!("{} {}", indicator, abbreviate_peer_id(&peer.id));
				let id_cell = container(
					tooltip(
						text(id_label).size(16),
						text(peer.id.clone()),
						tooltip::Position::FollowCursor,
					)
					.style(theme::Container::Box),
				)
				.width(Length::FillPortion(2));
				let info = iced::widget::Row::new()
					.spacing(12)
					.push(id_cell)
					.push(
						text(peer.address.clone())
							.size(14)
							.width(Length::FillPortion(3)),
					)
					.push(
						text(peer.status.clone())
							.size(14)
							.width(Length::FillPortion(1)),
					)
					.push(
						button(text("Actions"))
							.on_press(GuiMessage::PeerActionsRequested(peer.id.clone())),
					);
				let card = container(info).padding(8).style(theme::Container::Box);
				list = list.push(card);
			}
			layout = layout.push(scrollable(list).height(Length::Fill));
		}
		layout.into()
	}

	fn view_peer_actions(&self, peer_id: &str) -> Element<'_, GuiMessage> {
		if let Some(peer) = self.peers.iter().find(|row| row.id == peer_id) {
			let mut layout = iced::widget::Column::new().spacing(12);
			layout = layout.push(text(format!("Peer {}", peer.id)).size(24));
			layout = layout.push(text(format!("Status: {}", peer.status)).size(16));
			if !peer.address.is_empty() {
				layout = layout.push(text(format!("Dial address: {}", peer.address)).size(16));
			}
			let addresses = self.gather_known_addresses(peer_id);
			if !addresses.is_empty() {
				let mut addr_box = iced::widget::Column::new().spacing(4);
				for addr in addresses {
					addr_box = addr_box.push(text(addr).size(14));
				}
				layout = layout.push(container(addr_box).padding(8).style(theme::Container::Box));
			}
			let controls = iced::widget::Row::new()
				.spacing(12)
				.push(button(text("CPU info")).on_press(GuiMessage::CpuRequested(peer.id.clone())))
				.push(
					button(text("File browser")).on_press(GuiMessage::FileBrowserRequested {
						peer_id: peer.id.clone(),
						path: String::from("/"),
					}),
				)
				.push(button(text("Back")).on_press(GuiMessage::BackToPeers));
			layout = layout.push(controls);
			layout.into()
		} else {
			container(text("Selected peer not available").size(16))
				.align_x(Horizontal::Center)
				.align_y(Vertical::Center)
				.width(Length::Fill)
				.height(Length::Fill)
				.into()
		}
	}

	fn view_peer_cpus(&self, state: &PeerCpuState) -> Element<'_, GuiMessage> {
		let mut layout = iced::widget::Column::new().spacing(12);
		layout = layout.push(text(format!("CPU inventory for {}", state.peer_id)).size(24));
		if state.cpus.is_empty() {
			layout = layout.push(text("No CPU information available.").size(16));
		} else {
			let mut list = iced::widget::Column::new().spacing(4);
			for (idx, cpu) in state.cpus.iter().enumerate() {
				let row = iced::widget::Row::new()
					.spacing(12)
					.push(text(format!("{}", idx)).size(14).width(Length::Shrink))
					.push(
						text(cpu.name.clone())
							.size(14)
							.width(Length::FillPortion(2)),
					)
					.push(
						text(format!("{:.1}%", cpu.usage))
							.size(14)
							.width(Length::FillPortion(1)),
					)
					.push(
						text(format_frequency(cpu.frequency_hz))
							.size(14)
							.width(Length::FillPortion(1)),
					);
				let card = container(row).padding(8).style(theme::Container::Box);
				list = list.push(card);
			}
			layout = layout.push(scrollable(list).height(Length::Fill));
		}
		let controls = iced::widget::Row::new()
			.spacing(12)
			.push(button(text("Refresh")).on_press(GuiMessage::CpuRequested(state.peer_id.clone())))
			.push(
				button(text("Back to actions"))
					.on_press(GuiMessage::PeerActionsRequested(state.peer_id.clone())),
			);
		layout = layout.push(controls);
		layout.into()
	}

	fn view_file_browser(&self, state: &FileBrowserState) -> Element<'_, GuiMessage> {
		let mut layout = iced::widget::Column::new().spacing(12);
		layout =
			layout.push(text(format!("Browsing {} on {}", state.path, state.peer_id)).size(24));
		let controls = iced::widget::Row::new()
			.spacing(12)
			.push(button(text("Up")).on_press(GuiMessage::FileNavigateUp))
			.push(
				button(text("Back to actions"))
					.on_press(GuiMessage::PeerActionsRequested(state.peer_id.clone())),
			);
		layout = layout.push(controls);
		if state.loading {
			layout = layout.push(text("Loading directory...").size(16));
		} else if let Some(err) = &state.error {
			layout = layout.push(text(format!("Error: {}", err)).size(16));
		} else if state.entries.is_empty() {
			layout = layout.push(text("Directory is empty").size(16));
		} else {
			let mut list = iced::widget::Column::new().spacing(4);
			for entry in &state.entries {
				let label = if entry.is_dir {
					format!("[DIR] {}", entry.name)
				} else {
					format!("{} ({})", entry.name, format_size(entry.size))
				};
				let button = button(text(label))
					.width(Length::Fill)
					.on_press(GuiMessage::FileEntryActivated(entry.clone()));
				list = list.push(button);
			}
			layout = layout.push(scrollable(list).height(Length::Fill));
		}
		layout.into()
	}

	fn view_file_viewer(&self, state: &FileViewerState) -> Element<'_, GuiMessage> {
		let mut layout = iced::widget::Column::new().spacing(12);
		layout = layout.push(text(format!("Viewing {} on {}", state.path, state.peer_id)).size(24));
		let mut summary = format!("Loaded {} bytes", state.data.len());
		if state.eof {
			summary.push_str(" (end of file)");
		}
		layout = layout.push(text(summary).size(14));
		if let Some(err) = &state.error {
			layout = layout.push(text(format!("Error: {}", err)).size(14));
		}
		if !state.data.is_empty() {
			let (preview, lossy) = file_preview_text(&state.data);
			let mut preview_column = iced::widget::Column::new().spacing(4);
			if lossy {
				preview_column = preview_column.push(text("Binary data - non UTF-8 bytes replaced").size(12));
			}
			preview_column = preview_column.push(text(preview).size(14).width(Length::Fill));
			layout = layout.push(
				scrollable(
					container(preview_column)
						.padding(8)
						.style(theme::Container::Box),
				)
				.height(Length::Fill),
			);
		} else if state.loading {
			layout = layout.push(text("Loading file chunk...").size(14));
		} else if state.eof {
			layout = layout.push(text("File is empty").size(14));
		} else {
			layout = layout.push(text("No data loaded yet").size(14));
		}
		let mut controls = iced::widget::Row::new().spacing(12);
		if !state.eof {
			let label = if state.loading { "Loading..." } else { "Load more" };
			let mut load_btn = button(text(label));
			if !state.loading {
				load_btn = load_btn.on_press(GuiMessage::FileReadMore);
			}
			controls = controls.push(load_btn);
		}
		controls = controls.push(button(text("Back to browser")).on_press(GuiMessage::FileViewerBack));
		layout = layout.push(controls);
		layout.into()
	}

	fn view_graph(&self) -> Element<'_, GuiMessage> {
		let mut layout = iced::widget::Column::new().spacing(12);
		layout = layout.push(text("Peers Graph Overview").size(24));
		if self.graph.nodes.is_empty() {
			layout = layout.push(text("Graph is empty.").size(16));
		} else {
			if let Some(id) = self.graph.selected_id() {
				layout = layout.push(text(format!("Selected peer: {}", id)).size(16));
			}
			let mut list = iced::widget::Column::new().spacing(4);
			for node in &self.graph.nodes {
				let marker = if Some(node.id.as_str()) == self.graph.selected_id() {
					"▶"
				} else {
					""
				};
				list = list.push(
					text(format!(
						"{} {} (angle {:.2} rad)",
						marker, node.id, node.angle
					))
					.size(14),
				);
			}
			layout = layout.push(scrollable(list).height(Length::Fill));
			let action_message = self
				.graph
				.selected_id()
				.map(|id| GuiMessage::PeerActionsRequested(id.to_string()))
				.unwrap_or(GuiMessage::BackToPeers);
			let controls = iced::widget::Row::new()
				.spacing(12)
				.push(button(text("Previous")).on_press(GuiMessage::GraphPrev))
				.push(button(text("Next")).on_press(GuiMessage::GraphNext))
				.push(button(text("Open actions")).on_press(action_message));
			layout = layout.push(controls);
		}
		layout.into()
	}

	fn view_create_user(&self, form: &CreateUserForm) -> Element<'_, GuiMessage> {
		let mut layout = iced::widget::Column::new().spacing(12);
		layout = layout.push(text("Create User (placeholder)").size(24));
		layout = layout
			.push(text_input("username", &form.username).on_input(GuiMessage::UsernameChanged));
		layout = layout.push(
			text_input("password", &form.password)
				.secure(true)
				.on_input(GuiMessage::PasswordChanged),
		);
		layout = layout.push(button(text("Submit")).on_press(GuiMessage::CreateUserSubmit));
		if let Some(status) = &form.status {
			layout = layout.push(text(status).size(16));
		}
		layout.into()
	}

	fn view_file_search(&self, state: &FileSearchState) -> Element<'_, GuiMessage> {
		let mut layout = iced::widget::Column::new().spacing(12);
		layout = layout.push(text("File Search").size(24));
		// query input
		layout = layout.push(
			text_input("Search text (substring)", &state.query)
				.on_input(GuiMessage::FileSearchQueryChanged),
		);
		// mime pick list (simple typed filter) using available list; allow empty selection
		let mut mime_options = state.available_mime_types.clone();
		mime_options.sort();
		layout = layout.push(
			pick_list(
				mime_options,
				if state.selected_mime.is_empty() { None } else { Some(state.selected_mime.clone()) },
				|v| GuiMessage::FileSearchMimeChanged(v),
			)
			.placeholder("(any mime type)"),
		);
		// sort toggle
		let sort_label = if state.sort_desc { "Sort: Latest ↓" } else { "Sort: Latest ↑" };
		let controls_row = iced::widget::Row::new()
			.spacing(12)
			.push(button(text(sort_label)).on_press(GuiMessage::FileSearchToggleSort))
			.push(button(text("Search")).on_press(GuiMessage::FileSearchExecute));
		layout = layout.push(controls_row);
		if state.loading { return layout.push(text("Searching...")) .into(); }
		if let Some(err) = &state.error { return layout.push(text(format!("Error: {}", err))).into(); }
		if state.results.is_empty() { return layout.push(text("No results (run a search)")).into(); }
		let mut list = iced::widget::Column::new().spacing(4);
		for entry in &state.results {
			let row = iced::widget::Row::new()
				.spacing(8)
				.push(text(&abbreviate_hash(&entry.hash)).size(14).width(Length::FillPortion(2)))
				.push(text(entry.mime_type.clone().unwrap_or_else(|| "?".into())).size(14).width(Length::FillPortion(2)))
				.push(text(format_size(entry.size)).size(14).width(Length::FillPortion(1)))
				.push(text(entry.latest.clone()).size(14).width(Length::FillPortion(2)));
			list = list.push(container(row).padding(4).style(theme::Container::Box));
		}
		layout.push(scrollable(list).height(Length::Fill)).into()
	}

	fn gather_known_addresses(&self, peer_id: &str) -> Vec<String> {
		if let Some(state) = &self.latest_state {
			if let Ok(target) = PeerId::from_str(peer_id) {
				state
					.discovered_peers
					.iter()
					.filter(|p| p.peer_id == target)
					.map(|p| p.multiaddr.to_string())
					.collect()
			} else {
				Vec::new()
			}
		} else {
			Vec::new()
		}
	}
}

fn aggregate_peers(state: &State) -> Vec<PeerRow> {
	let mut rows: HashMap<String, PeerRow> = HashMap::new();
	for discovered in &state.discovered_peers {
		let id = format!("{}", discovered.peer_id);
		rows.entry(id.clone())
			.and_modify(|row| {
				if row.address.is_empty() {
					row.address = discovered.multiaddr.to_string();
				}
			})
			.or_insert(PeerRow {
				id,
				address: discovered.multiaddr.to_string(),
				status: String::from("discovered"),
			});
	}
	for connection in &state.connections {
		let id = format!("{}", connection.peer_id);
		rows.entry(id.clone())
			.and_modify(|row| {
				row.status = String::from("connected");
			})
			.or_insert(PeerRow {
				id,
				address: String::new(),
				status: String::from("connected"),
			});
	}
	for peer in &state.peers {
		let id = format!("{}", peer.id);
		rows.entry(id.clone()).or_insert(PeerRow {
			id,
			address: String::new(),
			status: String::new(),
		});
	}
	let me_id = format!("{}", state.me);
	rows.entry(me_id.clone())
		.and_modify(|row| {
			row.status = String::from("local");
			if row.address.is_empty() {
				row.address = LOCAL_LISTEN_MULTIADDR.into();
			}
		})
		.or_insert(PeerRow {
			id: me_id,
			address: LOCAL_LISTEN_MULTIADDR.into(),
			status: String::from("local"),
		});
	let mut vec: Vec<PeerRow> = rows.into_iter().map(|(_, row)| row).collect();
	vec.sort_by(|a, b| a.id.cmp(&b.id));
	vec
}

fn cpu_summary(cpus: &[CpuInfo]) -> String {
	if cpus.is_empty() {
		return String::from("No CPU information available");
	}
	let avg = cpus.iter().map(|cpu| cpu.usage).sum::<f32>() / cpus.len() as f32;
	let max = cpus.iter().map(|cpu| cpu.usage).fold(0.0, f32::max);
	format!("CPUs: {} — avg {:.1}% max {:.1}%", cpus.len(), avg, max)
}

fn format_frequency(freq: u64) -> String {
	if freq >= 1_000_000_000 {
		format!("{:.2} GHz", freq as f64 / 1_000_000_000.0)
	} else if freq >= 1_000_000 {
		format!("{:.2} MHz", freq as f64 / 1_000_000.0)
	} else {
		format!("{} Hz", freq)
	}
}

fn format_size(bytes: u64) -> String {
	const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
	let mut value = bytes as f64;
	let mut unit = 0usize;
	while value >= 1024.0 && unit + 1 < UNITS.len() {
		value /= 1024.0;
		unit += 1;
	}
	if unit == 0 {
		format!("{} {}", bytes, UNITS[unit])
	} else {
		format!("{:.2} {}", value, UNITS[unit])
	}
}

fn file_preview_text(data: &[u8]) -> (String, bool) {
	match std::str::from_utf8(data) {
		Ok(text) => (text.to_string(), false),
		Err(_) => (String::from_utf8_lossy(data).to_string(), true),
	}
}

fn abbreviate_peer_id(id: &str) -> String {
	const PREFIX: usize = 8;
	const SUFFIX: usize = 6;
	if id.len() <= PREFIX + SUFFIX + 1 {
		id.to_string()
	} else {
		format!("{}…{}", &id[..PREFIX], &id[id.len() - SUFFIX..])
	}
}

fn abbreviate_hash(hash_hex: &str) -> String {
	const PREFIX: usize = 8; const SUFFIX: usize = 8;
	if hash_hex.len() <= PREFIX + SUFFIX + 1 { hash_hex.to_string() } else { format!("{}…{}", &hash_hex[..PREFIX], &hash_hex[hash_hex.len()-SUFFIX..]) }
}

fn join_child_path(base: &str, child: &str) -> String {
	if base.is_empty() || base == "/" {
		format!("/{}", child.trim_start_matches('/'))
	} else {
		format!(
			"{}/{}",
			base.trim_end_matches('/'),
			child.trim_start_matches('/')
		)
	}
}

fn parent_path(path: &str) -> String {
	let trimmed = path.trim_end_matches('/');
	if trimmed.is_empty() || trimmed == "/" {
		return String::from("/");
	}
	if let Some(pos) = trimmed.rfind('/') {
		if pos == 0 {
			String::from("/")
		} else {
			trimmed[..pos].to_string()
		}
	} else {
		String::from("/")
	}
}

async fn fetch_cpus(
	peer: Arc<PuppyPeer>,
	peer_id: String,
) -> (String, Result<Vec<CpuInfo>, String>) {
	let result = match PeerId::from_str(&peer_id) {
		Ok(id) => peer.list_cpus(id).await.map_err(|err| err.to_string()),
		Err(err) => Err(err.to_string()),
	};
	(peer_id, result)
}

async fn fetch_dir_entries(
	peer: Arc<PuppyPeer>,
	local_peer_id: Option<String>,
	peer_id: String,
	path: String,
) -> (String, String, Result<Vec<DirEntry>, String>) {
	let path_clone = path.clone();
	let result = if local_peer_id.as_deref() == Some(peer_id.as_str()) {
		peer.list_dir_local(path_clone)
			.await
			.map_err(|err| err.to_string())
	} else {
		match PeerId::from_str(&peer_id) {
			Ok(target) => peer
				.list_dir_remote(target, path_clone)
				.await
				.map_err(|err| err.to_string()),
			Err(err) => Err(err.to_string()),
		}
	};
	(peer_id, path, result)
}

async fn fetch_file_chunk(
	peer: Arc<PuppyPeer>,
	local_peer_id: Option<String>,
	peer_id: String,
	path: String,
	offset: u64,
	length: u64,
) -> (String, String, u64, Result<FileChunk, String>) {
	let path_clone = path.clone();
	let result = if local_peer_id.as_deref() == Some(peer_id.as_str()) {
		peer
			.read_file_local(path_clone, offset, Some(length))
			.await
			.map_err(|err| err.to_string())
	} else {
		match PeerId::from_str(&peer_id) {
			Ok(target) => peer
				.read_file_remote(target, path_clone, offset, Some(length))
				.await
				.map_err(|err| err.to_string()),
			Err(err) => Err(err.to_string()),
		}
	};
	(peer_id, path, offset, result)
}

async fn search_files(
	_peer: Arc<PuppyPeer>,
	_query: String,
	mime: Option<String>,
	sort_desc: bool,
) -> Result<(Vec<FileSearchEntry>, Vec<String>), String> {
	// Placeholder in-memory search over local sqlite not yet wired: return empty until DB API exposed.
	// For now, we just simulate no results but allow UI to function.
	let _ = (_query, mime, sort_desc); // suppress warnings
	Ok((Vec::new(), Vec::new()))
}

pub fn run() -> iced::Result {
	let mut settings = Settings::default();
	settings.window.size = iced::Size::new(1024.0, 720.0);
	GuiApp::run(settings)
}

#[cfg(test)]
mod tests {
	use super::*;

	use libp2p::PeerId;

	fn with_runtime<T>(test: impl FnOnce() -> T) -> T {
		let runtime = tokio::runtime::Runtime::new().expect("runtime");
		let guard = runtime.enter();
		let result = test();
		drop(guard);
		runtime.shutdown_background();
		result
	}

	#[test]
	fn selecting_peers_refreshes_from_state() {
		with_runtime(|| {
			let (mut app, _) = GuiApp::new(());
			let new_peer = PeerId::random();
			{
				let state = app.peer.state();
				let mut guard = state.lock().expect("state lock");
				guard.peer_discovered(new_peer, "/ip4/127.0.0.1/tcp/7000".parse().unwrap());
			}
			app.peers.clear();
			let _ = app.update(GuiMessage::MenuSelected(MenuItem::Peers));
			assert!(matches!(app.mode, Mode::Peers));
			assert!(app.peers.iter().any(|row| row.id == new_peer.to_string()));
			assert!(app.status.contains("Showing peers"));
		});
	}

	#[test]
	fn selecting_graph_rebuilds_nodes() {
		with_runtime(|| {
			let (mut app, _) = GuiApp::new(());
			let new_peer = PeerId::random();
			{
				let state = app.peer.state();
				let mut guard = state.lock().expect("state lock");
				guard.peer_discovered(new_peer, "/ip4/127.0.0.1/tcp/8000".parse().unwrap());
			}
			app.graph.nodes.clear();
			let _ = app.update(GuiMessage::MenuSelected(MenuItem::PeersGraph));
			assert!(matches!(app.mode, Mode::PeersGraph));
			assert!(
				app.graph
					.nodes
					.iter()
					.any(|node| node.id == new_peer.to_string())
			);
			assert!(app.status.contains("Graph overview"));
		});
	}
}
