use bevy::prelude::*;
use bevy_egui::{egui, EguiContext};
use cfg_if::cfg_if;
use chrono::Utc;
use enum_iterator::all;
use meru_interface::{File, MultiKey, SingleKey};
use schemars::{
    schema::{InstanceType, RootSchema, Schema, SchemaObject, SingleOrVec},
    visit::{visit_schema, Visitor},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::{
    app::{AppState, FullscreenState, ShowMessage, WindowControlEvent},
    config::{Config, PersistentState, RecentFile, SystemKey, SystemKeys},
    core::{Emulator, StateFile, ARCHIVE_EXTENSIONS, EMULATOR_CORES},
    hotkey::{HotKey, HotKeys},
    input::ConvertInput,
    utils::{spawn_local, unbounded_channel, Receiver, Sender},
};

pub const MENU_WIDTH: usize = 1280;
pub const MENU_HEIGHT: usize = 768;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_system_set(SystemSet::on_enter(AppState::Menu).with_system(setup_menu_system))
            .add_system_set(
                SystemSet::on_update(AppState::Menu)
                    .with_system(menu_system)
                    .with_system(menu_event_system),
            )
            .add_system_set(SystemSet::on_exit(AppState::Menu).with_system(menu_exit))
            .add_event::<MenuEvent>();
    }
}

pub enum MenuEvent {
    OpenRomFile {
        path: PathBuf,
        data: Vec<u8>,
    },
    OpenRomDone {
        recent: RecentFile,
        result: anyhow::Result<Emulator>,
    },
    StateSaved {
        slot: usize,
    },
    StateLoaded {
        slot: usize,
        data: anyhow::Result<Vec<u8>>,
    },
}

struct ConfigValue {
    abbrev: String,
    value: Value,
}

struct ConfigChannel {
    receiver: Receiver<ConfigValue>,
    sender: Sender<ConfigValue>,
}

impl ConfigChannel {
    fn new() -> Self {
        let (sender, receiver) = unbounded_channel();
        Self { receiver, sender }
    }
}

struct MenuError {
    title: String,
    message: String,
}

fn setup_menu_system(
    mut commands: Commands,
    #[cfg(not(target_arch = "wasm32"))] mut windows: ResMut<Windows>,
    fullscreen_state: Res<FullscreenState>,
) {
    if !fullscreen_state.0 {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let window = windows.get_primary_mut().unwrap();
            window.set_resolution(MENU_WIDTH as f32, MENU_HEIGHT as f32);
        }
    }

    commands.insert_resource(MenuState::default());
    commands.insert_resource(None as Option<MenuError>);

    let (s, r) = unbounded_channel::<MenuEvent>();
    commands.insert_resource(s);
    commands.insert_resource(r);

    commands.insert_resource(ConfigChannel::new());
}

fn menu_exit(config: Res<Config>) {
    let config = config.clone();
    spawn_local(async move { config.save().await.unwrap() });
}

#[allow(clippy::too_many_arguments)]
fn menu_event_system(
    mut commands: Commands,
    mut emulator: Option<ResMut<Emulator>>,
    recv: Res<Receiver<MenuEvent>>,
    send: Res<Sender<MenuEvent>>,
    mut app_state: ResMut<State<AppState>>,
    mut persistent_state: ResMut<PersistentState>,
    mut menu_error: ResMut<Option<MenuError>>,
    mut message_event: EventWriter<ShowMessage>,
    config: Res<Config>,
) {
    while let Ok(event) = recv.try_recv() {
        match event {
            MenuEvent::OpenRomFile { path, data } => {
                let config = config.clone();
                let send = send.clone();

                let recent = RecentFile {
                    path: path.clone(),
                    #[cfg(target_arch = "wasm32")]
                    data: data.clone(),
                };

                let fut = async move {
                    info!("Opening file: {:?}", path);
                    let result = Emulator::try_new_from_bytes(&path, data, &config).await;
                    send.send(MenuEvent::OpenRomDone { recent, result }).await?;
                    Ok::<(), anyhow::Error>(())
                };

                spawn_local(async move {
                    fut.await.unwrap();
                });
            }
            MenuEvent::OpenRomDone { recent, result } => match result {
                Ok(emulator) => {
                    commands.insert_resource(emulator);

                    persistent_state.add_recent(recent);
                    let fut = persistent_state.save();
                    spawn_local(async move {
                        fut.await.unwrap();
                    });
                    app_state.set(AppState::Running).unwrap();
                }
                Err(err) => {
                    *menu_error.as_mut() = Some(MenuError {
                        title: "Failed to open ROM".into(),
                        message: err.to_string(),
                    });
                }
            },
            MenuEvent::StateSaved { slot } => {
                if let Some(emulator) = emulator.as_deref_mut() {
                    let state_file = StateFile {
                        modified: Utc::now().into(),
                    };
                    emulator.state_files[slot] = Some(state_file);
                }
                message_event.send(ShowMessage(format!("State saved: #{slot}")));
            }
            MenuEvent::StateLoaded { slot, data } => {
                let f = || -> anyhow::Result<()> {
                    let data = data?;
                    let emulator = emulator
                        .as_deref_mut()
                        .ok_or_else(|| anyhow::anyhow!("No emulator instance"))?;
                    emulator.load_state_data(&data)?;
                    Ok(())
                };

                match f() {
                    Ok(_) => {
                        message_event.send(ShowMessage(format!("State loaded: #{slot}")));
                    }
                    Err(e) => {
                        message_event.send(ShowMessage(format!(
                            "Failed to load state from slot #{slot}: {e}"
                        )));
                    }
                }
                app_state.set(AppState::Running).unwrap();
            }
        }
    }
}

