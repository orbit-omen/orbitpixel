use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind};
use tokio::sync::mpsc::UnboundedSender;

use crate::html::Document;
use crate::renderer::{render_document, RenderedPage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    Text,
    Color,
    Pixel,
}

impl RenderMode {
    pub fn label(self) -> &'static str {
        match self {
            RenderMode::Text => "TEXT",
            RenderMode::Color => "COLOR",
            RenderMode::Pixel => "PIXEL",
        }
    }

    pub fn cycle(self) -> Self {
        match self {
            RenderMode::Text => RenderMode::Color,
            RenderMode::Color => RenderMode::Pixel,
            RenderMode::Pixel => RenderMode::Text,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    InsertUrl,
}

#[derive(Debug)]
pub enum AppCommand {
    FetchUrl {
        tab_id: usize,
        request_id: u64,
        raw_url: String,
        push_history: bool,
    },
}

#[derive(Debug)]
pub enum AppEvent {
    PageLoaded {
        tab_id: usize,
        request_id: u64,
        final_url: String,
        status_code: u16,
        document: Document,
        push_history: bool,
    },
    PageError {
        tab_id: usize,
        request_id: u64,
        url: String,
        error: String,
    },
}

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub url: String,
    pub scroll_y: u16,
}

pub struct TabState {
    pub id: usize,
    pub url: String,
    pub document: Document,
    pub back_stack: Vec<HistoryEntry>,
    pub forward_stack: Vec<HistoryEntry>,
    pub scroll_y: u16,
    pub focused_link: Option<usize>,
    pub rendered: RenderedPage,
    pub layout_dirty: bool,
    pub last_width: u16,
    pub is_loading: bool,
    pub active_request_id: u64,
    pub status_text: String,
}

impl TabState {
    pub fn new(id: usize) -> Self {
        let doc = Document::welcome();
        Self {
            id,
            url: "https://orbitpixel.local/".to_string(),
            document: doc,
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
            scroll_y: 0,
            focused_link: None,
            rendered: RenderedPage::default(),
            layout_dirty: true,
            last_width: 0,
            is_loading: false,
            active_request_id: 0,
            status_text: "Ready".to_string(),
        }
    }
}

pub struct App {
    pub mode: RenderMode,
    pub input_mode: InputMode,
    pub url_input: String,
    pub tabs: Vec<TabState>,
    pub active_tab: usize,
    pub next_tab_id: usize,
    pub next_request_id: u64,
    pub viewport_height: u16,
    pub should_quit: bool,
    cmd_tx: UnboundedSender<AppCommand>,
}

impl App {
    pub fn new(mode: RenderMode, cmd_tx: UnboundedSender<AppCommand>) -> Self {
        let initial_tab = TabState::new(1);
        let initial_url = initial_tab.url.clone();
        Self {
            mode,
            input_mode: InputMode::Normal,
            url_input: initial_url,
            tabs: vec![initial_tab],
            active_tab: 0,
            next_tab_id: 2,
            next_request_id: 1,
            viewport_height: 20,
            should_quit: false,
            cmd_tx,
        }
    }

    pub fn current_tab(&self) -> &TabState {
        &self.tabs[self.active_tab]
    }

    pub fn current_tab_mut(&mut self) -> &mut TabState {
        &mut self.tabs[self.active_tab]
    }

    pub fn ensure_layout(&mut self, width: u16, height: u16) {
        self.viewport_height = height;
        let mode = self.mode;
        let tab = self.current_tab_mut();
        if tab.layout_dirty || tab.last_width != width {
            tab.rendered = render_document(&tab.document, width, mode, tab.focused_link);
            tab.last_width = width;
            tab.layout_dirty = false;
        }
    }

    pub fn invalidate_all_layouts(&mut self) {
        for tab in &mut self.tabs {
            tab.layout_dirty = true;
        }
    }

