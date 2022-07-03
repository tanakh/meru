use bevy::prelude::*;
use bevy_egui::{egui, EguiContext};
use enum_iterator::all;
use meru_interface::key_assign::*;
use std::path::PathBuf;

use crate::{
    app::{AppState, FullscreenState, ShowMessage, WindowControlEvent},
    config::{Config, PersistentState},
    core::{Emulator, ARCHIVE_EXTENSIONS},
    file::state_date,
    hotkey::{HotKey, HotKeys},
};

pub const MENU_WIDTH: usize = 1280;
pub const MENU_HEIGHT: usize = 720;

pub struct MenuPlugin;

pub enum MenuEvent {
    OpenRomFile(PathBuf),
}

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

fn setup_menu_system(
    mut commands: Commands,
    mut windows: ResMut<Windows>,
    fullscreen_state: Res<FullscreenState>,
) {
    if !fullscreen_state.0 {
        let window = windows.get_primary_mut().unwrap();
        window.set_resolution(MENU_WIDTH as f32, MENU_HEIGHT as f32);
    }

    commands.insert_resource(MenuState::default());
}

fn menu_exit(config: Res<Config>) {
    config.save().unwrap();
}

fn menu_event_system(
    mut commands: Commands,
    mut event: EventReader<MenuEvent>,
    mut app_state: ResMut<State<AppState>>,
    mut persistent_state: ResMut<PersistentState>,
    config: Res<Config>,
) {
    for event in event.iter() {
        match event {
            MenuEvent::OpenRomFile(path) => {
                info!("Opening file: {:?}", path);
                match Emulator::try_new(path, &config) {
                    Ok(emulator) => {
                        commands.insert_resource(emulator);
                        persistent_state.add_recent(&path);
                        app_state.set(AppState::Running).unwrap();
                    }
                    Err(err) => {
                        error!("{err}");
                    }
                }
            }
        }
    }
}

#[derive(PartialEq, Eq)]
enum MenuTab {
    File,
    State,
    GameInfo,
    GeneralSetting,
    CoreSetting(String),
    ControllerSetting(String),
    Graphics,
    HotKey,
}

#[derive(PartialEq, Eq)]
enum ControllerTab {
    Keyboard,
    Gamepad,
}

struct MenuState {
    tab: MenuTab,
    controller_tab: ControllerTab,
    controller_button_ix: usize,
    hotkey_select: usize,
    constructing_hotkey: Option<Vec<SingleKey>>,
}