#[derive(PartialEq, Eq, Clone)]
enum MenuTab {
    File,
    State,
    GameInfo,
    GeneralSetting,
    CoreSetting(String),
    ControllerSetting(String),
    Graphics,
    HotKey,
    SystemKey,
}

#[derive(PartialEq, Eq)]
enum ControllerTab {
    Keyboard,
    Gamepad,
}

struct MenuState {
    tab: MenuTab,
    controller_tab: ControllerTab,
    controller_ix: usize,
    controller_button_ix: usize,
    hotkey_select: usize,
    constructing_hotkey: Option<Vec<SingleKey>>,
    system_key_tab: ControllerTab,
    system_key_ix: usize,
}

impl Default for MenuState {
    fn default() -> Self {
        MenuState {
            tab: MenuTab::File,
            controller_tab: ControllerTab::Keyboard,
            controller_ix: 0,
            controller_button_ix: 0,
            hotkey_select: 0,
            constructing_hotkey: None,
            system_key_tab: ControllerTab::Keyboard,
            system_key_ix: 0,
        }
    }
}

impl MenuState {
    fn tab_selector(&mut self, ui: &mut egui::Ui, emulator_loaded: bool) {
        ui.selectable_value(&mut self.tab, MenuTab::File, "📁 File");

        ui.add_enabled_ui(emulator_loaded, |ui| {
            ui.selectable_value(&mut self.tab, MenuTab::State, "💾 State Save/Load");
        });

        ui.add_enabled_ui(emulator_loaded, |ui| {
            ui.selectable_value(&mut self.tab, MenuTab::GameInfo, "ℹ Game Info");
        });

        ui.selectable_value(&mut self.tab, MenuTab::GeneralSetting, "🔧 General Setting");
        ui.selectable_value(&mut self.tab, MenuTab::Graphics, "🖼 Graphics");

        ui.collapsing("⚙ Core Setting", |ui| {
            for core_info in Emulator::core_infos() {
                ui.selectable_value(
                    &mut self.tab,
                    MenuTab::CoreSetting(core_info.abbrev.into()),
                    core_info.system_name,
                );
            }
        });
        ui.collapsing("🎮 Controller Setting", |ui| {
            for core_info in Emulator::core_infos() {
                ui.selectable_value(
                    &mut self.tab,
                    MenuTab::ControllerSetting(core_info.abbrev.into()),
                    core_info.system_name,
                );
            }
        });

        ui.selectable_value(&mut self.tab, MenuTab::HotKey, "⌨ Hotkey");
        ui.selectable_value(&mut self.tab, MenuTab::SystemKey, "💻 System Key");
    }

    fn tab_controller(
        &mut self,
        ui: &mut egui::Ui,
        config: &mut Config,
        core: &str,
        key_code_input: &Input<KeyCode>,
        gamepad_button_input: &Input<GamepadButton>,
    ) {
        let mut key_config = config.key_config(core).clone();

        if self.controller_ix >= key_config.controllers.len() {
            self.controller_ix = 0;
        }

        ui.horizontal(|ui| {
            for i in 0..key_config.controllers.len() {
                let resp = ui.selectable_value(&mut self.controller_ix, i, format!("Pad{}", i + 1));
                if resp.clicked() {
                    self.controller_button_ix = 0;
                }
            }
        });

        ui.horizontal(|ui| {
            let mut resp = ui.selectable_value(
                &mut self.controller_tab,
                ControllerTab::Keyboard,
                "Keyboard",
            );
            resp |=
                ui.selectable_value(&mut self.controller_tab, ControllerTab::Gamepad, "Gamepad");
            if resp.clicked() {
                self.controller_button_ix = 0;
            }
        });

        ui.group(|ui| {
            let grid = egui::Grid::new("key_config")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true);

            grid.show(ui, |ui| {
                ui.label("Button");
                ui.label("Assignment");
                ui.end_row();

                ui.separator();
                ui.separator();
                ui.end_row();

                let mut changed: Option<usize> = None;

                match self.controller_tab {
                    ControllerTab::Keyboard => {
                        for (ix, (name, assign)) in key_config.controllers[self.controller_ix]
                            .iter_mut()
                            .enumerate()
                        {
                            let ix = ix + 1;
                            ui.label(name.clone());
                            let assign_str = assign
                                .extract_keycode()
                                .map_or_else(|| "".to_string(), |k| format!("{k:?}"));

                            ui.selectable_value(&mut self.controller_button_ix, ix, assign_str)
                                .on_hover_text("Click and type the key you want to assign");

                            if self.controller_button_ix == ix {
                                if let Some(kc) = key_code_input.get_just_pressed().next() {
                                    assign.insert_keycode(ConvertInput(*kc).into());
                                    changed = Some(ix);
                                }
                            }

                            ui.end_row();
                        }
                    }

                    ControllerTab::Gamepad => {
                        for (ix, (name, assign)) in key_config.controllers[self.controller_ix]
                            .iter_mut()
                            .enumerate()
                        {
                            let ix = ix + 1;
                            ui.label(name.clone());

                            let assign_str = assign
                                .extract_gamepad()
                                .map_or_else(|| "".to_string(), |k| k.to_string());

                            ui.selectable_value(&mut self.controller_button_ix, ix, assign_str)
                                .on_hover_text("Click and press the button you want to assign");

                            if self.controller_button_ix == ix {
                                if let Some(button) = gamepad_button_input.get_just_pressed().next()
                                {
                                    assign.insert_gamepad(ConvertInput(*button).into());
                                    changed = Some(ix);
                                }
                            }

                            ui.end_row();
                        }
                    }
                }

                if let Some(ix) = changed {
                    self.controller_button_ix = ix + 1;
                    config.set_key_config(core, key_config);
                }
            });
        });

