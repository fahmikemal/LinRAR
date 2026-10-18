use eframe::egui::{self, Align, Color32, Layout, RichText, ScrollArea, Vec2};
use egui_extras::{Column, TableBuilder};
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::converter::ArchiveConverter;
use crate::core::detector::{detect_archive_type, is_supported_archive};
use crate::core::finder::ArchiveFinder;
use crate::core::rar_engine::RarEngine;
use crate::core::repair::ArchiveRepair;
use crate::core::report::{ArchiveReport, ReportFormat};
use crate::core::tar_engine::TarEngine;
use crate::core::zip_engine::ZipEngine;
use crate::gui::dialogs::DialogState;
use crate::gui::icons::IconManager;
use crate::types::{AddOptions, ArchiveEntry, ArchiveInfo, ArchiveType, CompressionLevel};

#[derive(PartialEq, Eq)]
pub enum ViewMode {
    LocalFileSystem,
    VirtualArchive,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum ThemeMode {
    Dark,
    Light,
}

#[allow(dead_code)]
pub struct LocalFileItem {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub modified: String,
}

pub struct LinrarApp {
    pub view_mode: ViewMode,
    pub current_dir: PathBuf,
    pub local_items: Vec<LocalFileItem>,
    pub selected_local: Vec<PathBuf>,

    pub current_archive: Option<PathBuf>,
    pub archive_info: Option<ArchiveInfo>,
    pub archive_entries: Vec<ArchiveEntry>,
    pub archive_subpath: String,
    pub selected_archive_files: Vec<String>,

    pub dialog_state: DialogState,
    pub dialog_frames_open: u32,
    #[allow(dead_code)]
    pub icons: Option<IconManager>,
    pub status_message: String,
    pub show_comment_pane: bool,
    pub theme_mode: ThemeMode,
}

impl LinrarApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_path: Option<PathBuf>) -> Self {
        let icons = Some(IconManager::new(&cc.egui_ctx));

        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/home"));
        let mut app = Self {
            view_mode: ViewMode::LocalFileSystem,
            current_dir,
            local_items: Vec::new(),
            selected_local: Vec::new(),
            current_archive: None,
            archive_info: None,
            archive_entries: Vec::new(),
            archive_subpath: String::new(),
            selected_archive_files: Vec::new(),
            dialog_state: DialogState::None,
            dialog_frames_open: 0,
            icons,
            status_message: "Ready".to_string(),
            show_comment_pane: true,
            theme_mode: ThemeMode::Dark,
        };

        if let Some(path) = initial_path {
            if path.exists() {
                if is_supported_archive(&path) {
                    app.open_archive(&path);
                } else if path.is_dir() {
                    app.navigate_local(&path);
                }
            }
        } else {
            app.refresh_local();
        }

        app
    }

    pub fn navigate_local(&mut self, path: &Path) {
        if path.is_dir() {
            self.current_dir = path.to_path_buf();
            self.selected_local.clear();
            self.refresh_local();
        }
    }

    pub fn refresh_local(&mut self) {
        self.local_items.clear();
        self.view_mode = ViewMode::LocalFileSystem;
        self.current_archive = None;
        self.archive_info = None;

        if let Ok(entries) = fs::read_dir(&self.current_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                let is_dir = path.is_dir();
                let name = entry.file_name().to_string_lossy().to_string();
                let size = if is_dir {
                    0
                } else {
                    entry.metadata().map(|m| m.len()).unwrap_or(0)
                };

                self.local_items.push(LocalFileItem {
                    name,
                    path,
                    is_dir,
                    size,
                    modified: "-".to_string(),
                });
            }
        }

        self.local_items.sort_by(|a, b| {
            if a.is_dir == b.is_dir {
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            } else if a.is_dir {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        });

        self.status_message = format!(
            "{} objects in {}",
            self.local_items.len(),
            self.current_dir.display()
        );
    }

    pub fn open_archive(&mut self, path: &Path) {
        let arch_type = detect_archive_type(path);
        let result = match arch_type {
            ArchiveType::Rar5 | ArchiveType::Rar4 => RarEngine::read_entries(path),
            ArchiveType::Zip => ZipEngine::read_entries(path),
            ArchiveType::Tar | ArchiveType::TarGz | ArchiveType::TarBz2 => {
                TarEngine::read_entries(path, arch_type)
            }
            _ => Err("Unsupported archive format".to_string()),
        };

        match result {
            Ok((info, entries)) => {
                self.current_archive = Some(path.to_path_buf());
                self.archive_info = Some(info);
                self.archive_entries = entries;
                self.archive_subpath.clear();
                self.selected_archive_files.clear();
                self.view_mode = ViewMode::VirtualArchive;
                self.status_message = format!(
                    "Opened archive: {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                );
            }
            Err(e) => {
                self.dialog_state = DialogState::Message {
                    title: "Error Opening Archive".to_string(),
                    message: e,
                    is_error: true,
                };
            }
        }
    }

    pub fn visible_archive_entries(&self) -> Vec<ArchiveEntry> {
        let prefix = if self.archive_subpath.is_empty() {
            "".to_string()
        } else {
            format!("{}/", self.archive_subpath)
        };

        let mut seen_dirs = std::collections::HashSet::new();
        let mut results = Vec::new();

        for entry in &self.archive_entries {
            let relative = if prefix.is_empty() {
                &entry.full_path
            } else if entry.full_path.starts_with(&prefix) {
                &entry.full_path[prefix.len()..]
            } else {
                continue;
            };

            let parts: Vec<&str> = relative.trim_end_matches('/').split('/').collect();
            if parts.is_empty() || parts[0].is_empty() {
                continue;
            }

            if parts.len() == 1 {
                results.push(entry.clone());
            } else {
                let dir_name = parts[0].to_string();
                if seen_dirs.insert(dir_name.clone()) {
                    let mut dir_entry = entry.clone();
                    dir_entry.name = dir_name.clone();
                    dir_entry.full_path = if prefix.is_empty() {
                        dir_name
                    } else {
                        format!("{}{}", prefix, dir_name)
                    };
                    dir_entry.is_dir = true;
                    results.push(dir_entry);
                }
            }
        }

        results
    }

    pub fn format_size(bytes: u64) -> String {
        if bytes < 1024 {
            format!("{} B", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }

    fn render_icon_button(
        &self,
        ui: &mut egui::Ui,
        icon: Option<&egui::TextureHandle>,
        text: &str,
        strong: bool,
    ) -> egui::Response {
        let mut rt = RichText::new(text).size(12.0);
        if strong {
            rt = rt.strong();
        }
        if let Some(tex) = icon {
            ui.add(egui::Button::image_and_text(
                egui::Image::from_texture((tex.id(), egui::vec2(20.0, 20.0))),
                rt,
            ))
        } else {
            ui.button(rt)
        }
    }

    pub fn accent_color(&self) -> Color32 {
        match self.theme_mode {
            ThemeMode::Dark => Color32::from_rgb(32, 194, 214),
            ThemeMode::Light => Color32::from_rgb(0, 110, 150),
        }
    }

    fn handle_window_resize(&self, ctx: &egui::Context) {
        let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        if is_maximized {
            return;
        }

        let screen_rect = ctx.viewport_rect();
        let margin = 5.0;
        if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
            let on_left = pos.x >= screen_rect.left() && pos.x <= screen_rect.left() + margin;
            let on_right = pos.x >= screen_rect.right() - margin && pos.x <= screen_rect.right();
            // Don't trigger North resize over the titlebar buttons (approx 120px from right) unless it's the corner
            let over_buttons =
                pos.x >= screen_rect.right() - 120.0 && pos.y <= screen_rect.top() + 28.0;
            let on_top = pos.y >= screen_rect.top()
                && pos.y <= screen_rect.top() + margin
                && (!over_buttons || on_right);
            let on_bottom = pos.y >= screen_rect.bottom() - margin && pos.y <= screen_rect.bottom();

            let resize_dir = match (on_left, on_right, on_top, on_bottom) {
                (true, _, true, _) => Some((
                    egui::ResizeDirection::NorthWest,
                    egui::CursorIcon::ResizeNorthWest,
                )),
                (true, _, _, true) => Some((
                    egui::ResizeDirection::SouthWest,
                    egui::CursorIcon::ResizeSouthWest,
                )),
                (_, true, true, _) => Some((
                    egui::ResizeDirection::NorthEast,
                    egui::CursorIcon::ResizeNorthEast,
                )),
                (_, true, _, true) => Some((
                    egui::ResizeDirection::SouthEast,
                    egui::CursorIcon::ResizeSouthEast,
                )),
                (true, _, _, _) => {
                    Some((egui::ResizeDirection::West, egui::CursorIcon::ResizeWest))
                }
                (_, true, _, _) => {
                    Some((egui::ResizeDirection::East, egui::CursorIcon::ResizeEast))
                }
                (_, _, true, _) => {
                    Some((egui::ResizeDirection::North, egui::CursorIcon::ResizeNorth))
                }
                (_, _, _, true) => {
                    Some((egui::ResizeDirection::South, egui::CursorIcon::ResizeSouth))
                }
                _ => None,
            };

            if let Some((dir, cursor)) = resize_dir {
                ctx.set_cursor_icon(cursor);
                if ctx.input(|i| i.pointer.primary_pressed()) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
                }
            }
        }
    }
}