impl Default for MenuState {
    fn default() -> Self {
        MenuState {
            tab: MenuTab::File,
            controller_tab: ControllerTab::Keyboard,
            controller_button_ix: 0,
            hotkey_select: 0,
            constructing_hotkey: None,
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
    mut menu_event: EventWriter<MenuEvent>,
    mut message_event: EventWriter<ShowMessage>,
    mut window_control_event: EventWriter<WindowControlEvent>,
    key_code_input: Res<Input<KeyCode>>,
    gamepad_button_input: Res<Input<GamepadButton>>,
    fullscreen_state: Res<FullscreenState>,
) {
    let MenuState {
        tab,
        controller_tab,
        controller_button_ix,
        hotkey_select,
        constructing_hotkey,
    } = menu_state.as_mut();

    let old_config = config.clone();

    egui::CentralPanel::default().show(egui_ctx.ctx_mut(), |ui| {
        let width = ui.available_width();

        let frame = egui::Frame::default();

        egui::SidePanel::left("left_panel")
            .frame(frame)
            .show_inside(ui, |ui| {
                ui.set_width(width / 4.0);

                ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                    ui.heading("Main Menu");
                    ui.separator();

                    ui.selectable_value(tab, MenuTab::File, "📁 File");

                    ui.add_enabled_ui(emulator.is_some(), |ui| {
                        ui.selectable_value(tab, MenuTab::State, "💾 State Save/Load");
                    });

                    ui.add_enabled_ui(emulator.is_some(), |ui| {
                        ui.selectable_value(tab, MenuTab::GameInfo, "ℹ Game Info");
                    });

                    ui.selectable_value(tab, MenuTab::GeneralSetting, "🔧 General Setting");
                    ui.selectable_value(tab, MenuTab::Graphics, "🖼 Graphics");

                    ui.collapsing("⚙ Core Setting", |ui| {
                        for core_info in Emulator::core_infos() {
                            ui.selectable_value(
                                tab,
                                MenuTab::CoreSetting(core_info.abbrev.into()),
                                core_info.system_name,
                            );
                        }
                    });
                    ui.collapsing("🎮 Controller Setting", |ui| {
                        for core_info in Emulator::core_infos() {
                            ui.selectable_value(
                                tab,
                                MenuTab::ControllerSetting(core_info.abbrev.into()),
                                core_info.system_name,
                            );
                        }
                    });

                    ui.selectable_value(tab, MenuTab::HotKey, "⌨ HotKey");
                });
            });

        egui::CentralPanel::default().show_inside(ui, |ui| match tab {
            MenuTab::File => {
                tab_file(
                    ui,
                    emulator.as_ref().map(|r| r.as_ref()),
                    app_state.as_mut(),
                    persistent_state.as_ref(),
                    &mut menu_event,
                );
            }
            MenuTab::State => {
                if let Some(emulator) = emulator.as_deref_mut() {
                    tab_state(
                        ui,
                        emulator,
                        config.as_ref(),
                        app_state.as_mut(),
                        &mut message_event,
                    );
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
                        Emulator::config_ui(ui, core_info.abbrev, config.as_mut());
                    });
                });
            }
            MenuTab::ControllerSetting(core) => {
                let core_info = Emulator::core_infos()
                    .into_iter()
                    .find(|c| c.abbrev == core)
                    .unwrap();

                ui.heading(format!("{} Controller Settings", core_info.system_name));
                controller_ui(
                    ui,
                    core,
                    config.as_mut(),
                    controller_tab,
                    controller_button_ix,
                    key_code_input,
                    gamepad_button_input,
                );
            }
            MenuTab::HotKey => {
                tab_hotkey(
                    ui,
                    hotkey_select,
                    key_code_input,
                    gamepad_button_input,
                    constructing_hotkey,
                    config.as_mut(),
                );
            }
        });
    });

    if &old_config != config.as_ref() {
        if let Some(emulator) = emulator.as_deref_mut() {
            emulator.core.set_config(config.as_ref());
        }
        config.save().unwrap();
    }
}

fn tab_file(
    ui: &mut egui::Ui,
    emulator: Option<&Emulator>,
    app_state: &mut State<AppState>,
    persistent_state: &PersistentState,
    menu_event: &mut EventWriter<MenuEvent>,
) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
            if let Some(emulator) = &emulator {
                ui.label(format!("Running `{}`", emulator.game_name));
                if ui.button("Resume").clicked() {
                    app_state.set(AppState::Running).unwrap();
                }
                ui.separator();
            }

            ui.label("Load ROM");
            if ui.button("Open File").clicked() {
                let mut fd = rfd::FileDialog::new();

                for (name, exts) in file_dialog_filters() {
                    let exts = exts.iter().map(|r| r.as_str()).collect::<Vec<_>>();
                    fd = fd.add_filter(&name, &exts);
                }

                let file = fd.pick_file();

                if let Some(file) = file {
                    menu_event.send(MenuEvent::OpenRomFile(file));
                }
            }

            ui.separator();
            ui.label("Recent Files");

            for recent in &persistent_state.recent {
                if ui
                    .button(recent.file_name().unwrap().to_string_lossy().to_string())
                    .clicked()
                {
                    menu_event.send(MenuEvent::OpenRomFile(recent.clone()));
                }
            }
        });
    });
}