        if ui.button("Reset to default").clicked() {
            let default_key_config = Emulator::default_key_config(core);
            self.controller_ix = 0;
            self.controller_button_ix = 0;
            config.set_key_config(core, default_key_config);
        }
    }

    fn tab_hotkey(
        &mut self,
        ui: &mut egui::Ui,
        config: &mut Config,
        key_code_input: &Input<KeyCode>,
        gamepad_button_input: &Input<GamepadButton>,
    ) {
        let grid = |ui: &mut egui::Ui| {
            ui.label("HotKey");
            ui.label("Assignment");
            ui.end_row();

            ui.separator();
            ui.separator();
            ui.end_row();

            let mut ix = 1;
            let mut hotkey_determined = false;

            if self.hotkey_select != 0 {
                let mut current_pushed = vec![];
                for r in key_code_input.get_pressed() {
                    current_pushed.push(SingleKey::KeyCode(ConvertInput(*r).into()));
                }
                for r in gamepad_button_input.get_pressed() {
                    current_pushed.push(SingleKey::GamepadButton(ConvertInput(*r).into()));
                }

                if self.constructing_hotkey.is_none() {
                    if !current_pushed.is_empty() {
                        self.constructing_hotkey = Some(current_pushed);
                    }
                } else {
                    let released = self
                        .constructing_hotkey
                        .as_ref()
                        .unwrap()
                        .iter()
                        .any(|k| !current_pushed.contains(k));

                    if released {
                        hotkey_determined = true;
                    } else {
                        for pushed in current_pushed {
                            if !self.constructing_hotkey.as_ref().unwrap().contains(&pushed) {
                                self.constructing_hotkey.as_mut().unwrap().push(pushed);
                            }
                        }
                    }
                }
            }

            for hotkey in all::<HotKey>() {
                ui.label(hotkey.to_string());

                ui.horizontal(|ui| {
                    let key_assign = config.hotkeys.key_assign_mut(&hotkey).unwrap();
                    for i in 0..key_assign.0.len() {
                        let key_str = if self.hotkey_select == ix {
                            if hotkey_determined {
                                self.hotkey_select = 0;
                                key_assign.0[i] =
                                    MultiKey(self.constructing_hotkey.clone().unwrap());
                                self.constructing_hotkey = None;
                            }

                            if let Some(mk) = &self.constructing_hotkey {
                                MultiKey(mk.clone()).to_string()
                            } else {
                                key_assign.0[i].to_string()
                            }
                        } else {
                            key_assign.0[i].to_string()
                        };

                        if ui
                            .selectable_value(&mut self.hotkey_select, ix, key_str)
                            .on_hover_text("Click to change\nRight click to remove")
                            .clicked_by(egui::PointerButton::Secondary)
                        {
                            key_assign.0.remove(i);
                            break;
                        }
                        ix += 1;
                    }

                    let key_str = if self.hotkey_select == ix {
                        if hotkey_determined {
                            self.hotkey_select = 0;
                            key_assign
                                .0
                                .push(MultiKey(self.constructing_hotkey.clone().unwrap()));
                            self.constructing_hotkey = None;
                        }

                        if let Some(mk) = &self.constructing_hotkey {
                            MultiKey(mk.clone()).to_string()
                        } else {
                            "...".to_string()
                        }
                    } else {
                        "...".to_string()
                    };

                    ui.selectable_value(&mut self.hotkey_select, ix, key_str)
                        .on_hover_text("Add new key assignment");
                    ix += 1;
                });

                ui.end_row();
            }
        };
        ui.group(|ui| {
            egui::Grid::new("key_config")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, grid);
        });
        if ui.button("Reset to default").clicked() {
            config.hotkeys = HotKeys::default();
        }
    }

    fn tab_system_key(
        &mut self,
        ui: &mut egui::Ui,
        config: &mut Config,
        key_code_input: &Input<KeyCode>,
        gamepad_button_input: &Input<GamepadButton>,
    ) {
        ui.horizontal(|ui| {
            let mut resp = ui.selectable_value(
                &mut self.system_key_tab,
                ControllerTab::Keyboard,
                "Keyboard",
            );
            resp |=
                ui.selectable_value(&mut self.system_key_tab, ControllerTab::Gamepad, "Gamepad");
            if resp.clicked() {
                self.system_key_ix = 0;
            }
        });

        ui.group(|ui| {
            let grid = egui::Grid::new("key_config")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true);

            grid.show(ui, |ui| {
                ui.label("Button");
                ui.label("Assignment");
                ui.end_row();

                ui.separator();
                ui.separator();
                ui.end_row();

                let mut changed: Option<usize> = None;

                match self.system_key_tab {
                    ControllerTab::Keyboard => {
                        for (ix, key) in all::<SystemKey>().enumerate() {
                            let ix = ix + 1;

                            ui.label(key.to_string());

                            let assign = config.system_keys.key_assign_mut(&key);

                            let assign_str = assign
                                .and_then(|r| r.extract_keycode())
                                .map_or_else(|| "".to_string(), |k| format!("{k:?}"));

                            ui.selectable_value(&mut self.system_key_ix, ix, assign_str)
                                .on_hover_text("Click and type the key you want to assign");

                            if self.system_key_ix == ix {
                                if let Some(kc) = key_code_input.get_just_pressed().next() {
                                    config
                                        .system_keys
                                        .insert_keycode(&key, ConvertInput(*kc).into());
                                    changed = Some(ix);
                                }
                            }

                            ui.end_row();
                        }
                    }

                    ControllerTab::Gamepad => {
                        for (ix, key) in all::<SystemKey>().enumerate() {
                            let ix = ix + 1;

                            ui.label(key.to_string());

                            let assign = config.system_keys.key_assign_mut(&key);

                            let assign_str = assign
                                .and_then(|r| r.extract_gamepad())
                                .map_or_else(|| "".to_string(), |k| k.to_string());

                            ui.selectable_value(&mut self.system_key_ix, ix, assign_str)
                                .on_hover_text("Click and type the key you want to assign");

                            if self.system_key_ix == ix {
                                if let Some(button) = gamepad_button_input.get_just_pressed().next()
                                {
                                    config
                                        .system_keys
                                        .insert_gamepad(&key, ConvertInput(*button).into());
                                    changed = Some(ix);
                                }
                            }

                            ui.end_row();
                        }
                    }
                }

                if let Some(ix) = changed {
                    self.system_key_ix = ix + 1;
                }
            });
        });

        if ui.button("Reset to default").clicked() {
            config.system_keys = SystemKeys::default();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn menu_system(
    mut config: ResMut<Config>,
    persistent_state: Res<PersistentState>,
    mut egui_ctx: ResMut<EguiContext>,
    mut app_state: ResMut<State<AppState>>,
    mut menu_state: ResMut<MenuState>,
    mut emulator: Option<ResMut<Emulator>>,
    menu_event: Res<Sender<MenuEvent>>,
    config_channel: Res<ConfigChannel>,
    mut window_control_event: EventWriter<WindowControlEvent>,
    mut menu_error: ResMut<Option<MenuError>>,
    key_code_input: Res<Input<KeyCode>>,
    gamepad_button_input: Res<Input<GamepadButton>>,
    fullscreen_state: Res<FullscreenState>,
) {
    if let Some(error) = menu_error.as_ref() {
        let mut open = true;
        let mut clicked = false;
        egui::Window::new(&error.title)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(egui_ctx.ctx_mut(), |ui| {
                let layout = egui::Layout::top_down(egui::Align::Center);

                ui.with_layout(layout, |ui| {
                    ui.label(&error.message);
                    if ui.button("OK").clicked() {
                        clicked = true;
                    }
                });
            });

        if !open || clicked {
            *menu_error.as_mut() = None;
        }
    }

    while let Ok(config_value) = config_channel.receiver.try_recv() {
        if let Some(emulator) = emulator.as_deref_mut() {
            if emulator.core.core_info().abbrev == config_value.abbrev {
                emulator.core.set_config(&config_value.value);
            }
        }

        config.set_core_config(&config_value.abbrev, config_value.value);

        let config = config.clone();
        spawn_local(async move { config.save().await.unwrap() });
    }

    let old_config = config.clone();

    egui::CentralPanel::default().show(egui_ctx.ctx_mut(), |ui| {
        let width = ui.available_width();

        let frame = egui::Frame::default();

        let left_panel = egui::SidePanel::left("left_panel").frame(frame);
        left_panel.show_inside(ui, |ui| {
            ui.set_width(width / 4.0);

            ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                menu_state.tab_selector(ui, emulator.is_some());
            });
        });

        egui::CentralPanel::default().show_inside(ui, |ui| match menu_state.tab.clone() {
            MenuTab::File => {
                tab_file(
                    ui,
                    emulator.as_ref().map(|r| r.as_ref()),
                    app_state.as_mut(),
                    persistent_state.as_ref(),
                    menu_event.as_ref(),
                    menu_error.as_mut(),
                );
            }
            MenuTab::State => {
                if let Some(emulator) = emulator.as_deref_mut() {
                    tab_state(ui, emulator, config.as_ref(), &menu_event);
                }
            }
            MenuTab::GameInfo => {
                if let Some(emulator) = emulator.as_deref() {
                    tab_game_info(ui, emulator);
                }
            }
            MenuTab::GeneralSetting => {
                ui.heading("General Settings");
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                    ui.group(|ui| {
                        tab_general_setting(ui, &mut config);
                    });
                });
            }
            MenuTab::Graphics => {
                ui.heading("Gaphics Settings");
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                    ui.group(|ui| {
                        ui.checkbox(&mut config.show_fps, "Display FPS");

                        let mut fullscreen = fullscreen_state.0;
                        if ui.checkbox(&mut fullscreen, "Full Screen").changed() {
                            window_control_event.send(WindowControlEvent::ToggleFullscreen);
                        }

                        #[cfg(not(target_arch = "wasm32"))]
                        ui.horizontal(|ui| {
                            ui.label("Window Scale:");

                            if ui
                                .add(egui::Slider::new(&mut config.scaling, 1..=8))
                                .changed()
                            {
                                window_control_event
                                    .send(WindowControlEvent::ChangeScale(config.scaling));
                            }
                        });
                    });
                });
            }
            MenuTab::CoreSetting(core) => {
                let core_info = Emulator::core_infos()
                    .into_iter()
                    .find(|c| c.abbrev == core)
                    .unwrap();

                ui.heading(format!("{} Settings", core_info.system_name));
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                    ui.group(|ui| {
                        let core_config = config.core_config(core_info.abbrev);
                        core_config_ui(ui, core_info.abbrev, core_config, &config_channel.sender);
                    });
                });
            }
            MenuTab::ControllerSetting(core) => {
                let core_info = Emulator::core_infos()
                    .into_iter()
                    .find(|c| c.abbrev == core)
                    .unwrap();

                ui.heading(format!("{} Controller Settings", core_info.system_name));
                menu_state.tab_controller(
                    ui,
                    config.as_mut(),
                    &core,
                    key_code_input.as_ref(),
                    gamepad_button_input.as_ref(),
                );
            }
            MenuTab::HotKey => {
                ui.heading("Hotkey Settings");
                menu_state.tab_hotkey(
                    ui,
                    config.as_mut(),
                    key_code_input.as_ref(),
                    gamepad_button_input.as_ref(),
                );
            }
            MenuTab::SystemKey => {
                ui.heading("System Key Settings");
                menu_state.tab_system_key(
                    ui,
                    config.as_mut(),
                    key_code_input.as_ref(),
                    gamepad_button_input.as_ref(),
                );
            }
        });
    });

    if &old_config != config.as_ref() {
        if let Some(emulator) = emulator.as_deref_mut() {
            emulator
                .core
                .set_config(&config.core_config(emulator.core.core_info().abbrev));
        }

        let config = config.clone();
        spawn_local(async move {
            config.save().await.unwrap();
        });
    }
}

