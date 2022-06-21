use crate::{
    app::{AppState, FullscreenState, ShowMessage, WindowControlEvent},
    config::{Config, PersistentState},
    core::Emulator,
    hotkey::{HotKey, HotKeys},
    // input::KeyConfig,
    key_assign::{MultiKey, SingleKey, ToStringKey},
};
use bevy::{app::AppExit, prelude::*};
use bevy_egui::{
    egui::{self, SelectableLabel},
    EguiContext,
};
use enum_iterator::all;
use std::{path::PathBuf, time::SystemTime};

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
                match Emulator::try_new(&path, &config) {
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
    GeneralSetting,
    Graphics,
    Controller,
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
    last_palette_changed: Option<SystemTime>,
    hotkey_select: usize,
    constructing_hotkey: Option<Vec<SingleKey>>,
}

impl Default for MenuState {
    fn default() -> Self {
        MenuState {
            tab: MenuTab::File,
            controller_tab: ControllerTab::Keyboard,
            controller_button_ix: 0,
            last_palette_changed: None,
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
    mut exit: EventWriter<AppExit>,
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
        last_palette_changed,
        hotkey_select,
        constructing_hotkey,
    } = menu_state.as_mut();

    if let Some(changed) = last_palette_changed {
        if changed.elapsed().unwrap().as_secs_f64() > 5.0 {
            config.save().unwrap();
            *last_palette_changed = None;
        }
    }

    egui::CentralPanel::default().show(egui_ctx.ctx_mut(), |ui| {
        let width = ui.available_width();
        egui::SidePanel::left("left_panel").show_inside(ui, |ui| {
            ui.set_width(width / 4.0);
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                ui.selectable_value(tab, MenuTab::File, "📁 File");

                // ui.add_enabled_ui(gb_state.is_some(), |ui| {
                //     ui.selectable_value(tab, MenuTab::State, "💾 State Save/Load");
                // });

                ui.selectable_value(tab, MenuTab::GeneralSetting, "🔧 General Setting");
                ui.selectable_value(tab, MenuTab::Graphics, "🖼 Graphics");
                ui.selectable_value(tab, MenuTab::Controller, "🎮 Controller");
                ui.selectable_value(tab, MenuTab::HotKey, "⌨ HotKey");
                if ui.selectable_label(false, "↩ Quit").clicked() {
                    exit.send(AppExit);
                }
            });
        });

        egui::CentralPanel::default().show_inside(ui, |ui| match *tab {
            MenuTab::File => {
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
                            let file = rfd::FileDialog::new()
                                .add_filter("GameBoy ROM file", &["gb", "gbc", "zip"])
                                .pick_file();
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
            MenuTab::State => {
                ui.heading("State Save / Load");

                // if let Some(gb_state) = gb_state.as_mut() {
                //     ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                //         ui.group(|ui| {
                //             ui.label("Slot");

                //             let grid = |ui:&mut egui::Ui| {
                //                 for i in 0..10 {
                //                     ui.label(format!("{}", i));

                //                     let date = state_data_date(&gb_state.rom_file, i, config.state_dir()).unwrap();

                //                     if ui.button( "Save").clicked() {
                //                         gb_state.save_state(i, config.as_ref()).unwrap();
                //                         message_event.send(ShowMessage(format!("State saved: #{}", i)));
                //                         app_state.set(AppState::Running).unwrap();
                //                     }
                //                     ui.add_enabled_ui(date.is_some(), |ui| {
                //                         if ui.button( "Load").clicked() {
                //                             match gb_state.load_state(i, config.as_ref()) {
                //                                 Ok(_) => {
                //                                     message_event.send(ShowMessage(format!("State loaded: #{}", i)));
                //                                 },
                //                                 Err(e) => {
                //                                     message_event.send(ShowMessage("Failed to load state".to_string()));
                //                                     error!("Failed to load state: {}", e);
                //                                 },
                //                             }
                //                             app_state.set(AppState::Running).unwrap();
                //                         }
                //                     });

                //                     ui.label(date.map_or_else(|| "---".to_string(), |date| date.format("%Y/%m/%d %H:%M:%S").to_string()));
                //                     ui.end_row();
                //                 }
                //             };

                //             egui::Grid::new("key_config")
                //             .num_columns(4)
                //             .spacing([40.0, 4.0])
                //             .striped(true)
                //             .show(ui, grid);

                //         });
                //     });
                // }
            }
            MenuTab::GeneralSetting => {
                ui.heading("General Settings");
                // ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                //     ui.group(|ui| {
                //         ui.horizontal(|ui| {
                //             ui.label("Model:");
                //             let mut val = config.model();
                //             ui.radio_value(&mut val, Model::Auto, "Auto");
                //             ui.radio_value(&mut val, Model::Dmg, "GameBoy");
                //             ui.radio_value(&mut val, Model::Cgb, "GameBoy Color");
                //             if config.model() != val {
                //                 config.set_model(val);
                //             }
                //         });

                //         ui.horizontal(|ui| {
                //             ui.label("Frame skip on turbo:");

                //             let mut frame_skip_on_turbo = config.frame_skip_on_turbo();
                //             if ui.add(egui::Slider::new(&mut frame_skip_on_turbo, 1..=10)).changed() {
                //                 config.set_frame_skip_on_turbo(frame_skip_on_turbo);
                //             }
                //         });

                //         ui.separator();

                //         let mut save_dir = Some(config.save_dir().to_owned());
                //         if file_field(ui, "Save file directory:", &mut save_dir, &[], false) {
                //             config.set_save_dir(save_dir.unwrap());
                //         }
                //         let mut state_dir = Some(config.state_dir().to_owned());
                //         if file_field(ui, "State save directory:", &mut state_dir, &[], false) {
                //             config.set_state_dir(state_dir.unwrap());
                //         }

                //         ui.separator();

                //         ui.label("Boot ROM:");

                //         let mut boot_rom = config.boot_rom().clone();

                //         ui.horizontal(|ui| {
                //             ui.radio_value(&mut boot_rom,BootRom::None, "Do not use");
                //             ui.radio_value(&mut boot_rom, BootRom::Internal, "Use internal ROM");
                //             ui.radio_value(&mut boot_rom, BootRom::Custom, "Use specified ROM");
                //         });

                //         if &boot_rom != config.boot_rom() {
                //             config.set_boot_rom(boot_rom.clone());
                //         }

                //         ui.add_enabled_ui(boot_rom == BootRom::Custom, |ui| {
                //             let mut path = config.custom_boot_roms().dmg.clone();
                //             if file_field(ui, "DMG boot ROM:", &mut path, &[("Boot ROM file", &["bin"])], true) {
                //                 config.custom_boot_roms_mut().dmg = path;
                //                 config.save().unwrap();
                //             }
                //             let mut path = config.custom_boot_roms().cgb.clone();
                //             if file_field(ui, "CGB boot ROM:", &mut path, &[("Boot ROM file", &["bin"])], true) {
                //                 config.custom_boot_roms_mut().cgb = path;
                //                 config.save().unwrap();
                //             }
                //         });
                //     });
                // });
            }
            MenuTab::Graphics => {
                ui.heading("Gaphics Settings");
                // ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                //     ui.group(|ui| {
                //         let mut color_correction = config.color_correction();
                //         if ui
                //             .checkbox(&mut color_correction, "Color Correction")
                //             .changed()
                //         {
                //             config.set_color_correction(color_correction);
                //         }

                //         let mut show_fps = config.show_fps();
                //         if ui.checkbox(&mut show_fps, "Display FPS").changed() {
                //             config.set_show_fps(show_fps);
                //         }

                //         let mut fullscreen = fullscreen_state.0;
                //         if ui.checkbox(&mut fullscreen, "FullScreen").changed() {
                //             window_control_event.send(WindowControlEvent::ToggleFullscreen);
                //         }

                //         ui.horizontal(|ui| {
                //             ui.label("Window Scale:");

                //             let mut scale = config.scaling();
                //             if ui.add(egui::Slider::new(&mut scale, 1..=8)).changed() {
                //                 window_control_event
                //                     .send(WindowControlEvent::ChangeScale(scale));
                //             }
                //         });

                //         ui.label("GameBoy Palette:");

                //         let mut pal: PaletteSelect = config.palette().clone();

                //         ui.horizontal(|ui| {
                //             egui::ComboBox::from_label("")
                //                 .width(250.0)
                //                 .selected_text(match pal {
                //                     PaletteSelect::Dmg => "GameBoy",
                //                     PaletteSelect::Pocket => "GameBoy Pocket",
                //                     PaletteSelect::Light => "GameBoy Light",
                //                     PaletteSelect::Grayscale => "Grayscale",
                //                     PaletteSelect::Custom(_) => "Custom",
                //                 })
                //                 .show_ui(ui, |ui| {
                //                     ui.selectable_value(&mut pal, PaletteSelect::Dmg, "GameBoy");
                //                     ui.selectable_value(&mut pal, PaletteSelect::Pocket, "GameBoy Pocket");
                //                     ui.selectable_value(&mut pal, PaletteSelect::Light, "GameBoy Light");
                //                     ui.selectable_value(&mut pal, PaletteSelect::Grayscale, "Grayscale");
                //                     if ui.add(SelectableLabel::new(matches!(pal, PaletteSelect::Custom(_)), "Custom")).clicked() {
                //                         pal = PaletteSelect::Custom(*pal.get_palette());
                //                     }
                //                 }
                //             );

                //             let cols = *pal.get_palette();

                //             for i in (0..4).rev() {
                //                 let mut col = [cols[i].r, cols[i].g, cols[i].b];
                //                 ui.color_edit_button_srgb(&mut col).changed();

                //                 if let PaletteSelect::Custom(r) = &mut pal {
                //                     r[i] = tgbr_core::Color::new(col[0], col[1], col[2]);
                //                 }
                //             }
                //         });

                //         if &pal != config.palette() {
                //             if let Some(gb_state) = gb_state.as_mut() {
                //                 gb_state.gb.set_dmg_palette(pal.get_palette());
                //             }
                //             config.set_palette(pal);
                //             *last_palette_changed = Some(SystemTime::now());
                //         }
                //     });
                // });
            }
            MenuTab::Controller => {
                ui.heading("Controller Settings");

                // ui.horizontal(|ui| {
                //     let mut resp = ui.selectable_value(controller_tab, ControllerTab::Keyboard, "Keyboard");
                //     resp |= ui.selectable_value(controller_tab, ControllerTab::Gamepad, "Gamepad");
                //     if resp.clicked() {
                //         *controller_button_ix = 0;
                //     }
                // });

                // ui.group(|ui| {
                //     egui::Grid::new("key_config")
                //         .num_columns(2)
                //         .spacing([40.0, 4.0])
                //         .striped(true)
                //         .show(ui, |ui| {
                //             ui.label("Button");
                //             ui.label("Assignment");
                //             ui.end_row();

                //             ui.separator();
                //             ui.separator();
                //             ui.end_row();

                //             let mut changed: Option<usize> = None;

                //             match *controller_tab {
                //                 ControllerTab::Keyboard => {
                //                     macro_rules! button {
                //                         {$ix:literal, $button:ident, $label:literal} => {
                //                             ui.label($label);
                //                             let assign = config.key_config().$button.extract_keycode()
                //                                 .map_or_else(|| "".to_string(), |k| format!("{k:?}"));

                //                             ui.selectable_value(controller_button_ix, $ix, assign)
                //                                 .on_hover_text("Click and type the key you want to assign");

                //                             if *controller_button_ix == $ix {
                //                                 if let Some(kc) = key_code_input.get_just_pressed().nth(0) {
                //                                     config.key_config_mut().$button.insert_keycode(*kc);
                //                                     config.save().unwrap();
                //                                     changed = Some($ix);
                //                                 }
                //                             }

                //                             ui.end_row();
                //                         }
                //                     }

                //                     button!(1, up, "⏶");
                //                     button!(2, down, "⏷");
                //                     button!(3, left, "⏴");
                //                     button!(4, right, "⏵");
                //                     button!(5, a, "A");
                //                     button!(6, b, "B");
                //                     button!(7, start, "start");
                //                     button!(8, select, "select");
                //                 }

                //                 ControllerTab::Gamepad => {
                //                     macro_rules! button {
                //                         {$ix:literal, $button:ident, $label:literal} => {
                //                             ui.label($label);
                //                             let assign = config.key_config().$button.extract_gamepad()
                //                                 .map_or_else(|| "".to_string(), |k| ToStringKey(k).to_string());

                //                             ui.selectable_value(controller_button_ix, $ix, assign)
                //                                 .on_hover_text("Click and press the button you want to assign");

                //                             if *controller_button_ix == $ix {
                //                                 if let Some(button) = gamepad_button_input.get_just_pressed().nth(0) {
                //                                     config.key_config_mut().$button.insert_gamepad(*button);
                //                                     config.save().unwrap();
                //                                     changed = Some($ix);
                //                                 }
                //                             }

                //                             ui.end_row();
                //                         }
                //                     }

                //                     button!(1, up, "⏶");
                //                     button!(2, down, "⏷");
                //                     button!(3, left, "⏴");
                //                     button!(4, right, "⏵");
                //                     button!(5, a, "A");
                //                     button!(6, b, "B");
                //                     button!(7, start, "start");
                //                     button!(8, select, "select");
                //                 }
                //             }

                //             if let Some(ix) = changed {
                //                 *controller_button_ix = ix + 1;
                //             }

                //         });
                // });

                // if ui.button("Reset to default").clicked() {
                //     let key_config = KeyConfig::default();
                //     match *controller_tab {
                //         ControllerTab::Keyboard => {
                //             macro_rules! button {
                //                 {$key:ident} => {
                //                     let kc = key_config.$key.extract_keycode().unwrap();
                //                     config.key_config_mut().$key.insert_keycode(kc);
                //                 }
                //             }
                //             button!(up);
                //             button!(down);
                //             button!(left);
                //             button!(right);
                //             button!(a);
                //             button!(b);
                //             button!(start);
                //             button!(select);
                //         }
                //         ControllerTab::Gamepad => {
                //             macro_rules! button {
                //                 {$key:ident} => {
                //                     let button = key_config.$key.extract_gamepad().unwrap();
                //                     config.key_config_mut().$key.insert_gamepad(button);
                //                 }
                //             }
                //             button!(up);
                //             button!(down);
                //             button!(left);
                //             button!(right);
                //             button!(a);
                //             button!(b);
                //             button!(start);
                //             button!(select);
                //         }
                //     }
                //     *controller_button_ix = 0;
                //     config.save().unwrap();
                // }
            }
            MenuTab::HotKey => {
                ui.heading("HotKey Settings");

                let grid = |ui: &mut egui::Ui| {
                    ui.label("HotKey");
                    ui.label("Assignment");
                    ui.end_row();

                    ui.separator();
                    ui.separator();
                    ui.end_row();

                    let mut ix = 1;
                    let mut changed = false;
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
                            let key_assign = config.hotkeys_mut().key_assign_mut(hotkey).unwrap();
                            for i in 0..key_assign.0.len() {
                                let key_str = if *hotkey_select == ix {
                                    if hotkey_determined {
                                        *hotkey_select = 0;
                                        key_assign.0[i] =
                                            MultiKey(constructing_hotkey.clone().unwrap());
                                        *constructing_hotkey = None;
                                        changed = true;
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
                                    changed = true;
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

                    if changed {
                        config.save().unwrap();
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
                    *config.hotkeys_mut() = HotKeys::default();
                    config.save().unwrap();
                }
            }
        });
    });
}

fn file_field(
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
            .map_or_else(|| "N/A".to_string(), |r| r.display().to_string());
        ui.add(egui::TextEdit::singleline(&mut s.as_ref()));
    });
    ret
}