    pub fn navigate_active(&mut self, raw_url: &str, push_history: bool) {
        let trimmed = raw_url.trim().to_string();
        if trimmed.is_empty() {
            return;
        }
        let req_id = self.next_request_id;
        self.next_request_id += 1;

        let tab = self.current_tab_mut();
        if push_history && !tab.url.is_empty() && tab.url != trimmed {
            tab.back_stack.push(HistoryEntry {
                url: tab.url.clone(),
                scroll_y: tab.scroll_y,
            });
            tab.forward_stack.clear();
        }
        tab.url = trimmed.clone();
        tab.is_loading = true;
        tab.active_request_id = req_id;
        tab.status_text = format!("Loading {}...", trimmed);
        self.url_input = trimmed.clone();

        let tab_id = tab.id;
        let _ = self.cmd_tx.send(AppCommand::FetchUrl {
            tab_id,
            request_id: req_id,
            raw_url: trimmed,
            push_history: false,
        });
    }

    pub fn handle_background_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::PageLoaded {
                tab_id,
                request_id,
                final_url,
                status_code,
                document,
                ..
            } => {
                if let Some((idx, tab)) = self
                    .tabs
                    .iter_mut()
                    .enumerate()
                    .find(|(_, t)| t.id == tab_id && t.active_request_id == request_id)
                {
                    tab.url = final_url.clone();
                    tab.document = document;
                    tab.scroll_y = 0;
                    tab.focused_link = None;
                    tab.layout_dirty = true;
                    tab.is_loading = false;
                    tab.status_text = format!("HTTP {} | Ready", status_code);
                    if idx == self.active_tab && self.input_mode == InputMode::Normal {
                        self.url_input = final_url;
                    }
                }
            }
            AppEvent::PageError {
                tab_id,
                request_id,
                url,
                error,
            } => {
                if let Some((_, tab)) = self
                    .tabs
                    .iter_mut()
                    .enumerate()
                    .find(|(_, t)| t.id == tab_id && t.active_request_id == request_id)
                {
                    tab.document = Document::error_page(&url, &error);
                    tab.scroll_y = 0;
                    tab.focused_link = None;
                    tab.layout_dirty = true;
                    tab.is_loading = false;
                    tab.status_text = format!("Error: {}", error);
                }
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match self.input_mode {
            InputMode::InsertUrl => self.handle_insert_key(key),
            InputMode::Normal => self.handle_normal_key(key),
        }
    }