fn file_dialog_filters() -> Vec<(String, Vec<String>)> {
    let mut ret = vec![("All files".into(), vec!["*".to_string()])];

    for info in Emulator::core_infos() {
        let name = format!("{} file", info.abbrev);
        let exts = info
            .file_extensions
            .iter()
            .chain(ARCHIVE_EXTENSIONS)
            .map(|e| e.to_string())
            .collect();
        ret.push((name, exts));
    }

    ret
}

async fn file_dialog(
    current_directory: Option<&Path>,
    filter: &[(&str, &[&str])],
    is_dir: bool,
) -> Option<(PathBuf, Vec<u8>)> {
    let fd = rfd::AsyncFileDialog::new();

    let fd = if let Some(path) = current_directory {
        fd.set_directory(path)
    } else {
        fd
    };

    let fd = filter
        .iter()
        .fold(fd, |fd, (name, extensions)| fd.add_filter(name, extensions));

    let file = if is_dir {
        cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                panic!("Wasm does not support directory selection")
            } else {
                fd.pick_folder().await
            }
        }
    } else {
        fd.pick_file().await
    };

    if let Some(file) = file {
        let data = file.read().await;

        cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                Some((PathBuf::from(file.file_name()), data))
            } else {
                Some((file.path().canonicalize().unwrap(), data))
            }
        }
    } else {
        None
    }
}

