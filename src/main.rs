// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    app::{message, Command, Core, Settings},
    executor,
    iced::{
        widget::{row, text},
        Alignment, Length, Limits,
    },
    style,
    widget::{self, button, icon, nav_bar, segmented_button, view_switcher},
    ApplicationExt, Element,
};
use cosmic_text::{FontSystem, SyntaxSystem, ViMode};
use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use self::menu::menu_bar;
mod menu;

use self::project::ProjectNode;
mod project;

use self::tab::Tab;
mod tab;

use self::text_box::text_box;
mod text_box;

//TODO: re-use iced FONT_SYSTEM
lazy_static::lazy_static! {
    static ref FONT_SYSTEM: Mutex<FontSystem> = Mutex::new(FontSystem::new());
    static ref SYNTAX_SYSTEM: SyntaxSystem = SyntaxSystem::new();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let settings = Settings::default().size_limits(Limits::NONE.min_width(400.0).min_height(200.0));
    let flags = ();
    cosmic::app::run::<App>(settings, flags)?;

    Ok(())
}

#[derive(Clone, Debug)]
pub struct Config {
    wrap: bool,
}

impl Config {
    //TODO: load from cosmic-config
    pub fn new() -> Self {
        Self { wrap: false }
    }
}

pub struct App {
    core: Core,
    nav_model: segmented_button::SingleSelectModel,
    tab_model: segmented_button::SingleSelectModel,
    config: Config,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum Message {
    New,
    OpenDialog,
    Open(PathBuf),
    Save,
    TabActivate(segmented_button::Entity),
    TabClose(segmented_button::Entity),
    Todo,
    Wrap(bool),
}

impl App {
    pub fn active_tab(&self) -> Option<&Tab> {
        self.tab_model.active_data()
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tab_model.active_data_mut()
    }

    fn open_folder<P: AsRef<Path>>(&mut self, path: P, mut position: u16, indent: u16) {
        let read_dir = match fs::read_dir(&path) {
            Ok(ok) => ok,
            Err(err) => {
                log::error!("failed to read directory {:?}: {}", path.as_ref(), err);
                return;
            }
        };

        let mut nodes = Vec::new();
        for entry_res in read_dir {
            let entry = match entry_res {
                Ok(ok) => ok,
                Err(err) => {
                    log::error!(
                        "failed to read entry in directory {:?}: {}",
                        path.as_ref(),
                        err
                    );
                    continue;
                }
            };

            let entry_path = entry.path();
            let node = match ProjectNode::new(&entry_path) {
                Ok(ok) => ok,
                Err(err) => {
                    log::error!(
                        "failed to open directory {:?} entry {:?}: {}",
                        path.as_ref(),
                        entry_path,
                        err
                    );
                    continue;
                }
            };
            nodes.push(node);
        }

        nodes.sort();

        for node in nodes {
            self.nav_model
                .insert()
                .position(position)
                .indent(indent)
                .icon(icon::from_name(node.icon_name()).size(16).icon())
                .text(node.name().to_string())
                .data(node);

            position += 1;
        }
    }

    pub fn open_project<P: AsRef<Path>>(&mut self, path: P) {
        let node = match ProjectNode::new(&path) {
            Ok(mut node) => {
                match &mut node {
                    ProjectNode::Folder { open, root, .. } => {
                        *open = true;
                        *root = true;
                    }
                    _ => {
                        log::error!(
                            "failed to open project {:?}: not a directory",
                            path.as_ref()
                        );
                        return;
                    }
                }
                node
            }
            Err(err) => {
                log::error!("failed to open project {:?}: {}", path.as_ref(), err);
                return;
            }
        };

        let id = self
            .nav_model
            .insert()
            .icon(icon::from_name(node.icon_name()).size(16).icon())
            .text(node.name().to_string())
            .data(node)
            .id();

        let position = self.nav_model.position(id).unwrap_or(0);

        self.open_folder(&path, position + 1, 1);
    }

    pub fn open_tab(&mut self, path_opt: Option<PathBuf>) {
        let mut tab = Tab::new();
        tab.set_config(&self.config);
        if let Some(path) = path_opt {
            tab.open(path);
        }
        self.tab_model
            .insert()
            .text(tab.title())
            .icon(icon::from_name("text-x-generic").size(16).icon())
            .data::<Tab>(tab)
            .closable()
            .activate();
    }

    pub fn update_title(&mut self) -> Command<Message> {
        let title = match self.active_tab() {
            Some(tab) => tab.title(),
            None => format!("No Open File"),
        };
        let window_title = format!("{title} - COSMIC Text Editor");
        self.set_header_title(title.clone());
        self.set_window_title(window_title)
    }
}

/// Implement [`cosmic::Application`] to integrate with COSMIC.
impl cosmic::Application for App {
    /// Default async executor to use with the app.
    type Executor = executor::Default;

    /// Argument received [`cosmic::Application::new`].
    type Flags = ();

    /// Message type specific to our [`App`].
    type Message = Message;