    fn handle_insert_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.url_input = self.current_tab().url.clone();
            }
            KeyCode::Enter => {
                self.input_mode = InputMode::Normal;
                let target = self.url_input.clone();
                self.navigate_active(&target, true);
            }
            KeyCode::Backspace => {
                self.url_input.pop();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.url_input.clear();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.url_input.push(c);
            }
            _ => {}
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.input_mode = InputMode::InsertUrl;
                self.url_input = self.current_tab().url.clone();
            }
            KeyCode::Char('i') => {
                self.input_mode = InputMode::InsertUrl;
                self.url_input = self.current_tab().url.clone();
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let url = self.current_tab().url.clone();
                self.navigate_active(&url, false);
            }
            KeyCode::Char('m') => {
                self.mode = self.mode.cycle();
                self.invalidate_all_layouts();
            }
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let id = self.next_tab_id;
                self.next_tab_id += 1;
                let new_tab = TabState::new(id);
                self.url_input = new_tab.url.clone();
                self.tabs.push(new_tab);
                self.active_tab = self.tabs.len() - 1;
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.tabs.len() > 1 {
                    self.tabs.remove(self.active_tab);
                    if self.active_tab >= self.tabs.len() {
                        self.active_tab = self.tabs.len() - 1;
                    }
                    self.url_input = self.current_tab().url.clone();
                } else {
                    self.should_quit = true;
                }
            }
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.active_tab = (self.active_tab + 1) % self.tabs.len();
                self.url_input = self.current_tab().url.clone();
            }
            KeyCode::BackTab if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.active_tab = if self.active_tab == 0 {
                    self.tabs.len() - 1
                } else {
                    self.active_tab - 1
                };
                self.url_input = self.current_tab().url.clone();
            }
            KeyCode::Left if key.modifiers.contains(KeyModifiers::ALT) => self.go_back(),
            KeyCode::Right if key.modifiers.contains(KeyModifiers::ALT) => self.go_forward(),
            KeyCode::Down | KeyCode::Char('j') => self.scroll_by(1),
            KeyCode::Up | KeyCode::Char('k') => self.scroll_by(-1),
            KeyCode::PageDown | KeyCode::Char(' ') => {
                let step = self.viewport_height.saturating_sub(2).max(1) as i32;
                self.scroll_by(step);
            }
            KeyCode::PageUp => {
                let step = self.viewport_height.saturating_sub(2).max(1) as i32;
                self.scroll_by(-step);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.current_tab_mut().scroll_y = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                let max_scroll = self.max_scroll();
                self.current_tab_mut().scroll_y = max_scroll;
            }
            KeyCode::Tab => self.cycle_link(true),
            KeyCode::BackTab => self.cycle_link(false),
            KeyCode::Enter => self.activate_focused_link(),
            _ => {}
        }
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::ScrollDown => self.scroll_by(3),
            MouseEventKind::ScrollUp => self.scroll_by(-3),
            MouseEventKind::Down(_) => {
                // Viewport starts at row 4 (3 rows tab bar + 3 rows address bar - 2 shared borders)
                if mouse.row >= 4 {
                    let line_idx =
                        (mouse.row.saturating_sub(4) + self.current_tab().scroll_y) as usize;
                    let col = mouse.column.saturating_sub(1) as usize;
                    let clicked_url = self
                        .current_tab()
                        .rendered
                        .links
                        .iter()
                        .find(|l| {
                            l.line_index == line_idx && col >= l.col_start && col <= l.col_end
                        })
                        .map(|l| l.url.clone());

                    if let Some(url) = clicked_url {
                        self.input_mode = InputMode::Normal;
                        self.navigate_active(&url, true);
                    }
                }
            }
            _ => {}
        }
    }

    fn max_scroll(&self) -> u16 {
        let total_lines = self.current_tab().rendered.lines.len() as u16;
        total_lines.saturating_sub(self.viewport_height)
    }

    fn scroll_by(&mut self, delta: i32) {
        let max_s = self.max_scroll() as i32;
        let current = self.current_tab().scroll_y as i32;
        let next = (current + delta).clamp(0, max_s) as u16;
        self.current_tab_mut().scroll_y = next;
    }

    fn cycle_link(&mut self, forward: bool) {
        let vp_height = self.viewport_height;
        let tab = self.current_tab_mut();
        let count = tab.rendered.links.len();
        if count == 0 {
            return;
        }
        let next = match (tab.focused_link, forward) {
            (None, true) => 0,
            (None, false) => count - 1,
            (Some(i), true) => (i + 1) % count,
            (Some(0), false) => count - 1,
            (Some(i), false) => i - 1,
        };
        tab.focused_link = Some(next);
        tab.layout_dirty = true;

        let link_line = tab.rendered.links[next].line_index as u16;
        if link_line < tab.scroll_y {
            tab.scroll_y = link_line;
        } else if link_line >= tab.scroll_y + vp_height {
            tab.scroll_y = link_line.saturating_sub(vp_height / 2);
        }
    }

    fn activate_focused_link(&mut self) {
        let target = {
            let tab = self.current_tab();
            tab.focused_link
                .and_then(|idx| tab.rendered.links.get(idx))
                .map(|l| l.url.clone())
        };
        if let Some(url) = target {
            self.navigate_active(&url, true);
        }
    }

    fn go_back(&mut self) {
        let entry = {
            let tab = self.current_tab_mut();
            let prev = tab.back_stack.pop()?;
            tab.forward_stack.push(HistoryEntry {
                url: tab.url.clone(),
                scroll_y: tab.scroll_y,
            });
            prev
        };
        self.navigate_active(&entry.url, false);
    }

    fn go_forward(&mut self) {
        let entry = {
            let tab = self.current_tab_mut();
            let next = tab.forward_stack.pop()?;
            tab.back_stack.push(HistoryEntry {
                url: tab.url.clone(),
                scroll_y: tab.scroll_y,
            });
            next
        };
        self.navigate_active(&entry.url, false);
    }
}