fn tab_file(
    ui: &mut egui::Ui,
    emulator: Option<&Emulator>,
    app_state: &mut State<AppState>,
    persistent_state: &PersistentState,
    menu_event: &Sender<MenuEvent>,
    #[allow(unused_variables)] menu_error: &mut Option<MenuError>,
) {
    let f = |ui: &mut egui::Ui| {
        if let Some(emulator) = &emulator {
            ui.label(format!("Running `{}`", emulator.game_name));
            if ui.button("Resume").clicked() {
                app_state.set(AppState::Running).unwrap();
            }
            ui.separator();
        }

        ui.label("Load ROM");
        if ui.button("Open File").clicked() {
            let menu_event = menu_event.clone();

            spawn_local(async move {
                let filter = file_dialog_filters();
                let filter_ref = filter
                    .iter()
                    .map(|(name, exts)| {
                        let exts = exts.iter().map(|r| r.as_str()).collect::<Vec<_>>();
                        (name.as_ref(), exts)
                    })
                    .collect::<Vec<_>>();
                let filter_ref = filter_ref
                    .iter()
                    .map(|(key, filter)| (*key, filter.as_slice()))
                    .collect::<Vec<_>>();

                if let Some((path, data)) = file_dialog(None, &filter_ref, false).await {
                    menu_event
                        .try_send(MenuEvent::OpenRomFile { path, data })
                        .unwrap();
                }
            });
        }

        ui.separator();
        ui.label("Recent Files");

        for recent in &persistent_state.recent {
            if ui
                .button(
                    recent
                        .path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .to_string(),
                )
                .clicked()
            {
                #[cfg(not(target_arch = "wasm32"))]
                let data = {
                    match std::fs::read(&recent.path) {
                        Ok(data) => data,
                        Err(err) => {
                            *menu_error = Some(MenuError {
                                title: "Failed to open ROM".into(),
                                message: err.to_string(),
                            });
                            continue;
                        }
                    }
                };

                #[cfg(target_arch = "wasm32")]
                let data = recent.data.clone();

                let path = recent.path.clone();

                menu_event
                    .try_send(MenuEvent::OpenRomFile { path, data })
                    .unwrap();
            }
        }
    };

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), f);
    });
}