    /// The unique application ID to supply to the window manager.
    const APP_ID: &'static str = "com.system76.CosmicTextEditor";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    /// Creates the application, and optionally emits command on initialize.
    fn init(core: Core, _flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let mut app = App {
            core,
            nav_model: nav_bar::Model::builder().build(),
            tab_model: segmented_button::Model::builder().build(),
            config: Config::new(),
        };

        for arg in env::args().skip(1) {
            let path = PathBuf::from(arg);
            if path.is_dir() {
                app.open_project(path);
            } else {
                app.open_tab(Some(path));
            }
        }

        // Show nav bar only if project is provided
        if app.core.nav_bar_active() != app.nav_model.iter().next().is_some() {
            app.core.nav_bar_toggle();
        }

        // Open an empty file if no arguments provided
        if app.tab_model.iter().next().is_none() {
            app.open_tab(None);
        }

        let command = app.update_title();
        (app, command)
    }

    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav_model)
    }

    fn on_nav_select(&mut self, id: nav_bar::Id) -> Command<Message> {
        let node = match self.nav_model.data_mut::<ProjectNode>(id) {
            Some(node) => {
                match node {
                    ProjectNode::Folder { open, .. } => {
                        *open = !*open;
                    }
                    _ => {}
                }
                node.clone()
            }
            None => {
                log::warn!("no path found for id {:?}", id);
                return Command::none();
            }
        };

        self.nav_model
            .icon_set(id, icon::from_name(node.icon_name()).size(16).icon());

        match node {
            ProjectNode::Folder { path, open, .. } => {
                let position = self.nav_model.position(id).unwrap_or(0);
                let indent = self.nav_model.indent(id).unwrap_or(0);
                if open {
                    self.open_folder(path, position + 1, indent + 1);
                } else {
                    loop {
                        let child_id = match self.nav_model.entity_at(position + 1) {
                            Some(some) => some,
                            None => break,
                        };

                        if self.nav_model.indent(child_id).unwrap_or(0) > indent {
                            self.nav_model.remove(child_id);
                        } else {
                            break;
                        }
                    }
                }
                Command::none()
            }
            ProjectNode::File { path, .. } => {
                //TODO: go to already open file if possible
                self.open_tab(Some(path.clone()));
                self.update_title()
            }
        }
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::New => {
                self.open_tab(None);
                return self.update_title();
            }
            Message::OpenDialog => {
                return Command::perform(
                    async {
                        if let Some(handle) = rfd::AsyncFileDialog::new().pick_file().await {
                            message::app(Message::Open(handle.path().to_owned()))
                        } else {
                            message::none()
                        }
                    },
                    |x| x,
                );
            }
            Message::Open(path) => {
                self.open_tab(Some(path));
                return self.update_title();
            }
            Message::Save => {
                let mut title_opt = None;

                match self.active_tab_mut() {
                    Some(tab) => {
                        if tab.path_opt.is_none() {
                            //TODO: use async file dialog
                            tab.path_opt = rfd::FileDialog::new().save_file();
                            title_opt = Some(tab.title());
                        }
                        tab.save();
                    }
                    None => {
                        log::warn!("TODO: NO TAB OPEN");
                    }
                }

                if let Some(title) = title_opt {
                    self.tab_model.text_set(self.tab_model.active(), title);
                }
            }
            Message::TabActivate(entity) => {
                self.tab_model.activate(entity);
                return self.update_title();
            }
            Message::TabClose(entity) => {
                // Activate closest item
                if let Some(position) = self.tab_model.position(entity) {
                    if position > 0 {
                        self.tab_model.activate_position(position - 1);
                    } else {
                        self.tab_model.activate_position(position + 1);
                    }
                }

                // Remove item
                self.tab_model.remove(entity);

                // If that was the last tab, make a new empty one
                if self.tab_model.iter().next().is_none() {
                    self.open_tab(None);
                }

                return self.update_title();
            }
            Message::Todo => {
                log::warn!("TODO");
            }
            Message::Wrap(wrap) => {
                self.config.wrap = wrap;
                //TODO: provide iterator over data
                let entities: Vec<_> = self.tab_model.iter().collect();
                for entity in entities {
                    if let Some(tab) = self.tab_model.data_mut::<Tab>(entity) {
                        tab.set_config(&self.config);
                    }
                }
            }
        }

        Command::none()
    }

    fn header_start(&self) -> Vec<Element<Message>> {
        vec![menu_bar(&self.config)]
    }

    fn view(&self) -> Element<Message> {
        let mut tab_column = widget::column::with_capacity(3).padding([0, 16]);

        tab_column = tab_column.push(
            row![
                view_switcher::horizontal(&self.tab_model)
                    .on_activate(Message::TabActivate)
                    .on_close(Message::TabClose)
                    .width(Length::Shrink),
                button(icon::from_name("list-add-symbolic").size(16).icon())
                    .on_press(Message::New)
                    .padding(8)
                    .style(style::Button::Icon)
            ]
            .align_items(Alignment::Center),
        );

        match self.active_tab() {
            Some(tab) => {
                tab_column = tab_column.push(text_box(&tab.editor).padding(8));
                let status = match tab.editor.lock().unwrap().mode() {
                    ViMode::Passthrough => {
                        //TODO: status line
                        String::new()
                    }
                    ViMode::Normal => {
                        //TODO: status line
                        String::new()
                    }
                    ViMode::Insert => {
                        format!("-- INSERT --")
                    }
                    ViMode::Command { value } => {
                        format!(":{value}|")
                    }
                    ViMode::Search { value, forwards } => {
                        if *forwards {
                            format!("/{value}|")
                        } else {
                            format!("?{value}|")
                        }
                    }
                };
                tab_column = tab_column.push(text(status).font(cosmic::font::Font::MONOSPACE));
            }
            None => {
                log::warn!("TODO: No tab open");
            }
        };

        let content: Element<_> = tab_column.into();

        // Uncomment to debug layout:
        //content.explain(cosmic::iced::Color::WHITE)
        content
    }
}