fn tab_state(
    ui: &mut egui::Ui,
    emulator: &mut Emulator,
    config: &Config,
    app_state: &mut State<AppState>,
    message_event: &mut EventWriter<ShowMessage>,
) {
    ui.heading("State Save / Load");

    ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
        ui.group(|ui| {
            ui.label("Slot");

            let grid = |ui: &mut egui::Ui| {
                for i in 0..10 {
                    ui.label(format!("{}", i));

                    let date = state_date(
                        emulator.core.core_info().abbrev,
                        &emulator.game_name,
                        i,
                        &config.save_dir,
                    )
                    .unwrap();

                    if ui.button("Save").clicked() {
                        emulator.save_state_slot(i, config).unwrap();
                        message_event.send(ShowMessage(format!("State saved: #{}", i)));
                    }
                    ui.add_enabled_ui(date.is_some(), |ui| {
                        if ui.button("Load").clicked() {
                            match emulator.load_state_slot(i, config) {
                                Ok(_) => {
                                    message_event
                                        .send(ShowMessage(format!("State loaded: #{}", i)));
                                }
                                Err(e) => {
                                    message_event
                                        .send(ShowMessage("Failed to load state".to_string()));
                                    error!("Failed to load state: {}", e);
                                }
                            }
                            app_state.set(AppState::Running).unwrap();
                        }
                    });

                    ui.label(date.map_or_else(
                        || "---".to_string(),
                        |date| date.format("%Y/%m/%d %H:%M:%S").to_string(),
                    ));
                    ui.end_row();
                }
            };

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

    let mut save_dir = Some(config.save_dir.clone());
    if file_field(ui, "Save file directory:", &mut save_dir, &[], false) {
        config.save_dir = save_dir.unwrap();
    }

    ui.separator();

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

fn tab_hotkey(
    ui: &mut egui::Ui,
    hotkey_select: &mut usize,
    key_code_input: Res<Input<KeyCode>>,
    gamepad_button_input: Res<Input<GamepadButton>>,
    constructing_hotkey: &mut Option<Vec<SingleKey>>,
    config: &mut Config,
) {
    ui.heading("HotKey Settings");
    let grid = |ui: &mut egui::Ui| {
        ui.label("HotKey");
        ui.label("Assignment");
        ui.end_row();

        ui.separator();
        ui.separator();
        ui.end_row();

        let mut ix = 1;
        let mut hotkey_determined = false;

        if *hotkey_select != 0 {
            let mut current_pushed = vec![];
            for r in key_code_input.get_pressed() {
                current_pushed.push(SingleKey::KeyCode(*r));
            }
            for r in gamepad_button_input.get_pressed() {
                current_pushed.push(SingleKey::GamepadButton(*r));
            }

            if constructing_hotkey.is_none() {
                if !current_pushed.is_empty() {
                    *constructing_hotkey = Some(current_pushed);
                }
            } else {
                let released = constructing_hotkey
                    .as_ref()
                    .unwrap()
                    .iter()
                    .any(|k| !current_pushed.contains(k));

                if released {
                    hotkey_determined = true;
                } else {
                    for pushed in current_pushed {
                        if !constructing_hotkey.as_ref().unwrap().contains(&pushed) {
                            constructing_hotkey.as_mut().unwrap().push(pushed);
                        }
                    }
                }
            }
        }

        for hotkey in all::<HotKey>() {
            ui.label(hotkey.to_string());

            ui.horizontal(|ui| {
                let key_assign = config.hotkeys.key_assign_mut(hotkey).unwrap();
                for i in 0..key_assign.0.len() {
                    let key_str = if *hotkey_select == ix {
                        if hotkey_determined {
                            *hotkey_select = 0;
                            key_assign.0[i] = MultiKey(constructing_hotkey.clone().unwrap());
                            *constructing_hotkey = None;
                        }

                        if let Some(mk) = constructing_hotkey {
                            MultiKey(mk.clone()).to_string()
                        } else {
                            key_assign.0[i].to_string()
                        }
                    } else {
                        key_assign.0[i].to_string()
                    };

                    if ui
                        .selectable_value(hotkey_select, ix, key_str)
                        .on_hover_text("Click to change\nRight click to remove")
                        .clicked_by(egui::PointerButton::Secondary)
                    {
                        key_assign.0.remove(i);
                        break;
                    }
                    ix += 1;
                }

                let key_str = if *hotkey_select == ix {
                    if hotkey_determined {
                        *hotkey_select = 0;
                        key_assign
                            .0
                            .push(MultiKey(constructing_hotkey.clone().unwrap()));
                        *constructing_hotkey = None;
                    }

                    if let Some(mk) = constructing_hotkey {
                        MultiKey(mk.clone()).to_string()
                    } else {
                        "...".to_string()
                    }
                } else {
                    "...".to_string()
                };

                ui.selectable_value(hotkey_select, ix, key_str)
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

fn controller_ui(
    ui: &mut egui::Ui,
    core: &str,
    config: &mut Config,
    controller_tab: &mut ControllerTab,
    controller_button_ix: &mut usize,
    key_code_input: Res<Input<KeyCode>>,
    gamepad_button_input: Res<Input<GamepadButton>>,
) {
    let mut key_config = config.key_config(core).clone();

    ui.horizontal(|ui| {
        let mut resp = ui.selectable_value(controller_tab, ControllerTab::Keyboard, "Keyboard");
        resp |= ui.selectable_value(controller_tab, ControllerTab::Gamepad, "Gamepad");
        if resp.clicked() {
            *controller_button_ix = 0;
        }
    });

    ui.group(|ui| {
        egui::Grid::new("key_config")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Button");
                ui.label("Assignment");
                ui.end_row();

                ui.separator();
                ui.separator();
                ui.end_row();

                let mut changed: Option<usize> = None;

                match *controller_tab {
                    ControllerTab::Keyboard => {
                        for (ix, (name, assign)) in key_config.keys.iter_mut().enumerate() {
                            let ix = ix + 1;
                            ui.label(name.clone());
                            let assign_str = assign
                                .extract_keycode()
                                .map_or_else(|| "".to_string(), |k| format!("{k:?}"));

                            ui.selectable_value(controller_button_ix, ix, assign_str)
                                .on_hover_text("Click and type the key you want to assign");

                            if *controller_button_ix == ix {
                                if let Some(kc) = key_code_input.get_just_pressed().next() {
                                    assign.insert_keycode(*kc);
                                    changed = Some(ix);
                                }
                            }

                            ui.end_row();
                        }
                    }

                    ControllerTab::Gamepad => {
                        for (ix, (name, assign)) in key_config.keys.iter_mut().enumerate() {
                            let ix = ix + 1;
                            ui.label(name.clone());

                            let assign_str = assign
                                .extract_gamepad()
                                .map_or_else(|| "".to_string(), |k| ToStringKey(k).to_string());

                            ui.selectable_value(controller_button_ix, ix, assign_str)
                                .on_hover_text("Click and press the button you want to assign");

                            if *controller_button_ix == ix {
                                if let Some(button) = gamepad_button_input.get_just_pressed().next()
                                {
                                    assign.insert_gamepad(*button);
                                    changed = Some(ix);
                                }
                            }

                            ui.end_row();
                        }
                    }
                }

                if let Some(ix) = changed {
                    *controller_button_ix = ix + 1;
                    config.set_key_config(core, key_config);
                }
            });
    });

    if ui.button("Reset to default").clicked() {
        let default_key_config = Emulator::default_key_config(core);
        let mut cur_key_config = config.key_config(core).clone();

        match *controller_tab {
            ControllerTab::Keyboard => {
                for (key, assign) in cur_key_config.keys.iter_mut() {
                    assign.insert_keycode(
                        default_key_config
                            .keys
                            .iter()
                            .find_map(|r| (&r.0 == key).then(|| r.1.extract_keycode()))
                            .unwrap()
                            .unwrap(),
                    );
                }
            }
            ControllerTab::Gamepad => {
                for (key, assign) in cur_key_config.keys.iter_mut() {
                    assign.insert_gamepad(
                        default_key_config
                            .keys
                            .iter()
                            .find_map(|r| (&r.0 == key).then(|| r.1.extract_gamepad()))
                            .unwrap()
                            .unwrap(),
                    );
                }
            }
        }
        *controller_button_ix = 0;
        config.set_key_config(core, cur_key_config);
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

pub fn file_field(
    ui: &mut egui::Ui,
    label: &str,
    path: &mut Option<PathBuf>,
    file_filter: &[(&str, &[&str])],
    has_clear: bool,
) -> bool {
    let mut ret = false;
    ui.horizontal(|ui| {
        ui.label(label);
        if ui.button("Change").clicked() {
            let fd = rfd::FileDialog::new();
            let fd = if let Some(path) = path {
                fd.set_directory(path)
            } else {
                fd
            };
            let fd = file_filter
                .iter()
                .fold(fd, |fd, (name, extensions)| fd.add_filter(name, extensions));
            let dir = if file_filter.is_empty() {
                fd.pick_folder()
            } else {
                fd.pick_file()
            };

            if let Some(new_path) = dir {
                *path = Some(new_path);
                ret = true;
            }
        }
        if has_clear && ui.button("Clear").clicked() {
            *path = None;
            ret = true;
        }
    });
    ui.indent("", |ui| {
        let s = path
            .as_ref()
            .map_or_else(|| "None".to_string(), |r| r.display().to_string());
        ui.add(egui::TextEdit::singleline(&mut s.as_ref()));
    });
    ret
}