fn tab_state(
    ui: &mut egui::Ui,
    emulator: &mut Emulator,
    config: &Config,
    menu_event: &Sender<MenuEvent>,
) {
    ui.heading("State Save / Load");

    let grid = |ui: &mut egui::Ui| {
        for i in 0..10 {
            ui.label(format!("{}", i));

            if ui.button("Save").clicked() {
                let menu_event = menu_event.clone();
                let fut = emulator.save_state_slot(i, config);
                spawn_local(async move {
                    fut.await.unwrap();
                    menu_event
                        .send(MenuEvent::StateSaved { slot: i })
                        .await
                        .unwrap();
                });
            }
            ui.add_enabled_ui(emulator.state_files[i].is_some(), |ui| {
                if ui.button("Load").clicked() {
                    let menu_event = menu_event.clone();
                    let fut = emulator.load_state_slot(i, config);
                    spawn_local(async move {
                        let data = fut.await;
                        menu_event
                            .send(MenuEvent::StateLoaded { slot: i, data })
                            .await
                            .unwrap();
                    });
                }
            });

            ui.label(emulator.state_files[i].as_ref().map_or_else(
                || "---".to_string(),
                |state_file| state_file.modified.format("%Y/%m/%d %H:%M:%S").to_string(),
            ));
            ui.end_row();
        }
    };

    ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
        ui.group(|ui| {
            ui.label("Slot");

            egui::Grid::new("state_save")
                .num_columns(4)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, grid);
        });
    });
}

fn tab_game_info(ui: &mut egui::Ui, emulator: &Emulator) {
    let info = emulator.core.game_info();

    ui.heading("Game Info");

    egui::Grid::new("key_config")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            for (key, value) in info {
                ui.label(key);
                ui.label(value);
                ui.end_row();
            }
        });
}

fn tab_general_setting(ui: &mut egui::Ui, config: &mut ResMut<Config>) {
    ui.horizontal(|ui| {
        ui.label("Frame skip on turbo:");

        ui.add(egui::Slider::new(&mut config.frame_skip_on_turbo, 1..=10));
    });

    ui.separator();

    #[cfg(not(target_arch = "wasm32"))]
    {
        ui.label("TODO: Save directory");

        // let mut save_dir = Some(config.save_dir.clone());
        // if file_field(ui, "Save file directory:", &mut save_dir, &[], false) {
        //     config.save_dir = save_dir.unwrap();
        // }
        // ui.separator();
    }

    ui.label("Rewinding:");

    ui.horizontal(|ui| {
        ui.label("Memory budget for rewinding:");
        let mut rate_in_kb = config.auto_state_save_rate / 1024;
        ui.add(
            egui::Slider::new(&mut rate_in_kb, 0..=8192)
                .logarithmic(true)
                .suffix("KiB/s"),
        );
        config.auto_state_save_rate = rate_in_kb * 1024;
    });

    ui.horizontal(|ui| {
        ui.label("Maximum memory amount for rewinding:");
        let mut amount_in_mb = config.auto_state_save_limit / (1024 * 1024);
        ui.add(
            egui::Slider::new(&mut amount_in_mb, 0..=8192)
                .logarithmic(true)
                .suffix("MiB"),
        );
        config.auto_state_save_limit = amount_in_mb * 1024 * 1024;
    });

    ui.horizontal(|ui| {
        ui.label("Minimum auto save span:");
        ui.add(
            egui::Slider::new(&mut config.minimum_auto_save_span, 1..=300)
                .logarithmic(true)
                .suffix("Frames"),
        );
    });

    // FIXME: reset auto save timing state when changed rewinding setting
}

pub struct FileFieldResult {
    file_sent: bool,
    cleard: bool,
}

#[derive(Clone, Debug)]
enum FieldIndex {
    Object(String),
    Array(usize),
}