impl eframe::App for LinrarApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        match self.theme_mode {
            ThemeMode::Dark => {
                ui.ctx().set_visuals(egui::Visuals::dark());
            }
            ThemeMode::Light => {
                ui.ctx().set_visuals(egui::Visuals::light());
            }
        }

        // Set Dynamic Window Title for Wayland / X11 (e.g. "payload.rar - LinRAR")
        let window_title = match &self.current_archive {
            Some(arch) => format!(
                "{} - LinRAR",
                arch.file_name().unwrap_or_default().to_string_lossy()
            ),
            None => {
                if self.view_mode == ViewMode::LocalFileSystem {
                    let dir_name = self
                        .current_dir
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy();
                    if dir_name.is_empty() {
                        "LinRAR".to_string()
                    } else {
                        format!("{} - LinRAR", dir_name)
                    }
                } else {
                    "LinRAR".to_string()
                }
            }
        };
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Title(window_title));
        self.handle_window_resize(ui.ctx());

        // Global Keyboard Shortcuts
        if ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::A)) {
            self.trigger_add_dialog();
        }
        if ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::E)) {
            self.trigger_extract_dialog();
        }
        if ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::T)) {
            self.trigger_test_archive();
        }
        if ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::F)) {
            self.trigger_find_dialog();
        }
        if ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::R)) {
            self.trigger_repair_dialog();
        }
        if ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::Q)) {
            self.trigger_convert_dialog();
        }
        if ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::S)) {
            self.trigger_scan_archive();
        }
        if ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::B)) {
            self.dialog_state = DialogState::new_benchmark();
        }
        if ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::I)) {
            self.trigger_info_dialog();
        }
        if ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::R)) {
            self.trigger_report_dialog();
        }
        if ui.input(|i| i.key_pressed(egui::Key::F2)) {
            self.trigger_rename_dialog();
        }
        if ui.input(|i| i.key_pressed(egui::Key::Delete)) {
            self.trigger_delete();
        }

        // Custom CSD Title Bar (Logo, Wordmark, Draggable Area, and Window Controls)
        egui::Panel::top("custom_title_bar").show(ui, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                if let Some(tex) = self.icons.as_ref().and_then(|i| i.app_icon.as_ref()) {
                    ui.add(egui::Image::from_texture((
                        tex.id(),
                        egui::vec2(18.0, 18.0),
                    )));
                }
                let brand_text = match &self.current_archive {
                    Some(arch) => format!(
                        "LinRAR - {}",
                        arch.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    None => {
                        if self.view_mode == ViewMode::LocalFileSystem {
                            let dir_name = self
                                .current_dir
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy();
                            if dir_name.is_empty() {
                                "LinRAR".to_string()
                            } else {
                                format!("LinRAR - {}", dir_name)
                            }
                        } else {
                            "LinRAR".to_string()
                        }
                    }
                };

                let title_resp = ui.add(
                    egui::Label::new(
                        RichText::new(brand_text)
                            .strong()
                            .size(13.0)
                            .color(self.accent_color()),
                    )
                    .sense(egui::Sense::click_and_drag()),
                );
                if title_resp.dragged() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if title_resp.double_clicked() {
                    let is_max = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(!is_max));
                }

                // Window controls aligned on the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);

                    // Close button
                    let (close_rect, close_resp) =
                        ui.allocate_exact_size(Vec2::new(38.0, 24.0), egui::Sense::click());
                    if close_resp.hovered() {
                        ui.painter()
                            .rect_filled(close_rect, 2.0, Color32::from_rgb(220, 50, 50));
                        ui.painter().text(
                            close_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "✕",
                            egui::FontId::proportional(12.0),
                            Color32::WHITE,
                        );
                    } else {
                        ui.painter().text(
                            close_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "✕",
                            egui::FontId::proportional(12.0),
                            ui.visuals().text_color(),
                        );
                    }
                    if close_resp.clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }

                    // Maximize / Restore button
                    let is_maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                    let (max_rect, max_resp) =
                        ui.allocate_exact_size(Vec2::new(36.0, 24.0), egui::Sense::click());
                    if max_resp.hovered() {
                        let hover_bg = if self.theme_mode == ThemeMode::Dark {
                            Color32::from_white_alpha(35)
                        } else {
                            Color32::from_black_alpha(25)
                        };
                        ui.painter().rect_filled(max_rect, 2.0, hover_bg);
                    }
                    let max_icon = if is_maximized { "🗗" } else { "🗖" };
                    ui.painter().text(
                        max_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        max_icon,
                        egui::FontId::proportional(12.0),
                        ui.visuals().text_color(),
                    );
                    if max_resp.clicked() {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                    }

                    // Minimize button
                    let (min_rect, min_resp) =
                        ui.allocate_exact_size(Vec2::new(36.0, 24.0), egui::Sense::click());
                    if min_resp.hovered() {
                        let hover_bg = if self.theme_mode == ThemeMode::Dark {
                            Color32::from_white_alpha(35)
                        } else {
                            Color32::from_black_alpha(25)
                        };
                        ui.painter().rect_filled(min_rect, 2.0, hover_bg);
                    }
                    ui.painter().text(
                        min_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "—",
                        egui::FontId::proportional(12.0),
                        ui.visuals().text_color(),
                    );
                    if min_resp.clicked() {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }

                    // Middle draggable area spanning between the title and controls
                    let available_w = ui.available_width().max(0.0);
                    let (_drag_rect, drag_resp) = ui.allocate_exact_size(
                        Vec2::new(available_w, 24.0),
                        egui::Sense::click_and_drag(),
                    );
                    if drag_resp.dragged() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    if drag_resp.double_clicked() {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                    }
                });
            });
            ui.add_space(2.0);
        });

        // Top Menu Bar
        egui::Panel::top("top_menu_bar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open Archive... (Ctrl+O)").clicked() {
                        ui.close();
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Archives", &["rar", "zip", "tar", "gz", "bz2", "7z"])
                            .pick_file()
                        {
                            self.open_archive(&path);
                        }
                    }
                    if ui.button("Browse Folder...").clicked() {
                        ui.close();
                        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                            self.navigate_local(&folder);
                        }
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        std::process::exit(0);
                    }
                });

                ui.menu_button("Commands", |ui| {
                    if ui.button("Add files to archive (Alt+A)").clicked() {
                        ui.close();
                        self.trigger_add_dialog();
                    }
                    if ui.button("Extract to specified folder (Alt+E)").clicked() {
                        ui.close();
                        self.trigger_extract_dialog();
                    }
                    if ui.button("Test archive files (Alt+T)").clicked() {
                        ui.close();
                        self.trigger_test_archive();
                    }
                    if ui.button("Delete files (Del)").clicked() {
                        ui.close();
                        self.trigger_delete();
                    }
                    if ui.button("Rename file (F2)").clicked() {
                        ui.close();
                        self.trigger_rename_dialog();
                    }
                    ui.separator();
                    if ui
                        .button("Protect archive (Recovery Record)... (Alt+P)")
                        .clicked()
                    {
                        ui.close();
                        self.trigger_protect_dialog();
                    }
                    if ui.button("Lock archive").clicked() {
                        ui.close();
                        self.trigger_lock_archive();
                    }
                    if ui.button("Repair damaged archive (Alt+R)").clicked() {
                        ui.close();
                        self.trigger_repair_dialog();
                    }
                    if ui.button("Convert archive format (Alt+Q)").clicked() {
                        ui.close();
                        self.trigger_convert_dialog();
                    }
                    ui.separator();
                    if ui.button("Generate report... (Ctrl+R)").clicked() {
                        ui.close();
                        self.trigger_report_dialog();
                    }
                    if ui.button("Show archive information (Alt+I)").clicked() {
                        ui.close();
                        self.trigger_info_dialog();
                    }
                });

                ui.menu_button("Tools", |ui| {
                    if ui.button("Find files... (Alt+F)").clicked() {
                        ui.close();
                        self.trigger_find_dialog();
                    }
                    if ui.button("Repair archive... (Alt+R)").clicked() {
                        ui.close();
                        self.trigger_repair_dialog();
                    }
                    if ui.button("Convert archive... (Alt+Q)").clicked() {
                        ui.close();
                        self.trigger_convert_dialog();
                    }
                    if ui.button("Generate report... (Ctrl+R)").clicked() {
                        ui.close();
                        self.trigger_report_dialog();
                    }
                    ui.separator();
                    if ui.button("Scan archive for viruses... (Alt+S)").clicked() {
                        ui.close();
                        self.trigger_scan_archive();
                    }
                    if ui.button("Benchmark and hardware test (Alt+B)").clicked() {
                        ui.close();
                        self.dialog_state = DialogState::new_benchmark();
                    }
                    if ui.button("Toggle Comment Pane").clicked() {
                        ui.close();
                        self.show_comment_pane = !self.show_comment_pane;
                    }
                });

                ui.menu_button("Theme", |ui| {
                    if ui
                        .selectable_label(self.theme_mode == ThemeMode::Dark, "Dark Theme")
                        .clicked()
                    {
                        self.theme_mode = ThemeMode::Dark;
                        ui.close();
                    }
                    if ui
                        .selectable_label(self.theme_mode == ThemeMode::Light, "Light Theme")
                        .clicked()
                    {
                        self.theme_mode = ThemeMode::Light;
                        ui.close();
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("License (MIT)").clicked() {
                        ui.close();
                        self.dialog_state = DialogState::TrialReminder;
                    }
                    ui.separator();
                    if ui.button("About LinRAR").clicked() {
                        ui.close();
                        self.dialog_state = DialogState::About;
                    }
                });
            });
        });

        // Toolbar Panel
        egui::Panel::top("toolbar_panel").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);

                if self
                    .render_icon_button(
                        ui,
                        self.icons.as_ref().and_then(|i| i.add_icon.as_ref()),
                        "Add",
                        true,
                    )
                    .clicked()
                {
                    self.trigger_add_dialog();
                }

                if self
                    .render_icon_button(
                        ui,
                        self.icons.as_ref().and_then(|i| i.extract_icon.as_ref()),
                        "Extract To",
                        true,
                    )
                    .clicked()
                {
                    self.trigger_extract_dialog();
                }

                if self
                    .render_icon_button(
                        ui,
                        self.icons.as_ref().and_then(|i| i.test_icon.as_ref()),
                        "Test",
                        true,
                    )
                    .clicked()
                {
                    self.trigger_test_archive();
                }

                if self
                    .render_icon_button(
                        ui,
                        self.icons.as_ref().and_then(|i| i.view_icon.as_ref()),
                        "View",
                        false,
                    )
                    .clicked()
                {
                    self.trigger_view_file();
                }

                if self
                    .render_icon_button(
                        ui,
                        self.icons.as_ref().and_then(|i| i.find_icon.as_ref()),
                        "Find",
                        false,
                    )
                    .clicked()
                {
                    self.trigger_find_dialog();
                }

                if self
                    .render_icon_button(
                        ui,
                        self.icons.as_ref().and_then(|i| i.repair_icon.as_ref()),
                        "Repair",
                        false,
                    )
                    .clicked()
                {
                    self.trigger_repair_dialog();
                }

                if self
                    .render_icon_button(
                        ui,
                        self.icons.as_ref().and_then(|i| i.convert_icon.as_ref()),
                        "Convert",
                        false,
                    )
                    .clicked()
                {
                    self.trigger_convert_dialog();
                }

                if self
                    .render_icon_button(
                        ui,
                        self.icons.as_ref().and_then(|i| i.report_icon.as_ref()),
                        "Report",
                        false,
                    )
                    .clicked()
                {
                    self.trigger_report_dialog();
                }

                if self
                    .render_icon_button(
                        ui,
                        self.icons.as_ref().and_then(|i| i.scan_icon.as_ref()),
                        "Scan",
                        false,
                    )
                    .clicked()
                {
                    self.trigger_scan_archive();
                }

                if self
                    .render_icon_button(
                        ui,
                        self.icons.as_ref().and_then(|i| i.benchmark_icon.as_ref()),
                        "Benchmark",
                        false,
                    )
                    .clicked()
                {
                    self.dialog_state = DialogState::new_benchmark();
                }

                ui.separator();

                if self
                    .render_icon_button(
                        ui,
                        self.icons.as_ref().and_then(|i| i.info_icon.as_ref()),
                        "Info",
                        false,
                    )
                    .clicked()
                {
                    self.trigger_info_dialog();
                }

                if self
                    .render_icon_button(
                        ui,
                        self.icons.as_ref().and_then(|i| i.delete_icon.as_ref()),
                        "Delete",
                        false,
                    )
                    .clicked()
                {
                    self.trigger_delete();
                }
            });
            ui.add_space(4.0);
        });

        // Address & Navigation Bar
        egui::Panel::top("address_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                if self
                    .render_icon_button(
                        ui,
                        self.icons.as_ref().and_then(|i| i.up_icon.as_ref()),
                        "Up",
                        false,
                    )
                    .clicked()
                {
                    self.go_up_one_level();
                }

                if self
                    .render_icon_button(
                        ui,
                        self.icons.as_ref().and_then(|i| i.refresh_icon.as_ref()),
                        "Refresh",
                        false,
                    )
                    .clicked()
                {
                    if self.view_mode == ViewMode::LocalFileSystem {
                        self.refresh_local();
                    } else if let Some(path) = self.current_archive.clone() {
                        self.open_archive(&path);
                    }
                }

                let current_display = match self.view_mode {
                    ViewMode::LocalFileSystem => self.current_dir.display().to_string(),
                    ViewMode::VirtualArchive => {
                        let arch_name = self
                            .current_archive
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default();
                        if self.archive_subpath.is_empty() {
                            arch_name
                        } else {
                            format!("{}/{}", arch_name, self.archive_subpath)
                        }
                    }
                };

                let path_color = self.accent_color();
                ui.monospace(RichText::new(current_display).color(path_color));
            });
        });

        // Status Bar
        egui::Panel::bottom("status_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                let status_color = if self.theme_mode == ThemeMode::Light {
                    Color32::from_rgb(70, 70, 70)
                } else {
                    Color32::LIGHT_GRAY
                };
                ui.label(
                    RichText::new(&self.status_message)
                        .size(11.0)
                        .color(status_color),
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(info) = &self.archive_info {
                        let flags = format!(
                            "{}{} | {} files",
                            if info.is_solid { "[Solid] " } else { "" },
                            if info.is_locked { "[Locked] " } else { "" },
                            info.total_files
                        );
                        let flag_color = self.accent_color();
                        ui.label(RichText::new(flags).size(11.0).color(flag_color));
                    } else {
                        let brand_color = if self.theme_mode == ThemeMode::Light {
                            Color32::from_rgb(100, 100, 100)
                        } else {
                            Color32::GRAY
                        };
                        ui.label(
                            RichText::new("LinRAR 0.1.0 (Rust Engine)")
                                .size(11.0)
                                .color(brand_color),
                        );
                    }
                });
            });
        });

        // Side Comment Pane (if active and archive has comment)
        if self.show_comment_pane
            && let Some(info) = &self.archive_info
            && let Some(cmt) = &info.comment
        {
            egui::Panel::right("comment_panel")
                .resizable(true)
                .default_size(220.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if let Some(tex) = self.icons.as_ref().and_then(|i| i.comment_icon.as_ref())
                        {
                            ui.add(egui::Image::from_texture((
                                tex.id(),
                                egui::vec2(16.0, 16.0),
                            )));
                        }
                        ui.heading(RichText::new("Archive Comment").size(14.0));
                    });
                    ui.separator();
                    ScrollArea::vertical().show(ui, |ui| {
                        ui.monospace(cmt);
                    });
                });
        }

        // Central File Table
        egui::CentralPanel::default().show(ui, |ui| match self.view_mode {
            ViewMode::LocalFileSystem => self.render_local_table(ui),
            ViewMode::VirtualArchive => self.render_archive_table(ui),
        });

        // Render Active Modal Dialog
        let ctx = ui.ctx().clone();
        self.render_dialogs(&ctx);
    }
}