pub fn file_field(
    ui: &mut egui::Ui,
    sender: &Sender<(PathBuf, Vec<u8>)>,
    label: &str,
    path: &mut Option<PathBuf>,
    file_filter: &[(&str, &[&str])],
    has_clear: bool,
) -> FileFieldResult {
    let mut file_sent = false;
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.label(label);
        if ui.button("Change").clicked() {
            file_sent = true;

            let cur_path = path.clone();
            let file_filter = file_filter
                .iter()
                .map(|(name, exts)| {
                    (
                        name.to_string(),
                        exts.iter().map(|ext| ext.to_string()).collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>();

            let sender = sender.clone();
            spawn_local(async move {
                let filter_keys = file_filter.iter().map(|(name, _)| name.as_str());
                let filter_vals = file_filter
                    .iter()
                    .map(|(_, val)| val.iter().map(|r| r.as_str()).collect::<Vec<_>>())
                    .collect::<Vec<_>>();
                let filter_ref = filter_keys
                    .into_iter()
                    .zip(filter_vals.iter())
                    .map(|(key, val)| (key, val.as_slice()))
                    .collect::<Vec<_>>();

                if let Some((path, data)) =
                    file_dialog(cur_path.as_deref(), &filter_ref, file_filter.is_empty()).await
                {
                    sender.send((path, data)).await.unwrap();
                }
            });
        }
        if has_clear && ui.button("Clear").clicked() {
            *path = None;
            changed = true;
        }
    });
    ui.indent("", |ui| {
        let s = path
            .as_ref()
            .map_or_else(|| "None".to_string(), |r| r.display().to_string());
        ui.add(egui::TextEdit::singleline(&mut s.as_ref()));
    });

    FileFieldResult {
        file_sent,
        cleard: changed,
    }
}

fn core_config_ui(ui: &mut egui::Ui, abbrev: &str, config: Value, sender: &Sender<ConfigValue>) {
    let mut schema = EMULATOR_CORES
        .iter()
        .find(|core| core.core_info().abbrev == abbrev)
        .unwrap()
        .config_schema();

    let (s, r) = unbounded_channel::<(Vec<FieldIndex>, Value)>();

    let is_empty = config == json!({});
    let mut visitor = ConfigVisitor::new(ui, &schema, config, s);

    if is_empty {
        visitor.ui().label("No config options");
    } else {
        visitor.visit_schema_object(&mut schema.schema);
    }

    let sender = sender.clone();
    let abbrev = abbrev.to_string();

    spawn_local(async move {
        while let Ok((path, value)) = r.recv().await {
            set_value_field(&mut visitor.new_val, &path, value);
        }

        if visitor.changed {
            sender
                .send(ConfigValue {
                    abbrev,
                    value: visitor.new_val,
                })
                .await
                .unwrap();
        }
    })
}

fn get_value_field<'a>(v: &'a mut Value, path: &'_ [FieldIndex]) -> &'a mut Value {
    let mut cur = v;
    for f in path {
        match f {
            FieldIndex::Object(field) => cur = &mut cur[field.as_str()],
            FieldIndex::Array(index) => cur = &mut cur[*index],
        }
    }
    cur
}

fn set_value_field(v: &mut Value, path: &[FieldIndex], value: Value) {
    *get_value_field(v, path) = value;
}

struct ConfigVisitor<'a> {
    ui: Option<&'a mut egui::Ui>,
    path: Vec<FieldIndex>,
    label: Option<String>,
    nullable: bool,
    cur_val: Value,
    new_val: Value,
    sender: Sender<(Vec<FieldIndex>, Value)>,
    changed: bool,
    defs: BTreeMap<String, Schema>,
}

impl<'a> ConfigVisitor<'a> {
    fn new(
        ui: &'a mut egui::Ui,
        schema: &RootSchema,
        value: Value,
        sender: Sender<(Vec<FieldIndex>, Value)>,
    ) -> Self {
        Self {
            ui: Some(ui),
            path: vec![],
            label: None,
            nullable: false,
            cur_val: value.clone(),
            new_val: value,
            sender,
            changed: false,
            defs: schema
                .definitions
                .iter()
                .map(|(name, schema)| (format!("#/definitions/{}", name), schema.clone()))
                .collect(),
        }
    }

    fn ui(&mut self) -> &mut egui::Ui {
        self.ui.as_deref_mut().unwrap()
    }
}

impl ConfigVisitor<'_> {
    fn resolve(&self, name: &str) -> Schema {
        self.defs.get(name).unwrap().clone()
    }
}