impl LinrarApp {
    fn render_local_table(&mut self, ui: &mut egui::Ui) {
        let mut to_navigate = None;
        let mut to_open_archive = None;
        let is_ctrl = ui.input(|i| i.modifiers.ctrl);

        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .sense(egui::Sense::click())
            .column(Column::remainder().at_least(220.0))
            .column(Column::exact(100.0))
            .column(Column::exact(90.0))
            .header(24.0, |mut header| {
                header.col(|ui| {
                    ui.strong("Name");
                });
                header.col(|ui| {
                    ui.strong("Size");
                });
                header.col(|ui| {
                    ui.strong("Type");
                });
            })
            .body(|mut body| {
                for item in &self.local_items {
                    let is_selected = self.selected_local.contains(&item.path);
                    let icon = self.icons.as_ref().and_then(|ic| {
                        if item.is_dir {
                            ic.folder_icon.as_ref()
                        } else if is_supported_archive(&item.path) {
                            ic.archive_icon.as_ref()
                        } else {
                            ic.file_icon.as_ref()
                        }
                    });

                    body.row(22.0, |mut row| {
                        row.set_selected(is_selected);

                        row.col(|ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
                                if let Some(tex) = icon {
                                    ui.add(egui::Image::from_texture((
                                        tex.id(),
                                        egui::vec2(16.0, 16.0),
                                    )));
                                }
                                ui.label(&item.name);
                            });
                        });

                        row.col(|ui| {
                            ui.label(if item.is_dir {
                                "-".to_string()
                            } else {
                                Self::format_size(item.size)
                            });
                        });

                        row.col(|ui| {
                            ui.label(if item.is_dir { "Folder" } else { "File" });
                        });

                        let resp = row.response();

                        if resp.clicked() {
                            if is_ctrl {
                                if is_selected {
                                    self.selected_local.retain(|p| p != &item.path);
                                } else {
                                    self.selected_local.push(item.path.clone());
                                }
                            } else {
                                self.selected_local = vec![item.path.clone()];
                            }
                        }

                        if resp.double_clicked() {
                            if item.is_dir {
                                to_navigate = Some(item.path.clone());
                            } else if is_supported_archive(&item.path) {
                                to_open_archive = Some(item.path.clone());
                            }
                        }
                    });
                }
            });

        if let Some(p) = to_navigate {
            self.navigate_local(&p);
        } else if let Some(p) = to_open_archive {
            self.open_archive(&p);
        }
    }

    fn render_archive_table(&mut self, ui: &mut egui::Ui) {
        let mut enter_folder = None;
        let visible_entries = self.visible_archive_entries();
        let is_ctrl = ui.input(|i| i.modifiers.ctrl);

        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .sense(egui::Sense::click())
            .column(Column::remainder().at_least(200.0))
            .column(Column::exact(100.0))
            .column(Column::exact(100.0))
            .column(Column::exact(80.0))
            .column(Column::exact(90.0))
            .header(24.0, |mut header| {
                header.col(|ui| {
                    ui.strong("Name");
                });
                header.col(|ui| {
                    ui.strong("Size");
                });
                header.col(|ui| {
                    ui.strong("Packed");
                });
                header.col(|ui| {
                    ui.strong("Ratio");
                });
                header.col(|ui| {
                    ui.strong("CRC32");
                });
            })
            .body(|mut body| {
                for entry in visible_entries {
                    let is_selected = self.selected_archive_files.contains(&entry.full_path);
                    let icon = self.icons.as_ref().and_then(|ic| {
                        if entry.is_dir {
                            ic.folder_icon.as_ref()
                        } else if entry.name.ends_with(".rar")
                            || entry.name.ends_with(".zip")
                            || entry.name.ends_with(".tar")
                            || entry.name.ends_with(".tar.gz")
                        {
                            ic.archive_icon.as_ref()
                        } else {
                            ic.file_icon.as_ref()
                        }
                    });

                    body.row(22.0, |mut row| {
                        row.set_selected(is_selected);

                        row.col(|ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
                                if let Some(tex) = icon {
                                    ui.add(egui::Image::from_texture((
                                        tex.id(),
                                        egui::vec2(16.0, 16.0),
                                    )));
                                }
                                ui.label(&entry.name);
                            });
                        });

                        row.col(|ui| {
                            ui.label(Self::format_size(entry.size));
                        });

                        row.col(|ui| {
                            ui.label(Self::format_size(entry.packed_size));
                        });

                        row.col(|ui| {
                            let ratio = if entry.size > 0 {
                                format!(
                                    "{:.0}%",
                                    (entry.packed_size as f64 / entry.size as f64) * 100.0
                                )
                            } else {
                                "-".to_string()
                            };
                            ui.label(ratio);
                        });

                        row.col(|ui| {
                            ui.monospace(&entry.crc);
                        });

                        let resp = row.response();

                        if resp.clicked() {
                            if is_ctrl {
                                if is_selected {
                                    self.selected_archive_files
                                        .retain(|p| p != &entry.full_path);
                                } else {
                                    self.selected_archive_files.push(entry.full_path.clone());
                                }
                            } else {
                                self.selected_archive_files = vec![entry.full_path.clone()];
                            }
                        }

                        if resp.double_clicked() && entry.is_dir {
                            enter_folder = Some(entry.name.clone());
                        }
                    });
                }
            });

        if let Some(folder) = enter_folder {
            if self.archive_subpath.is_empty() {
                self.archive_subpath = folder;
            } else {
                self.archive_subpath = format!("{}/{}", self.archive_subpath, folder);
            }
        }
    }

    fn go_up_one_level(&mut self) {
        match self.view_mode {
            ViewMode::LocalFileSystem => {
                if let Some(parent) = self.current_dir.parent() {
                    let p = parent.to_path_buf();
                    self.navigate_local(&p);
                }
            }
            ViewMode::VirtualArchive => {
                if self.archive_subpath.is_empty() {
                    self.refresh_local();
                } else {
                    let parts: Vec<&str> = self.archive_subpath.split('/').collect();
                    if parts.len() <= 1 {
                        self.archive_subpath.clear();
                    } else {
                        self.archive_subpath = parts[..parts.len() - 1].join("/");
                    }
                }
            }
        }
    }

    fn trigger_add_dialog(&mut self) {
        if self.selected_local.is_empty() {
            self.dialog_state = DialogState::Message {
                title: "No Files Selected".to_string(),
                message: "Please select one or more files or folders to add to an archive."
                    .to_string(),
                is_error: false,
            };
            return;
        }

        self.dialog_state = DialogState::new_add(self.selected_local.clone(), &self.current_dir);
    }

    fn trigger_extract_dialog(&mut self) {
        let arch_path = match self.view_mode {
            ViewMode::VirtualArchive => self.current_archive.clone(),
            ViewMode::LocalFileSystem => {
                if let Some(first) = self.selected_local.first() {
                    if is_supported_archive(first) {
                        Some(first.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        };

        if let Some(path) = arch_path {
            self.dialog_state = DialogState::new_extract(path);
        } else {
            self.dialog_state = DialogState::Message {
                title: "Extract".to_string(),
                message: "Please select an archive file to extract.".to_string(),
                is_error: false,
            };
        }
    }

    fn trigger_test_archive(&mut self) {
        let target = match self.view_mode {
            ViewMode::VirtualArchive => self.current_archive.clone(),
            ViewMode::LocalFileSystem => self.selected_local.first().cloned(),
        };

        if let Some(path) = target {
            if is_supported_archive(&path) {
                let arch_type = detect_archive_type(&path);
                let res = match arch_type {
                    ArchiveType::Zip => ZipEngine::test(&path),
                    ArchiveType::TarGz | ArchiveType::TarBz2 | ArchiveType::Tar => {
                        TarEngine::test(&path, arch_type)
                    }
                    _ => RarEngine::test(&path),
                };

                match res {
                    Ok(true) => {
                        self.dialog_state = DialogState::Message {
                            title: "Test Archive Integrity".to_string(),
                            message: format!(
                                "Archive: {}\n\nAll archive files passed integrity verification successfully. No errors found!",
                                path.file_name().unwrap_or_default().to_string_lossy()
                            ),
                            is_error: false,
                        };
                    }
                    Ok(false) => {
                        self.dialog_state = DialogState::Message {
                            title: "Test Archive Integrity".to_string(),
                            message: format!(
                                "Archive: {}\n\nErrors were detected during archive integrity testing.",
                                path.file_name().unwrap_or_default().to_string_lossy()
                            ),
                            is_error: true,
                        };
                    }
                    Err(e) => {
                        self.dialog_state = DialogState::Message {
                            title: "Test Archive Error".to_string(),
                            message: e,
                            is_error: true,
                        };
                    }
                }
            } else {
                self.dialog_state = DialogState::Message {
                    title: "Test Archive".to_string(),
                    message: format!(
                        "'{}' is not a supported archive format.\nPlease select a valid RAR, ZIP, or TAR archive to test.",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    is_error: false,
                };
            }
        } else {
            self.dialog_state = DialogState::Message {
                title: "Test Archive".to_string(),
                message: "Please open an archive or select a supported archive file (.rar, .zip, .tar, etc.) to test its integrity.".to_string(),
                is_error: false,
            };
        }
    }

    fn trigger_repair_dialog(&mut self) {
        let arch = match self.view_mode {
            ViewMode::VirtualArchive => self.current_archive.clone(),
            ViewMode::LocalFileSystem => self
                .selected_local
                .first()
                .filter(|p| is_supported_archive(p))
                .cloned(),
        };
        if let Some(path) = arch {
            self.dialog_state = DialogState::new_repair(path);
        } else {
            self.dialog_state = DialogState::Message {
                title: "Repair Archive".to_string(),
                message: "Please open or select an archive to repair.".to_string(),
                is_error: false,
            };
        }
    }

    fn trigger_convert_dialog(&mut self) {
        let arch = match self.view_mode {
            ViewMode::VirtualArchive => self.current_archive.clone(),
            ViewMode::LocalFileSystem => self
                .selected_local
                .first()
                .filter(|p| is_supported_archive(p))
                .cloned(),
        };
        if let Some(path) = arch {
            self.dialog_state = DialogState::new_convert(path);
        } else {
            self.dialog_state = DialogState::Message {
                title: "Convert Archive".to_string(),
                message: "Please open or select an archive to convert.".to_string(),
                is_error: false,
            };
        }
    }

    fn trigger_find_dialog(&mut self) {
        let dir = match self.view_mode {
            ViewMode::LocalFileSystem => self.current_dir.clone(),
            ViewMode::VirtualArchive => self
                .current_archive
                .as_ref()
                .and_then(|p| p.parent())
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| self.current_dir.clone()),
        };
        self.dialog_state = DialogState::new_find(dir);
    }

    fn trigger_report_dialog(&mut self) {
        let arch = match self.view_mode {
            ViewMode::VirtualArchive => self.current_archive.clone(),
            ViewMode::LocalFileSystem => self
                .selected_local
                .first()
                .filter(|p| is_supported_archive(p))
                .cloned(),
        };
        if let Some(path) = arch {
            self.dialog_state = DialogState::new_report(path);
        } else {
            self.dialog_state = DialogState::Message {
                title: "Generate Report".to_string(),
                message: "Please open or select an archive to generate a report.".to_string(),
                is_error: false,
            };
        }
    }

    pub fn trigger_info_dialog(&mut self) {
        match self.view_mode {
            ViewMode::VirtualArchive => {
                if let Some(info) = &self.archive_info {
                    self.dialog_state = DialogState::ArchiveInfo(info.clone());
                } else if let Some(path) = &self.current_archive {
                    let arch_type = detect_archive_type(path);
                    let result = match arch_type {
                        ArchiveType::Rar5 | ArchiveType::Rar4 => RarEngine::read_entries(path),
                        ArchiveType::Zip => ZipEngine::read_entries(path),
                        ArchiveType::Tar | ArchiveType::TarGz | ArchiveType::TarBz2 => {
                            TarEngine::read_entries(path, arch_type)
                        }
                        _ => Err("Unsupported archive format".to_string()),
                    };
                    match result {
                        Ok((info, _)) => {
                            self.archive_info = Some(info.clone());
                            self.dialog_state = DialogState::ArchiveInfo(info);
                        }
                        Err(e) => {
                            self.dialog_state = DialogState::Message {
                                title: "Archive Information".to_string(),
                                message: format!("Could not read archive details: {}", e),
                                is_error: true,
                            };
                        }
                    }
                } else {
                    self.dialog_state = DialogState::Message {
                        title: "Information".to_string(),
                        message: "No archive is currently open.".to_string(),
                        is_error: false,
                    };
                }
            }
            ViewMode::LocalFileSystem => {
                if let Some(selected_path) = self.selected_local.first() {
                    if is_supported_archive(selected_path) {
                        let arch_type = detect_archive_type(selected_path);
                        let result = match arch_type {
                            ArchiveType::Rar5 | ArchiveType::Rar4 => {
                                RarEngine::read_entries(selected_path)
                            }
                            ArchiveType::Zip => ZipEngine::read_entries(selected_path),
                            ArchiveType::Tar | ArchiveType::TarGz | ArchiveType::TarBz2 => {
                                TarEngine::read_entries(selected_path, arch_type)
                            }
                            _ => Err("Unsupported archive format".to_string()),
                        };
                        match result {
                            Ok((info, _)) => {
                                self.dialog_state = DialogState::ArchiveInfo(info);
                            }
                            Err(e) => {
                                self.dialog_state = DialogState::Message {
                                    title: "Archive Information".to_string(),
                                    message: format!("Could not read archive info: {}", e),
                                    is_error: true,
                                };
                            }
                        }
                    } else if let Ok(meta) = fs::metadata(selected_path) {
                        let kind = if meta.is_dir() { "Directory" } else { "File" };
                        let name = selected_path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy();
                        let size_str = if meta.is_dir() {
                            "N/A (Directory)".to_string()
                        } else {
                            Self::format_size(meta.len())
                        };
                        let read_only = if meta.permissions().readonly() {
                            "Yes"
                        } else {
                            "No"
                        };
                        self.dialog_state = DialogState::Message {
                            title: format!("{} Information", kind),
                            message: format!(
                                "Name: {}\nType: {}\nPath: {}\nSize: {}\nRead-only: {}",
                                name,
                                kind,
                                selected_path.display(),
                                size_str,
                                read_only
                            ),
                            is_error: false,
                        };
                    } else {
                        self.dialog_state = DialogState::Message {
                            title: "Information".to_string(),
                            message: "Please select or open an archive to view its information."
                                .to_string(),
                            is_error: false,
                        };
                    }
                } else {
                    self.dialog_state = DialogState::Message {
                        title: "Information".to_string(),
                        message: "Please open an archive or select a file to view its information."
                            .to_string(),
                        is_error: false,
                    };
                }
            }
        }
    }

    fn trigger_protect_dialog(&mut self) {
        let arch = match self.view_mode {
            ViewMode::VirtualArchive => self.current_archive.clone(),
            ViewMode::LocalFileSystem => self
                .selected_local
                .first()
                .filter(|p| is_supported_archive(p))
                .cloned(),
        };
        if let Some(path) = arch {
            self.dialog_state = DialogState::new_protect(path);
        } else {
            self.dialog_state = DialogState::Message {
                title: "Protect Archive".to_string(),
                message: "Please open or select a RAR archive to add a recovery record."
                    .to_string(),
                is_error: false,
            };
        }
    }

    fn trigger_rename_dialog(&mut self) {
        if self.view_mode == ViewMode::VirtualArchive
            && let Some(arch) = self.current_archive.clone()
            && let Some(first) = self.selected_archive_files.first()
        {
            self.dialog_state = DialogState::new_rename(arch, first.clone());
            return;
        }
        self.dialog_state = DialogState::Message {
            title: "Rename".to_string(),
            message: "Please select a file inside the archive to rename.".to_string(),
            is_error: false,
        };
    }

    fn trigger_lock_archive(&mut self) {
        let arch = match self.view_mode {
            ViewMode::VirtualArchive => self.current_archive.clone(),
            ViewMode::LocalFileSystem => self
                .selected_local
                .first()
                .filter(|p| is_supported_archive(p))
                .cloned(),
        };
        if let Some(path) = arch {
            match RarEngine::lock(&path) {
                Ok(_) => {
                    self.dialog_state = DialogState::Message {
                        title: "Lock Archive".to_string(),
                        message: "Archive locked successfully against modifications.".to_string(),
                        is_error: false,
                    };
                    if let Some(cur) = self.current_archive.clone() {
                        self.open_archive(&cur);
                    }
                }
                Err(e) => {
                    self.dialog_state = DialogState::Message {
                        title: "Lock Archive Error".to_string(),
                        message: e,
                        is_error: true,
                    };
                }
            }
        }
    }

    fn trigger_delete(&mut self) {
        if self.view_mode == ViewMode::VirtualArchive {
            if let Some(arch) = self.current_archive.clone()
                && !self.selected_archive_files.is_empty()
            {
                let files = self.selected_archive_files.clone();
                if let Err(e) = RarEngine::delete(&arch, &files) {
                    self.dialog_state = DialogState::Message {
                        title: "Delete Error".to_string(),
                        message: e,
                        is_error: true,
                    };
                } else {
                    self.open_archive(&arch);
                }
            } else {
                self.dialog_state = DialogState::Message {
                    title: "Delete".to_string(),
                    message: "Please select one or more files in the archive to delete."
                        .to_string(),
                    is_error: false,
                };
            }
        } else if !self.selected_local.is_empty() {
            for p in &self.selected_local {
                if p.is_dir() {
                    let _ = fs::remove_dir_all(p);
                } else {
                    let _ = fs::remove_file(p);
                }
            }
            self.refresh_local();
        } else {
            self.dialog_state = DialogState::Message {
                title: "Delete".to_string(),
                message: "Please select one or more files or folders to delete.".to_string(),
                is_error: false,
            };
        }
    }

    fn trigger_view_file(&mut self) {
        match self.view_mode {
            ViewMode::LocalFileSystem => {
                if let Some(first) = self.selected_local.first() {
                    let _ = std::process::Command::new("xdg-open").arg(first).spawn();
                } else {
                    self.dialog_state = DialogState::Message {
                        title: "View File".to_string(),
                        message:
                            "Please select a file to view with the default system application."
                                .to_string(),
                        is_error: false,
                    };
                }
            }
            ViewMode::VirtualArchive => {
                self.dialog_state = DialogState::Message {
                    title: "View File".to_string(),
                    message:
                        "Please extract the file or use Extract To to view files inside an archive."
                            .to_string(),
                    is_error: false,
                };
            }
        }
    }

    fn trigger_scan_archive(&mut self) {
        let arch_path = match self.view_mode {
            ViewMode::VirtualArchive => self.current_archive.clone(),
            ViewMode::LocalFileSystem => {
                if let Some(first) = self.selected_local.first() {
                    if is_supported_archive(first) {
                        Some(first.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        };

        if let Some(path) = arch_path {
            match crate::core::scanner::VirusScanner::scan_archive(&path) {
                Ok(res) => {
                    self.dialog_state = DialogState::ScanResults(res);
                }
                Err(e) => {
                    self.dialog_state = DialogState::Message {
                        title: "Scan Error".to_string(),
                        message: e,
                        is_error: true,
                    };
                }
            }
        } else {
            self.dialog_state = DialogState::Message {
                title: "Scan Archive".to_string(),
                message: "Please open or select an archive file to scan for viruses.".to_string(),
                is_error: false,
            };
        }
    }

    fn render_dialogs(&mut self, ctx: &egui::Context) {
        if matches!(self.dialog_state, DialogState::None) {
            self.dialog_frames_open = 0;
            return;
        }

        self.dialog_frames_open = self.dialog_frames_open.saturating_add(1);

        let mut close_dialog = false;
        let any_popup_open = egui::Popup::is_any_open(ctx);

        // 1. ESC key closes the dialog
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
            && !any_popup_open
        {
            close_dialog = true;
        }

        // 2. Render dimmed modal backdrop behind the dialog
        let screen_rect = ctx.viewport_rect();
        let backdrop_color = if self.theme_mode == ThemeMode::Dark {
            Color32::from_black_alpha(120)
        } else {
            Color32::from_black_alpha(75)
        };
        egui::Area::new(egui::Id::unique("modal_backdrop"))
            .order(egui::Order::Middle)
            .fixed_pos(screen_rect.min)
            .interactable(false)
            .show(ctx, |ui| {
                ui.painter().rect_filled(screen_rect, 0.0, backdrop_color);
            });

        let center = ctx.viewport_rect().center();
        let accent_color = self.accent_color();
        let is_light_theme = self.theme_mode == ThemeMode::Light;
        let mut dialog_window_rect: Option<egui::Rect> = None;
        let initial_discriminant = std::mem::discriminant(&self.dialog_state);

        match &mut self.dialog_state {
            DialogState::None => {}

            DialogState::AddArchive {
                sources,
                archive_name,
                format,
                level,
                volume_size,
                password,
                encrypt_headers,
                is_solid,
                is_locked,
                comment,
            } => {
                let mut is_open = true;
                let win_resp = egui::Window::new("Add to Archive")
                    .open(&mut is_open)
                    .order(egui::Order::Foreground)
                    .pivot(egui::Align2::CENTER_CENTER)
                    .default_pos(center)
                    .resizable(false)
                    .collapsible(false)
                    .default_width(420.0)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Archive Name:");
                            ui.text_edit_singleline(archive_name);
                        });

                        ui.separator();
                        ui.label("Archive Format:");
                        ui.horizontal(|ui| {
                            ui.radio_value(format, ArchiveType::Rar5, "RAR");
                            ui.radio_value(format, ArchiveType::Zip, "ZIP");
                            ui.radio_value(format, ArchiveType::TarGz, "TAR.GZ");
                        });

                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label("Compression Level:");
                            egui::ComboBox::from_id_salt("comp_level")
                                .selected_text(format!("{:?}", level))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        level,
                                        CompressionLevel::Store,
                                        "Store (0)",
                                    );
                                    ui.selectable_value(
                                        level,
                                        CompressionLevel::Fastest,
                                        "Fastest (1)",
                                    );
                                    ui.selectable_value(level, CompressionLevel::Fast, "Fast (2)");
                                    ui.selectable_value(
                                        level,
                                        CompressionLevel::Normal,
                                        "Normal (3)",
                                    );
                                    ui.selectable_value(level, CompressionLevel::Good, "Good (4)");
                                    ui.selectable_value(level, CompressionLevel::Best, "Best (5)");
                                });
                        });

                        ui.separator();
                        ui.checkbox(is_solid, "Create solid archive");
                        ui.checkbox(is_locked, "Lock archive");

                        ui.horizontal(|ui| {
                            ui.label("Split to volumes (e.g. 10M, 100M):");
                            ui.text_edit_singleline(volume_size);
                        });

                        ui.horizontal(|ui| {
                            ui.label("Password:");
                            ui.add(egui::TextEdit::singleline(password).password(true));
                        });

                        if !password.is_empty() {
                            ui.checkbox(encrypt_headers, "Encrypt file names (header encryption)");
                        }

                        ui.separator();
                        ui.label("Archive Comment (optional):");
                        ui.text_edit_multiline(comment);

                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("OK").strong()).clicked() {
                                let opts = AddOptions {
                                    compression_level: *level,
                                    password: if password.is_empty() {
                                        None
                                    } else {
                                        Some(password.clone())
                                    },
                                    encrypt_headers: *encrypt_headers,
                                    volume_size: if volume_size.is_empty() {
                                        None
                                    } else {
                                        Some(volume_size.clone())
                                    },
                                    is_solid: *is_solid,
                                    lock_archive: *is_locked,
                                    comment: if comment.is_empty() {
                                        None
                                    } else {
                                        Some(comment.clone())
                                    },
                                };

                                let arch_path = PathBuf::from(&archive_name);
                                let res = match format {
                                    ArchiveType::Rar5 | ArchiveType::Rar4 => {
                                        RarEngine::create(&arch_path, sources, &opts)
                                    }
                                    ArchiveType::Zip => {
                                        ZipEngine::create(&arch_path, sources, &opts)
                                    }
                                    ArchiveType::TarGz => TarEngine::create(
                                        &arch_path,
                                        sources,
                                        ArchiveType::TarGz,
                                        &opts,
                                    ),
                                    _ => Err("Unsupported format".to_string()),
                                };

                                close_dialog = true;
                                if let Err(e) = res {
                                    eprintln!("Add archive error: {}", e);
                                }
                            }

                            if ui.button("Cancel").clicked() {
                                close_dialog = true;
                            }
                        });
                    });
                if let Some(r) = win_resp {
                    dialog_window_rect = Some(r.response.rect);
                }
                if !is_open {
                    close_dialog = true;
                }
            }

            DialogState::ExtractArchive {
                archive_path,
                dest_path,
                password,
                overwrite,
            } => {
                let mut is_open = true;
                let win_resp = egui::Window::new("Extraction Path and Options")
                    .open(&mut is_open)
                    .order(egui::Order::Foreground)
                    .pivot(egui::Align2::CENTER_CENTER)
                    .default_pos(center)
                    .resizable(false)
                    .collapsible(false)
                    .default_width(400.0)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Destination Path:");
                            ui.text_edit_singleline(dest_path);
                            if ui.button("Browse...").clicked()
                                && let Some(folder) = rfd::FileDialog::new().pick_folder()
                            {
                                *dest_path = folder.to_string_lossy().to_string();
                            }
                        });

                        ui.separator();
                        ui.checkbox(overwrite, "Overwrite existing files");

                        ui.horizontal(|ui| {
                            ui.label("Password (if required):");
                            ui.add(egui::TextEdit::singleline(password).password(true));
                        });

                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Extract").strong()).clicked() {
                                let arch_type = detect_archive_type(&archive_path);
                                let pwd = if password.is_empty() {
                                    None
                                } else {
                                    Some(password.as_str())
                                };
                                let dest = PathBuf::from(&dest_path);

                                let _ = match arch_type {
                                    ArchiveType::Rar5 | ArchiveType::Rar4 => {
                                        RarEngine::extract(archive_path, &dest, None, pwd)
                                    }
                                    ArchiveType::Zip => {
                                        ZipEngine::extract(archive_path, &dest, None)
                                    }
                                    ArchiveType::TarGz => TarEngine::extract(
                                        archive_path,
                                        &dest,
                                        ArchiveType::TarGz,
                                        None,
                                    ),
                                    ArchiveType::TarBz2 => TarEngine::extract(
                                        archive_path,
                                        &dest,
                                        ArchiveType::TarBz2,
                                        None,
                                    ),
                                    ArchiveType::Tar => TarEngine::extract(
                                        archive_path,
                                        &dest,
                                        ArchiveType::Tar,
                                        None,
                                    ),
                                    _ => Err("Unsupported format".to_string()),
                                };

                                close_dialog = true;
                            }

                            if ui.button("Cancel").clicked() {
                                close_dialog = true;
                            }
                        });
                    });
                if let Some(r) = win_resp {
                    dialog_window_rect = Some(r.response.rect);
                }
                if !is_open {
                    close_dialog = true;
                }
            }

            DialogState::ArchiveInfo(info) => {
                let mut is_open = true;
                let win_resp = egui::Window::new("Archive Information")
                    .open(&mut is_open)
                    .order(egui::Order::Foreground)
                    .pivot(egui::Align2::CENTER_CENTER)
                    .default_pos(center)
                    .resizable(false)
                    .collapsible(false)
                    .default_width(380.0)
                    .show(ctx, |ui| {
                        egui::Grid::new("info_grid").striped(true).show(ui, |ui| {
                            ui.label("Archive Name:");
                            ui.strong(info.path.file_name().unwrap_or_default().to_string_lossy());
                            ui.end_row();

                            ui.label("Location:");
                            ui.monospace(
                                info.path
                                    .parent()
                                    .map(|p| p.display().to_string())
                                    .unwrap_or_default(),
                            );
                            ui.end_row();

                            ui.label("Archive Format:");
                            ui.monospace(info.archive_type.name());
                            ui.end_row();

                            ui.label("Total Files:");
                            ui.monospace(info.total_files.to_string());
                            ui.end_row();

                            ui.label("Total Folders:");
                            ui.monospace(info.total_folders.to_string());
                            ui.end_row();

                            ui.label("Unpacked Size:");
                            ui.monospace(Self::format_size(info.unpacked_size));
                            ui.end_row();

                            ui.label("Packed Size:");
                            ui.monospace(Self::format_size(info.packed_size));
                            ui.end_row();

                            ui.label("Compression Ratio:");
                            ui.monospace(format!("{:.1}%", info.compression_ratio()));
                            ui.end_row();

                            ui.label("Solid Archive:");
                            ui.monospace(if info.is_solid { "Yes" } else { "No" });
                            ui.end_row();

                            ui.label("Locked:");
                            ui.monospace(if info.is_locked { "Yes" } else { "No" });
                            ui.end_row();

                            if let Some(cmt) = &info.comment {
                                ui.label("Comment:");
                                ui.label(cmt);
                                ui.end_row();
                            }
                        });

                        ui.separator();
                        if ui.button("Close").clicked() {
                            close_dialog = true;
                        }
                    });
                if let Some(r) = win_resp {
                    dialog_window_rect = Some(r.response.rect);
                }
                if !is_open {
                    close_dialog = true;
                }
            }

            DialogState::Benchmark { runner } => {
                ctx.request_repaint();
                let mut is_open = true;
                let win_resp = egui::Window::new("Benchmark and Hardware Test")
                    .open(&mut is_open)
                    .order(egui::Order::Foreground)
                    .pivot(egui::Align2::CENTER_CENTER)
                    .default_pos(center)
                    .resizable(false)
                    .collapsible(false)
                    .default_width(340.0)
                    .show(ctx, |ui| {
                        ui.heading(
                            RichText::new("Multi-threaded Compression Test").color(accent_color),
                        );
                        ui.separator();

                        let speed = runner.current_speed_kb_s();
                        let processed = runner.total_processed_mb();
                        let elapsed = runner.elapsed_seconds();

                        egui::Grid::new("bench_grid").striped(true).show(ui, |ui| {
                            ui.label("Current Speed:");
                            ui.strong(format!("{:.0} KB/s", speed));
                            ui.end_row();

                            ui.label("Processed Data:");
                            ui.monospace(format!("{:.2} MB", processed));
                            ui.end_row();

                            ui.label("Elapsed Time:");
                            ui.monospace(format!("{:.1} s", elapsed));
                            ui.end_row();

                            ui.label("Active Threads:");
                            ui.monospace(format!("{}", rayon::current_num_threads()));
                            ui.end_row();
                        });

                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("Stop & Close").clicked() {
                                runner.stop();
                                close_dialog = true;
                            }
                        });
                    });
                if let Some(r) = win_resp {
                    dialog_window_rect = Some(r.response.rect);
                }
                if !is_open {
                    runner.stop();
                    close_dialog = true;
                }
            }

            DialogState::TrialReminder => {
                let mut is_open = true;
                let win_resp = egui::Window::new("LinRAR Open Source License")
                    .open(&mut is_open)
                    .order(egui::Order::Foreground)
                    .pivot(egui::Align2::CENTER_CENTER)
                    .default_pos(center)
                    .resizable(false)
                    .collapsible(false)
                    .default_width(420.0)
                    .show(ctx, |ui| {
                        ui.label(
                            RichText::new("LinRAR - MIT License")
                                .heading()
                                .color(accent_color),
                        );
                        ui.separator();
                        ui.label(
                            "LinRAR is free and open-source software under the MIT License.\n\n\
                            Copyright (c) 2026 LinRAR Team\n\n\
                            Permission is hereby granted, free of charge, to any person obtaining a copy \
                            of this software and associated documentation files, to deal in the Software \
                            without restriction, including without limitation the rights to use, copy, modify, \
                            merge, publish, distribute, sublicense, and/or sell copies of the Software.",
                        );
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Close").strong()).clicked() {
                                close_dialog = true;
                            }
                        });
                    });
                if let Some(r) = win_resp {
                    dialog_window_rect = Some(r.response.rect);
                }
                if !is_open {
                    close_dialog = true;
                }
            }

            DialogState::About => {
                let mut is_open = true;
                let win_resp = egui::Window::new("About LinRAR")
                    .open(&mut is_open)
                    .order(egui::Order::Foreground)
                    .pivot(egui::Align2::CENTER_CENTER)
                    .default_pos(center)
                    .resizable(false)
                    .collapsible(false)
                    .default_width(380.0)
                    .show(ctx, |ui| {
                        if let Some(tex) = self.icons.as_ref().and_then(|i| i.banner_icon.as_ref())
                        {
                            ui.add(egui::Image::from_texture((
                                tex.id(),
                                egui::vec2(360.0, 156.0),
                            )));
                        } else if let Some(tex) =
                            self.icons.as_ref().and_then(|i| i.app_icon.as_ref())
                        {
                            ui.add(egui::Image::from_texture((
                                tex.id(),
                                egui::vec2(64.0, 64.0),
                            )));
                        }
                        ui.heading(RichText::new("LinRAR 0.1.0 (Rust Engine)").color(accent_color));
                        ui.label("Modern Universal Archive Manager for Linux");
                        ui.separator();
                        ui.label("• Version: 0.1.0");
                        ui.label("• License: MIT (Open Source)");
                        ui.label("• Architecture: Pure Rust 2024 + Native Engine");
                        ui.label("• UI Toolkit: eframe / egui (Wayland & X11 Native)");
                        ui.label("• Formats: RAR 5.0, RAR 4.x, ZIP, TAR.GZ, TAR.BZ2, 7z");
                        ui.separator();
                        if ui.button("OK").clicked() {
                            close_dialog = true;
                        }
                    });
                if let Some(r) = win_resp {
                    dialog_window_rect = Some(r.response.rect);
                }
                if !is_open {
                    close_dialog = true;
                }
            }

            DialogState::ScanResults(res) => {
                let mut is_open = true;
                let win_resp = egui::Window::new("Scan Archive for Viruses & Threats")
                    .open(&mut is_open)
                    .order(egui::Order::Foreground)
                    .pivot(egui::Align2::CENTER_CENTER)
                    .default_pos(center)
                    .resizable(true)
                    .collapsible(false)
                    .default_width(450.0)
                    .show(ctx, |ui| {
                        ui.heading(RichText::new(&res.archive_name).color(accent_color));
                        ui.label(format!("Scanner Engine: {}", res.scanner_name));
                        ui.label(format!("Total Files Scanned: {}", res.files_scanned));
                        ui.separator();

                        if res.clean {
                            ui.label(
                                RichText::new(
                                    "[OK] CLEAN: No viruses or suspicious threats detected!",
                                )
                                .strong()
                                .color(Color32::GREEN),
                            );
                        } else {
                            ui.label(
                                RichText::new(format!(
                                    "[!] THREATS DETECTED ({}):",
                                    res.threats.len()
                                ))
                                .strong()
                                .color(Color32::RED),
                            );
                            egui::Grid::new("threats_grid")
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.strong("Severity");
                                    ui.strong("Threat");
                                    ui.strong("File");
                                    ui.end_row();

                                    for t in &res.threats {
                                        let col = match t.severity.as_str() {
                                            "Critical" => Color32::RED,
                                            "High" => Color32::from_rgb(255, 100, 100),
                                            _ => Color32::YELLOW,
                                        };
                                        ui.label(RichText::new(&t.severity).color(col));
                                        ui.label(&t.threat_name);
                                        ui.label(&t.file);
                                        ui.end_row();
                                    }
                                });
                        }

                        ui.separator();
                        if ui.button("Close").clicked() {
                            close_dialog = true;
                        }
                    });
                if let Some(r) = win_resp {
                    dialog_window_rect = Some(r.response.rect);
                }
                if !is_open {
                    close_dialog = true;
                }
            }

            DialogState::FindFiles {
                pattern,
                search_dir,
                search_inside_archives,
                results,
                has_searched,
            } => {
                let mut do_search = false;
                let mut pick_dir = false;
                let mut open_item = None;

                let mut is_open = true;
                let win_resp = egui::Window::new("Find Files")
                    .open(&mut is_open)
                    .order(egui::Order::Foreground)
                    .pivot(egui::Align2::CENTER_CENTER)
                    .default_pos(center)
                    .resizable(true)
                    .default_size([550.0, 420.0])
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("File name to find:");
                            let resp = ui.text_edit_singleline(pattern);
                            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                do_search = true;
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("Look in:");
                            let mut dir_str = search_dir.to_string_lossy().to_string();
                            if ui.text_edit_singleline(&mut dir_str).changed() {
                                *search_dir = PathBuf::from(dir_str);
                            }
                            if ui.button("Browse...").clicked() {
                                pick_dir = true;
                            }
                        });

                        ui.checkbox(search_inside_archives, "Search inside archives");

                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Find Now").strong()).clicked() {
                                do_search = true;
                            }
                            if ui.button("Close").clicked() {
                                close_dialog = true;
                            }
                        });

                        if *has_searched {
                            ui.separator();
                            ui.label(format!("Found {} matching entries:", results.len()));
                            ScrollArea::both().max_height(240.0).show(ui, |ui| {
                                egui::Grid::new("find_results_grid")
                                    .striped(true)
                                    .show(ui, |ui| {
                                        ui.strong("Name");
                                        ui.strong("Size");
                                        ui.strong("Container");
                                        ui.strong("Path");
                                        ui.end_row();

                                        for m in results.iter() {
                                            if ui.link(&m.file_name).clicked() {
                                                open_item = Some(m.full_path.clone());
                                            }
                                            ui.label(Self::format_size(m.size));
                                            ui.label(&m.container);
                                            ui.monospace(&m.full_path);
                                            ui.end_row();
                                        }
                                    });
                            });
                        }
                    });
                if let Some(r) = win_resp {
                    dialog_window_rect = Some(r.response.rect);
                }
                if !is_open {
                    close_dialog = true;
                }

                if pick_dir && let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    *search_dir = folder;
                }

                if do_search {
                    *results = ArchiveFinder::find_in_directory(
                        &search_dir,
                        pattern,
                        *search_inside_archives,
                    );
                    *has_searched = true;
                }

                if let Some(path) = open_item {
                    let p = PathBuf::from(path);
                    if p.exists() {
                        if p.is_dir() {
                            self.navigate_local(&p);
                            close_dialog = true;
                        } else if is_supported_archive(&p) {
                            self.open_archive(&p);
                            close_dialog = true;
                        } else {
                            let _ = std::process::Command::new("xdg-open").arg(&p).spawn();
                        }
                    }
                }
            }

            DialogState::ConvertArchive {
                source_path,
                target_format,
            } => {
                let mut do_convert = false;
                let src_clone = source_path.clone();
                let fmt_clone = *target_format;

                let mut is_open = true;
                let win_resp = egui::Window::new("Convert Archives")
                    .open(&mut is_open)
                    .order(egui::Order::Foreground)
                    .pivot(egui::Align2::CENTER_CENTER)
                    .default_pos(center)
                    .resizable(false)
                    .collapsible(false)
                    .default_width(380.0)
                    .show(ctx, |ui| {
                        ui.label(format!("Source Archive: {}", source_path.display()));
                        ui.separator();
                        ui.label("Select Target Archive Format:");

                        ui.radio_value(target_format, ArchiveType::Zip, "ZIP Archive (.zip)");
                        ui.radio_value(
                            target_format,
                            ArchiveType::TarGz,
                            "GZipped TAR Archive (.tar.gz)",
                        );
                        ui.radio_value(
                            target_format,
                            ArchiveType::TarBz2,
                            "BZip2 TAR Archive (.tar.bz2)",
                        );
                        ui.radio_value(
                            target_format,
                            ArchiveType::Tar,
                            "Standard TAR Archive (.tar)",
                        );
                        ui.radio_value(target_format, ArchiveType::Rar5, "RAR 5.0 Archive (.rar)");

                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Convert").strong()).clicked() {
                                do_convert = true;
                            }
                            if ui.button("Cancel").clicked() {
                                close_dialog = true;
                            }
                        });
                    });
                if let Some(r) = win_resp {
                    dialog_window_rect = Some(r.response.rect);
                }
                if !is_open {
                    close_dialog = true;
                }

                if do_convert {
                    let opts = AddOptions {
                        compression_level: CompressionLevel::Normal,
                        password: None,
                        encrypt_headers: false,
                        volume_size: None,
                        is_solid: false,
                        lock_archive: false,
                        comment: None,
                    };
                    match ArchiveConverter::convert(&src_clone, fmt_clone, &opts) {
                        Ok(out) => {
                            self.dialog_state = DialogState::Message {
                                title: "Convert Archive".to_string(),
                                message: format!(
                                    "Archive converted successfully!\nSaved to: {}",
                                    out.display()
                                ),
                                is_error: false,
                            };
                            self.refresh_local();
                        }
                        Err(e) => {
                            self.dialog_state = DialogState::Message {
                                title: "Conversion Error".to_string(),
                                message: e,
                                is_error: true,
                            };
                        }
                    }
                }
            }

            DialogState::RepairArchive {
                archive_path,
                result,
            } => {
                let mut do_repair = false;
                let arch_clone = archive_path.clone();

                let mut is_open = true;
                let win_resp = egui::Window::new("Repair Damaged Archive")
                    .open(&mut is_open)
                    .order(egui::Order::Foreground)
                    .pivot(egui::Align2::CENTER_CENTER)
                    .default_pos(center)
                    .resizable(true)
                    .default_size([460.0, 320.0])
                    .show(ctx, |ui| {
                        ui.label(format!("Archive: {}", archive_path.display()));
                        ui.separator();

                        if let Some(res) = result {
                            if res.success {
                                ui.label(
                                    RichText::new("[OK] Repair completed!")
                                        .strong()
                                        .color(Color32::GREEN),
                                );
                                ui.label(format!("Fixed archive: {}", res.fixed_path.display()));
                            } else {
                                ui.label(
                                    RichText::new("[!] Repair finished with warnings")
                                        .color(Color32::YELLOW),
                                );
                            }
                            ui.separator();
                            ui.label("Repair Log:");
                            ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                                ui.monospace(&res.log);
                            });
                        } else {
                            ui.label("This tool attempts to reconstruct corrupt or broken archive headers and recovery records.");
                        }

                        ui.separator();
                        ui.horizontal(|ui| {
                            if result.is_none()
                                && ui.button(RichText::new("Start Repair").strong()).clicked()
                            {
                                do_repair = true;
                            }
                            if ui.button("Close").clicked() {
                                close_dialog = true;
                            }
                        });
                    });
                if let Some(r) = win_resp {
                    dialog_window_rect = Some(r.response.rect);
                }
                if !is_open {
                    close_dialog = true;
                }

                if do_repair {
                    match ArchiveRepair::repair(&arch_clone) {
                        Ok(res) => {
                            *result = Some(res);
                            self.refresh_local();
                        }
                        Err(e) => {
                            self.dialog_state = DialogState::Message {
                                title: "Repair Error".to_string(),
                                message: e,
                                is_error: true,
                            };
                        }
                    }
                }
            }

            DialogState::GenerateReport {
                archive_path,
                format,
                content,
            } => {
                let mut do_save = false;
                let mut recompute = false;
                let arch_clone = archive_path.clone();
                let fmt_val = *format;

                let mut is_open = true;
                let win_resp = egui::Window::new("Archive Contents Report")
                    .open(&mut is_open)
                    .order(egui::Order::Foreground)
                    .pivot(egui::Align2::CENTER_CENTER)
                    .default_pos(center)
                    .resizable(true)
                    .default_size([600.0, 450.0])
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Format:");
                            if ui
                                .radio_value(format, ReportFormat::Text, "Plain Text")
                                .clicked()
                            {
                                recompute = true;
                            }
                            if ui
                                .radio_value(format, ReportFormat::Html, "HTML Document")
                                .clicked()
                            {
                                recompute = true;
                            }
                            ui.separator();
                            if ui.button("Save to File...").clicked() {
                                do_save = true;
                            }
                            if ui.button("Close").clicked() {
                                close_dialog = true;
                            }
                        });

                        ui.separator();
                        ScrollArea::both().show(ui, |ui| {
                            ui.monospace(content.as_str());
                        });
                    });
                if let Some(r) = win_resp {
                    dialog_window_rect = Some(r.response.rect);
                }
                if !is_open {
                    close_dialog = true;
                }

                if recompute {
                    *content = ArchiveReport::generate(&arch_clone, *format).unwrap_or_default();
                }

                if do_save {
                    let ext = if fmt_val == ReportFormat::Html {
                        "html"
                    } else {
                        "txt"
                    };
                    if let Some(save_path) = rfd::FileDialog::new()
                        .set_file_name(format!("report.{}", ext))
                        .save_file()
                        && let Err(e) = std::fs::write(&save_path, content.as_bytes())
                    {
                        eprintln!("Save report error: {}", e);
                    }
                }
            }

            DialogState::ProtectArchive {
                archive_path,
                percent,
            } => {
                let mut do_protect = false;
                let arch_clone = archive_path.clone();
                let pct_clone = percent.clone();

                let mut is_open = true;
                let win_resp = egui::Window::new("Protect Archive (Recovery Record)")
                    .open(&mut is_open)
                    .order(egui::Order::Foreground)
                    .pivot(egui::Align2::CENTER_CENTER)
                    .default_pos(center)
                    .resizable(false)
                    .collapsible(false)
                    .default_width(340.0)
                    .show(ctx, |ui| {
                        ui.label(format!(
                            "Archive: {}",
                            archive_path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                        ));
                        ui.separator();
                        ui.label("Add data recovery record to repair damaged archives.");
                        ui.horizontal(|ui| {
                            ui.label("Recovery record size:");
                            ui.text_edit_singleline(percent);
                            ui.label("(e.g. 3%, 5%, 10%)");
                        });

                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui
                                .button(RichText::new("Add Protection").strong())
                                .clicked()
                            {
                                do_protect = true;
                            }
                            if ui.button("Cancel").clicked() {
                                close_dialog = true;
                            }
                        });
                    });
                if let Some(r) = win_resp {
                    dialog_window_rect = Some(r.response.rect);
                }
                if !is_open {
                    close_dialog = true;
                }

                if do_protect {
                    match RarEngine::protect(&arch_clone, &pct_clone) {
                        Ok(_) => {
                            self.dialog_state = DialogState::Message {
                                title: "Protect Archive".to_string(),
                                message: format!(
                                    "Recovery record ({}) added successfully!",
                                    pct_clone
                                ),
                                is_error: false,
                            };
                            if let Some(cur) = self.current_archive.clone() {
                                self.open_archive(&cur);
                            }
                        }
                        Err(e) => {
                            self.dialog_state = DialogState::Message {
                                title: "Protect Archive Error".to_string(),
                                message: e,
                                is_error: true,
                            };
                        }
                    }
                }
            }

            DialogState::RenameEntry {
                archive_path,
                old_name,
                new_name,
            } => {
                let mut do_rename = false;
                let arch_clone = archive_path.clone();
                let old_clone = old_name.clone();
                let new_clone = new_name.clone();

                let mut is_open = true;
                let win_resp = egui::Window::new("Rename Archived File")
                    .open(&mut is_open)
                    .order(egui::Order::Foreground)
                    .pivot(egui::Align2::CENTER_CENTER)
                    .default_pos(center)
                    .resizable(false)
                    .collapsible(false)
                    .default_width(340.0)
                    .show(ctx, |ui| {
                        ui.label(format!("Current Name: {}", old_name));
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label("New Name:");
                            ui.text_edit_singleline(new_name);
                        });

                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Rename").strong()).clicked() {
                                do_rename = true;
                            }
                            if ui.button("Cancel").clicked() {
                                close_dialog = true;
                            }
                        });
                    });
                if let Some(r) = win_resp {
                    dialog_window_rect = Some(r.response.rect);
                }
                if !is_open {
                    close_dialog = true;
                }

                if do_rename {
                    match RarEngine::rename(&arch_clone, &old_clone, &new_clone) {
                        Ok(_) => {
                            close_dialog = true;
                            self.open_archive(&arch_clone);
                        }
                        Err(e) => {
                            self.dialog_state = DialogState::Message {
                                title: "Rename Error".to_string(),
                                message: e,
                                is_error: true,
                            };
                        }
                    }
                }
            }

            DialogState::Message {
                title,
                message,
                is_error,
            } => {
                let text_color = if *is_error {
                    Color32::RED
                } else if is_light_theme {
                    Color32::from_rgb(30, 30, 30)
                } else {
                    Color32::LIGHT_GRAY
                };
                let mut is_open = true;
                let win_resp = egui::Window::new(title.as_str())
                    .open(&mut is_open)
                    .order(egui::Order::Foreground)
                    .pivot(egui::Align2::CENTER_CENTER)
                    .default_pos(center)
                    .resizable(false)
                    .collapsible(false)
                    .default_width(320.0)
                    .show(ctx, |ui| {
                        ui.label(RichText::new(message.as_str()).color(text_color));
                        ui.separator();
                        if ui.button("OK").clicked() {
                            close_dialog = true;
                        }
                    });
                if let Some(r) = win_resp {
                    dialog_window_rect = Some(r.response.rect);
                }
                if !is_open {
                    close_dialog = true;
                }
            }
        }

        // 3. Click outside the dialog window closes the dialog
        if let Some(rect) = dialog_window_rect
            && self.dialog_frames_open > 1
            && ctx.input(|i| i.pointer.primary_clicked())
            && !any_popup_open
            && let Some(pos) = ctx.input(|i| i.pointer.interact_pos())
            && !rect.contains(pos)
        {
            close_dialog = true;
        }

        let final_discriminant = std::mem::discriminant(&self.dialog_state);
        if initial_discriminant != final_discriminant {
            self.dialog_frames_open = 0;
        }

        if close_dialog {
            if let DialogState::Benchmark { runner } = &mut self.dialog_state {
                runner.stop();
            }
            self.dialog_state = DialogState::None;
            self.dialog_frames_open = 0;
        }
    }
}