impl Visitor for ConfigVisitor<'_> {
    fn visit_schema_object(&mut self, schema: &mut SchemaObject) {
        if schema.is_ref() {
            let name = schema.reference.as_ref().unwrap().clone();
            let mut schema = self.resolve(&name);
            visit_schema(self, &mut schema);
            return;
        }

        let label = schema
            .metadata()
            .description
            .as_ref()
            .or(self.label.as_ref())
            .map_or_else(
                || {
                    self.path
                        .last()
                        .map(|index| match index {
                            FieldIndex::Object(field) => field.clone(),
                            FieldIndex::Array(index) => index.to_string(),
                        })
                        .unwrap_or_else(|| "".to_string())
                },
                |label| label.clone(),
            );

        if schema.has_type(InstanceType::Object) {
            // handle annotated
            if let Some(sub) = &schema.subschemas().all_of {
                if sub.len() != 1 {
                    let msg = format!("TODO: {:?}: Complex all_of", self.path);
                    self.ui().label(msg);
                    return;
                }

                let mut sub = sub[0].clone();

                let prev_label = self.label.take();
                self.label = schema.metadata().description.clone();

                visit_schema(self, &mut sub);

                self.label = prev_label;

                return;
            }

            // handle nullable
            if let Some(sub) = &schema.subschemas().any_of {
                let null_pos = if sub.len() == 2 {
                    sub.iter().position(is_null)
                } else {
                    None
                };

                if null_pos.is_none() {
                    let msg = format!("TODO: {:?}: Complex any_of", self.path);
                    self.ui().label(msg);
                    return;
                }

                let mut sub = sub[null_pos.unwrap() ^ 1].clone();

                let prev_nullable = self.nullable;
                self.nullable = true;

                let prev_label = self.label.take();
                self.label = schema.metadata().description.clone();

                visit_schema(self, &mut sub);

                self.label = prev_label;
                self.nullable = prev_nullable;

                return;
            }

            let obj = schema.object();

            if label.is_empty() {
                for (field_name, schema) in obj.properties.iter_mut() {
                    self.path.push(FieldIndex::Object(field_name.clone()));
                    visit_schema(self, schema);
                    self.path.pop();
                }
            } else {
                self.ui().label(&label);

                let mut parent_ui = self.ui.take();
                parent_ui.as_deref_mut().unwrap().indent("", |ui| {
                    // FIXME
                    let ui = unsafe { &mut *(ui as *mut egui::Ui) };
                    self.ui = Some(ui);

                    for (field_name, schema) in obj.properties.iter_mut() {
                        self.path.push(FieldIndex::Object(field_name.clone()));
                        visit_schema(self, schema);
                        self.path.pop();
                    }
                });
                self.ui = parent_ui;
            }
            return;
        }

        let nullable = schema.has_type(InstanceType::Null) || self.nullable;

        if schema.has_type(InstanceType::Array) {
            let array = schema.array();

            if array.min_items.is_some() && array.min_items != array.max_items {
                self.ui()
                    .label("TODO: Non-constant length arrays are not supported");
                return;
            }
            let len = array.min_items.unwrap();

            let items = if let Some(SingleOrVec::Single(items)) = &mut array.items {
                items
            } else {
                self.ui()
                    .label("TODO: Non-monomorphic arrays are not supported");
                return;
            };

            let mut parent_ui = self.ui.take();

            parent_ui.as_deref_mut().unwrap().horizontal(|ui| {
                ui.label(&label);

                // FIXME
                let ui = unsafe { &mut *(ui as *mut egui::Ui) };
                self.ui = Some(ui);
                for i in 0..len {
                    self.path.push(FieldIndex::Array(i as usize));
                    visit_schema(self, items);
                    self.path.pop();
                }
            });

            self.ui = parent_ui;
            return;
        }

        if schema.has_type(InstanceType::Boolean) {
            let mut value = get_value_field(&mut self.cur_val, &self.path)
                .as_bool()
                .unwrap();
            self.changed |= self.ui().checkbox(&mut value, &label).changed();
            set_value_field(&mut self.new_val, &self.path, value.into());
            return;
        }

        if schema.has_type(InstanceType::Number) {
            let msg = format!("TODO: {:?}: Number", self.path);
            self.ui().label(msg);
            return;
        }

        if schema.has_type(InstanceType::Integer) {
            let msg = format!("TODO: {:?}: Integer", self.path);
            self.ui().label(msg);
            return;
        }

        if schema.has_type(InstanceType::String) {
            let value = get_value_field(&mut self.cur_val, &self.path)
                .as_str()
                .unwrap_or("")
                .to_string();

            if let Some(enum_values) = &schema.enum_values {
                let alts = enum_values
                    .iter()
                    .map(|v| v.as_str().unwrap_or("").to_string())
                    .collect::<Vec<_>>();
                let mut selected = alts.iter().position(|v| v == &value).unwrap();

                self.changed |= egui::ComboBox::from_label(label)
                    .width(300.0)
                    .selected_text(&value)
                    .show_index(self.ui.as_mut().unwrap(), &mut selected, alts.len(), |i| {
                        alts[i].clone()
                    })
                    .changed();

                *get_value_field(&mut self.new_val, &self.path) =
                    Value::from(alts[selected].clone());
            } else if schema.format.as_deref() == Some("file") {
                #[cfg(not(target_arch = "wasm32"))]
                let mut path = {
                    serde_json::from_value::<Option<File>>(
                        get_value_field(&mut self.cur_val, &self.path).clone(),
                    )
                    .unwrap()
                    .map(|f| f.path().to_owned())
                };

                #[cfg(target_arch = "wasm32")]
                let mut path = {
                    self.path.push(FieldIndex::Object("path".to_string()));
                    let path = serde_json::from_value::<Option<PathBuf>>(
                        get_value_field(&mut self.cur_val, &self.path).clone(),
                    )
                    .unwrap();
                    self.path.pop();
                    path
                };

                // TODO: way to specify filters

                let (s, r) = unbounded_channel::<(PathBuf, Vec<u8>)>();

                let res = file_field(
                    self.ui(),
                    &s,
                    &label,
                    &mut path,
                    &[("All files", &["*"])],
                    nullable,
                );

                if res.cleard {
                    self.changed = true;
                    set_value_field(&mut self.new_val, &self.path, Value::Null);
                }

                if res.file_sent {
                    let json_path = self.path.clone();
                    let sender = self.sender.clone();
                    spawn_local(async move {
                        #[allow(unused_variables)]
                        if let Ok((path, data)) = r.recv().await {
                            let file = File::new(path, data);
                            sender
                                .send((json_path, serde_json::to_value(file).unwrap()))
                                .await
                                .unwrap();
                        }
                    });

                    self.changed = true;
                }
            } else if schema.format.as_deref() == Some("color") {
                let value = get_value_field(&mut self.cur_val, &self.path);

                let color = serde_json::from_value::<meru_interface::Color>(value.clone()).unwrap();
                let mut color = [color.r, color.g, color.b];
                if self
                    .ui
                    .as_mut()
                    .unwrap()
                    .color_edit_button_srgb(&mut color)
                    .changed()
                {
                    set_value_field(
                        &mut self.new_val,
                        &self.path,
                        serde_json::to_value(meru_interface::Color::new(
                            color[0], color[1], color[2],
                        ))
                        .unwrap(),
                    );
                    self.changed = true;
                }
            } else {
                let msg = format!("TODO: {:?}: String ({:?})", self.path, schema.format);
                self.ui().label(msg);
            }
        }
    }
}

fn is_null(s: &Schema) -> bool {
    if let Some(SingleOrVec::Single(r)) = s.clone().into_object().instance_type {
        matches!(r.as_ref(), InstanceType::Null)
    } else {
        false
    }
}
