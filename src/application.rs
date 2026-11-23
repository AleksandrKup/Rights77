use eframe::egui::{self, Color32, RichText};
use rights77::{AccessDecision, InformationDatabase, action_name, kind_name, reader, report};
use std::{path::PathBuf, sync::mpsc};

const GREEN: Color32 = Color32::from_rgb(91, 213, 163);
const AMBER: Color32 = Color32::from_rgb(239, 190, 100);
const MUTED: Color32 = Color32::from_rgb(149, 162, 181);
const BLUE: Color32 = Color32::from_rgb(116, 174, 255);
#[derive(Clone)]
struct TableCell {
    text: String,
    color: Color32,
    hint: String,
}
impl TableCell {
    fn text(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            hint: text.clone(),
            text,
            color: Color32::from_rgb(222, 230, 242),
        }
    }
    fn access(a: AccessDecision, hint: String) -> Self {
        Self {
            text: a.label().into(),
            color: match a {
                AccessDecision::Allow => GREEN,
                AccessDecision::Deny => Color32::from_rgb(243, 133, 143),
                _ => AMBER,
            },
            hint,
        }
    }
}
fn render_table(
    user_interface: &mut egui::Ui,
    table_identifier: &str,
    columns: &[(&str, f32)],
    rows: &[Vec<TableCell>],
    selected_row: Option<usize>,
    height: f32,
) -> Option<usize> {
    let mut clicked_row = None;
    egui::ScrollArea::horizontal()
        .id_salt(format!("{table_identifier}-horizontal"))
        .show(user_interface, |user_interface| {
            let width = user_interface.available_width().max(880.);
            let total_column_weight: f32 = columns.iter().map(|column| column.1).sum();
            let column_widths: Vec<_> = columns
                .iter()
                .map(|column| width * column.1 / total_column_weight)
                .collect();
            user_interface.set_min_width(width);
            user_interface.scope(|user_interface| {
                user_interface.spacing_mut().item_spacing = egui::Vec2::ZERO;
                user_interface.horizontal(|user_interface| {
                    for ((label, _), column_width) in columns.iter().zip(&column_widths) {
                        user_interface.allocate_ui_with_layout(
                            egui::vec2(*column_width, 34.),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |user_interface| {
                                user_interface.set_min_size(egui::vec2(*column_width, 34.));
                                user_interface.add_space(10.);
                                user_interface.add(
                                    egui::Label::new(RichText::new(*label).strong().color(MUTED))
                                        .halign(egui::Align::Min)
                                        .truncate(),
                                );
                            },
                        );
                    }
                });
                egui::ScrollArea::vertical()
                    .id_salt(table_identifier)
                    .max_height(height.max(90.))
                    .auto_shrink([false, false])
                    .show_rows(user_interface, 34., rows.len(), |user_interface, range| {
                        for index in range {
                            let rect = egui::Rect::from_min_size(
                                user_interface.next_widget_position(),
                                egui::vec2(width, 34.),
                            );
                            let fill = if selected_row == Some(index) {
                                Color32::from_rgb(40, 69, 99)
                            } else if index % 2 == 0 {
                                Color32::from_rgb(29, 37, 50)
                            } else {
                                Color32::from_rgb(24, 31, 43)
                            };
                            user_interface.painter().rect_filled(rect, 0., fill);
                            user_interface.horizontal(|user_interface| {
                                for (cell, column_width) in rows[index].iter().zip(&column_widths) {
                                    user_interface.allocate_ui_with_layout(
                                        egui::vec2(*column_width, 34.),
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        |user_interface| {
                                            user_interface
                                                .set_min_size(egui::vec2(*column_width, 34.));
                                            user_interface.add_space(10.);
                                            let response = user_interface
                                                .add(
                                                    egui::Label::new(
                                                        RichText::new(&cell.text).color(cell.color),
                                                    )
                                                    .halign(egui::Align::Min)
                                                    .truncate()
                                                    .sense(egui::Sense::click()),
                                                )
                                                .on_hover_text(&cell.hint);
                                            if response.clicked() {
                                                clicked_row = Some(index);
                                            }
                                        },
                                    );
                                }
                            });
                        }
                    });
            });
        });
    clicked_row
}
fn display_path(path: &str) -> String {
    if let Some(network_path) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{network_path}")
    } else {
        path.strip_prefix(r"\\?\").unwrap_or(path).into()
    }
}

fn settings_file_path() -> PathBuf {
    std::env::current_exe()
        .expect("Не удалось определить путь к приложению")
        .with_file_name("DataRights77")
        .join("last-path.txt")
}
fn contains(value: &str, query: &str) -> bool {
    query.is_empty() || value.to_lowercase().contains(&query.to_lowercase())
}
fn button(user_interface: &mut egui::Ui, text: &str) -> bool {
    user_interface
        .add_sized([180.0, 32.0], egui::Button::new(text))
        .clicked()
}

pub struct RightsApplication {
    logo: egui::TextureHandle,
    path: String,
    database: Option<InformationDatabase>,
    pending_database_load: Option<mpsc::Receiver<Result<InformationDatabase, String>>>,
    selected_tab: usize,
    search_query: String,
    selected_role: usize,
    selected_object: usize,
    selected_user: usize,
    action: u32,
    kind: u32,
    only_allowed: bool,
    left_role_index: usize,
    right_role_index: usize,
    only_differences: bool,
    baseline: Option<InformationDatabase>,
    status: String,
    error: Option<String>,
    render_verification: Option<(PathBuf, usize)>,
}
impl RightsApplication {
    pub fn new(
        creation_context: &eframe::CreationContext<'_>,
        initial_database_path: Option<String>,
    ) -> Self {
        creation_context.egui_ctx.set_visuals(egui::Visuals::dark());
        let mut fonts = egui::FontDefinitions::default();
        if let Ok(bytes) = std::fs::read("C:/Windows/Fonts/segoeui.ttf") {
            fonts
                .font_data
                .insert("segoe".into(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "segoe".into());
        }
        creation_context.egui_ctx.set_fonts(fonts);
        creation_context
            .egui_ctx
            .style_mut_of(egui::Theme::Dark, |style| {
                style.visuals.override_text_color = Some(Color32::from_rgb(222, 230, 242));
                style.spacing.item_spacing = egui::vec2(10., 9.);
                style.spacing.button_padding = egui::vec2(12., 7.);
                style.visuals.panel_fill = Color32::from_rgb(22, 28, 39);
                style.visuals.window_fill = Color32::from_rgb(28, 36, 49);
                style.visuals.selection.bg_fill = Color32::from_rgb(45, 77, 116);
            });
        let last_database_path = std::fs::read_to_string(settings_file_path()).ok();
        let bundled_database_path = std::env::current_exe()
            .ok()
            .and_then(|directory| directory.parent().map(|directory| directory.join("Demo")))
            .filter(|directory| directory.join("1cv7.md").exists())
            .map(|directory| directory.to_string_lossy().into_owned());
        let path = initial_database_path
            .or(last_database_path)
            .or(bundled_database_path)
            .unwrap_or_else(|| "C:\\InfoBase7\\DemoDB".into());
        let icon = image::load_from_memory(include_bytes!("../assets/rights77.png"))
            .expect("icon")
            .to_rgba8();
        let logo = creation_context.egui_ctx.load_texture(
            "rights77",
            egui::ColorImage::from_rgba_unmultiplied(
                [icon.width() as usize, icon.height() as usize],
                icon.as_raw(),
            ),
            Default::default(),
        );
        let mut application = Self {
            logo,
            path,
            database: None,
            pending_database_load: None,
            selected_tab: 0,
            search_query: String::new(),
            selected_role: 0,
            selected_object: 0,
            selected_user: 0,
            action: 4,
            kind: 3,
            only_allowed: false,
            left_role_index: 0,
            right_role_index: 1,
            only_differences: true,
            baseline: None,
            status: "Выберите папку информационной базы или файл 1CV7.MD".into(),
            error: None,
            render_verification: std::env::var_os("RIGHTS77_RENDER_DIR")
                .map(|directory| (PathBuf::from(directory), 0)),
        };
        if std::path::Path::new(&application.path).exists() {
            application.request_database_load(creation_context.egui_ctx.clone());
        }
        application
    }
    fn request_database_load(&mut self, context: egui::Context) {
        if self.pending_database_load.is_some() {
            return;
        }
        let database_path = PathBuf::from(self.path.trim());
        let (result_sender, result_receiver) = mpsc::channel();
        self.pending_database_load = Some(result_receiver);
        self.error = None;
        self.status = "Чтение конфигурации и пользователей…".into();
        std::thread::spawn(move || {
            let result = reader::load(&database_path);
            let _ = result_sender.send(result);
            context.request_repaint();
        });
    }
    fn poll_database_load(&mut self) {
        let result = self
            .pending_database_load
            .as_ref()
            .and_then(|result_receiver| result_receiver.try_recv().ok());
        if let Some(result) = result {
            self.pending_database_load = None;
            match result {
                Ok(database) => {
                    self.status = format!(
                        "Прочитано: {} объектов · {} наборов · {} пользователей",
                        database.objects.len(),
                        database.roles.len(),
                        database.users.len()
                    );
                    self.path = display_path(
                        &std::path::Path::new(&database.source)
                            .parent()
                            .unwrap()
                            .to_string_lossy(),
                    );
                    let settings_path = settings_file_path();
                    if let Err(error) = std::fs::create_dir_all(
                        settings_path.parent().expect("DataRights77 directory"),
                    )
                    .and_then(|()| std::fs::write(&settings_path, &self.path))
                    {
                        self.error = Some(format!(
                            "База прочитана, но не удалось сохранить настройки в {}: {error}",
                            settings_path.display()
                        ));
                    }
                    self.selected_object = database
                        .objects
                        .iter()
                        .position(|metadata_object| {
                            metadata_object.kind == 3 && metadata_object.name == "Номенклатура"
                        })
                        .unwrap_or(0);
                    self.selected_role = 0;
                    self.selected_user = 0;
                    if self.render_verification.is_some()
                        && let Ok(name) = std::env::var("RIGHTS77_QA_USER")
                    {
                        self.selected_user = database
                            .users
                            .iter()
                            .position(|user| user.name.contains(&name))
                            .unwrap_or(0);
                    }
                    self.left_role_index = 0;
                    self.right_role_index = usize::from(database.roles.len() > 1);
                    self.database = Some(database);
                }
                Err(error) => {
                    self.status =
                        "Загрузка не выполнена. Предыдущий результат сохранён на экране.".into();
                    self.error = Some(error);
                }
            }
        }
    }
    fn export(&mut self, export_json: bool) {
        let Some(database) = &self.database else {
            return;
        };
        let extension = if export_json { "json" } else { "csv" };
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(extension, &[extension])
            .set_file_name(format!("rights77.{extension}"))
            .save_file()
        {
            match report::save_export(database, &path, export_json) {
                Ok(()) => self.status = format!("Сохранено: {}", path.display()),
                Err(error) => self.error = Some(error.to_string()),
            }
        }
    }
    fn header(&mut self, user_interface: &mut egui::Ui, context: &egui::Context) {
        user_interface.horizontal(|user_interface| {
            user_interface
                .add(egui::Image::new(&self.logo).fit_to_exact_size(egui::vec2(48., 48.)));
            user_interface.vertical(|user_interface| {
                user_interface.label(RichText::new("Права 1С").size(23.).strong());
                user_interface.label(
                    RichText::new("Пользователи · Наборы прав · Объекты")
                        .small()
                        .color(MUTED),
                );
            });
            user_interface.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |user_interface| {
                    user_interface.label(RichText::new("ТОЛЬКО ЧТЕНИЕ").color(GREEN));
                },
            );
        });

        user_interface.add_space(10.);
        user_interface.horizontal(|user_interface| {
            user_interface.add(
                egui::TextEdit::singleline(&mut self.path)
                    .desired_width((user_interface.available_width() - 350.).max(180.))
                    .hint_text("Папка базы или 1CV7.MD"),
            );
            if user_interface.button("Папка…").clicked()
                && let Some(selected_path) = rfd::FileDialog::new().pick_folder()
            {
                self.path = selected_path.to_string_lossy().into();
                self.request_database_load(context.clone());
            }
            if user_interface.button("Файл…").clicked()
                && let Some(selected_path) = rfd::FileDialog::new()
                    .add_filter("Конфигурация 7.7", &["md"])
                    .pick_file()
            {
                self.path = selected_path.to_string_lossy().into();
                self.request_database_load(context.clone());
            }
            if user_interface
                .add_enabled(
                    self.pending_database_load.is_none(),
                    egui::Button::new("Прочитать / обновить"),
                )
                .clicked()
            {
                self.request_database_load(context.clone());
            }
        });
        user_interface.horizontal(|user_interface| {
            for (tab_index, label) in [
                "Обзор",
                "Кто имеет доступ",
                "Наборы прав",
                "Пользователи",
                "Сравнение",
                "Диагностика",
            ]
            .iter()
            .enumerate()
            {
                if user_interface
                    .selectable_label(self.selected_tab == tab_index, *label)
                    .clicked()
                {
                    self.selected_tab = tab_index;
                    self.search_query.clear();
                }
            }
        });
        user_interface.separator();
    }
    fn overview(&mut self, user_interface: &mut egui::Ui) {
        let Some(database) = &self.database else {
            user_interface.add_space(60.);
            user_interface.heading("Узнайте, кому доступно действие");
            user_interface.label("Откройте папку базы 1С 7.7 или файл конфигурации. Программа читает MD и находящийся рядом USRDEF\\USERS.USR.");
            return;
        };
        user_interface.heading(&database.title);
        user_interface.label(RichText::new(display_path(&database.source)).color(MUTED));
        user_interface.add_space(18.);
        user_interface.horizontal(|user_interface| {
            for (item_count, label) in [
                (database.objects.len(), "объектов"),
                (database.roles.len(), "наборов прав"),
                (database.users.len(), "пользователей"),
            ] {
                egui::Frame::group(user_interface.style())
                    .inner_margin(18.)
                    .show(user_interface, |user_interface| {
                        user_interface.set_min_width(185.);
                        user_interface
                            .label(RichText::new(item_count.to_string()).size(34.).color(BLUE));
                        user_interface.label(label);
                    });
            }
        });
        user_interface.add_space(22.);
        user_interface.heading("Быстрая проверка");
        user_interface.label(
            "Выберите справочник и действие — получите наборы прав и пользователей с доступом.",
        );
        if button(user_interface, "Кто может изменять?") {
            self.selected_tab = 1;
        }
        user_interface.add_space(18.);
        user_interface.heading("Экспорт и снимки");
        user_interface.label("CSV содержит матрицу разрешений. JSON сохраняет результат анализа для сравнения позже.");
        user_interface.horizontal(|user_interface| {
            if button(user_interface, "Экспорт CSV") {
                self.export(false);
            }
            if button(user_interface, "Сохранить снимок") {
                self.export(true);
            }
        });
        user_interface.add_space(18.);
        user_interface.label(RichText::new("Анализируются права платформы. Дополнительные ограничения в коде конфигурации здесь не вычисляются.").color(AMBER));
        if let Some(database) = &self.database {
            for warning in &database.warnings {
                user_interface.label(RichText::new(warning).color(AMBER));
            }
        }
    }
    fn object_picker(&mut self, user_interface: &mut egui::Ui) {
        let Some(database) = &self.database else {
            return;
        };
        user_interface.horizontal(|user_interface| {
            egui::ComboBox::from_id_salt("kind")
                .selected_text(if self.kind == 0 {
                    "Все типы".into()
                } else {
                    kind_name(self.kind)
                })
                .width(185.)
                .show_ui(user_interface, |user_interface| {
                    if user_interface
                        .selectable_value(&mut self.kind, 0, "Все типы")
                        .clicked()
                    {
                        self.search_query.clear();
                    }
                    let kinds: std::collections::BTreeSet<_> = database
                        .objects
                        .iter()
                        .map(|metadata_object| metadata_object.kind)
                        .collect();
                    for object_kind in kinds {
                        user_interface.selectable_value(
                            &mut self.kind,
                            object_kind,
                            kind_name(object_kind),
                        );
                    }
                });
            user_interface.add(
                egui::TextEdit::singleline(&mut self.search_query)
                    .hint_text("Найти объект по имени, синониму или ID")
                    .desired_width(360.),
            );
        });
        let matching_object_indices: Vec<_> = database
            .objects
            .iter()
            .enumerate()
            .filter(|(_, metadata_object)| {
                (self.kind == 0 || metadata_object.kind == self.kind)
                    && (contains(&metadata_object.name, &self.search_query)
                        || contains(&metadata_object.synonym, &self.search_query)
                        || contains(&metadata_object.identifier.to_string(), &self.search_query))
            })
            .map(|(object_index, _)| object_index)
            .collect();
        if !matching_object_indices.contains(&self.selected_object) {
            self.selected_object = matching_object_indices
                .first()
                .copied()
                .unwrap_or(usize::MAX);
        }
        egui::ComboBox::from_id_salt("object")
            .width(620.)
            .height(350.)
            .selected_text(
                database
                    .objects
                    .get(self.selected_object)
                    .map(|metadata_object| {
                        format!(
                            "{} · {}",
                            kind_name(metadata_object.kind),
                            metadata_object.name
                        )
                    })
                    .unwrap_or_else(|| "Объекты не найдены".into()),
            )
            .show_ui(user_interface, |user_interface| {
                for object_index in &matching_object_indices {
                    let metadata_object = &database.objects[*object_index];
                    user_interface.selectable_value(
                        &mut self.selected_object,
                        *object_index,
                        format!(
                            "{} · {}  #{}",
                            kind_name(metadata_object.kind),
                            metadata_object.name,
                            metadata_object.identifier
                        ),
                    );
                }
            });
        user_interface.label(
            RichText::new(format!(
                "Найдено объектов: {}",
                matching_object_indices.len()
            ))
            .small()
            .color(MUTED),
        );
    }
    fn access_overview(&mut self, user_interface: &mut egui::Ui) {
        user_interface.heading("Кто имеет доступ");
        self.object_picker(user_interface);
        let Some(database) = &self.database else {
            return;
        };
        let Some(metadata_object) = database.objects.get(self.selected_object) else {
            return;
        };
        let available_actions = database.object_actions(metadata_object.kind);
        if !available_actions.contains(&self.action) {
            self.action = available_actions.first().copied().unwrap_or(1);
        }
        user_interface.horizontal(|user_interface| {
            egui::ComboBox::from_id_salt("action")
                .width(300.)
                .selected_text(action_name(self.action))
                .show_ui(user_interface, |user_interface| {
                    for action_identifier in available_actions {
                        user_interface.selectable_value(
                            &mut self.action,
                            action_identifier,
                            action_name(action_identifier),
                        );
                    }
                });
            user_interface.checkbox(&mut self.only_allowed, "Только разрешённые");
        });

        user_interface.add_space(10.);
        user_interface.label(
            RichText::new(format!(
                "{} → {} → {}",
                kind_name(metadata_object.kind),
                metadata_object.name,
                action_name(self.action)
            ))
            .size(18.)
            .color(BLUE),
        );
        user_interface.label("Наведите на результат, чтобы увидеть исходное правило и поток MD.");
        user_interface.separator();
        let mut rows = Vec::new();
        for role in &database.roles {
            let (access_decision, source) = role.access(metadata_object, self.action);
            if self.only_allowed && access_decision != AccessDecision::Allow {
                continue;
            }
            let assigned_user_names = database
                .users
                .iter()
                .filter(|user| user.role == role.name)
                .map(|user| user.name.as_str())
                .collect::<Vec<_>>();
            rows.push(vec![
                TableCell::text(&role.name),
                TableCell::access(access_decision, source),
                TableCell::text(if assigned_user_names.is_empty() {
                    "Нет назначенных пользователей".into()
                } else {
                    assigned_user_names.join(", ")
                }),
            ]);
        }
        render_table(
            user_interface,
            "access-table",
            &[
                ("Набор прав", 0.25),
                ("Доступ", 0.18),
                ("Пользователи", 0.57),
            ],
            &rows,
            None,
            user_interface.available_height() - 4.,
        );
    }
    fn roles(&mut self, user_interface: &mut egui::Ui) {
        user_interface.heading("Содержание наборов прав");
        let Some(database) = &self.database else {
            return;
        };
        egui::ComboBox::from_id_salt("role")
            .width(300.)
            .selected_text(
                database
                    .roles
                    .get(self.selected_role)
                    .map(|permission_set| permission_set.name.as_str())
                    .unwrap_or("Нет наборов"),
            )
            .show_ui(user_interface, |user_interface| {
                for (role_index, permission_set) in database.roles.iter().enumerate() {
                    user_interface.selectable_value(
                        &mut self.selected_role,
                        role_index,
                        &permission_set.name,
                    );
                }
            });
        let Some(role) = database.roles.get(self.selected_role) else {
            return;
        };
        user_interface.label(&role.description);
        user_interface.label(
            RichText::new(format!("{} блоков · {}", role.blocks.len(), role.stream)).color(MUTED),
        );
        user_interface.horizontal(|user_interface| {
            user_interface.add(
                egui::TextEdit::singleline(&mut self.search_query)
                    .hint_text("Поиск объекта или действия")
                    .desired_width(420.),
            );
            user_interface.checkbox(&mut self.only_allowed, "Только разрешённые");
        });
        let rows: Vec<_> = database
            .objects
            .iter()
            .flat_map(|metadata_object| {
                database
                    .object_actions(metadata_object.kind)
                    .into_iter()
                    .map(move |action_identifier| (metadata_object, action_identifier))
            })
            .filter(|(metadata_object, action_identifier)| {
                contains(&metadata_object.name, &self.search_query)
                    || contains(&action_name(*action_identifier), &self.search_query)
                    || contains(&metadata_object.synonym, &self.search_query)
            })
            .filter(|(metadata_object, action_identifier)| {
                !self.only_allowed
                    || role.access(metadata_object, *action_identifier).0 == AccessDecision::Allow
            })
            .collect();
        user_interface.label(format!("Строк: {}", rows.len()));
        let cells = rows
            .iter()
            .map(|(metadata_object, action_identifier)| {
                let (access, source) = role.access(metadata_object, *action_identifier);
                vec![
                    TableCell::text(kind_name(metadata_object.kind)),
                    TableCell {
                        text: metadata_object.name.clone(),
                        color: Color32::from_rgb(222, 230, 242),
                        hint: format!(
                            "{} · ID {}",
                            metadata_object.synonym, metadata_object.identifier
                        ),
                    },
                    TableCell::text(action_name(*action_identifier)),
                    TableCell::access(access, source),
                ]
            })
            .collect::<Vec<_>>();
        render_table(
            user_interface,
            "role-rights",
            &[
                ("Тип", 0.15),
                ("Объект", 0.30),
                ("Действие", 0.37),
                ("Доступ", 0.18),
            ],
            &cells,
            None,
            user_interface.available_height() - 4.,
        );
    }
    fn users(&mut self, user_interface: &mut egui::Ui) {
        user_interface.heading("Пользователи базы");
        let Some(database) = &self.database else {
            return;
        };
        if database.users.is_empty() {
            user_interface.colored_label(AMBER, "Именованные пользователи не загружены");
            user_interface.label("Откройте папку базы с соответствующим файлом USRDEF\\USERS.USR.");
            return;
        }
        user_interface.add(
            egui::TextEdit::singleline(&mut self.search_query)
                .hint_text("Поиск по имени, полному имени или набору прав")
                .desired_width(520.),
        );
        let matching_user_indices = database
            .users
            .iter()
            .enumerate()
            .filter(|(_, user)| {
                contains(
                    &format!("{} {} {}", user.name, user.full_name, user.role),
                    &self.search_query,
                )
            })
            .map(|(user_index, _)| user_index)
            .collect::<Vec<_>>();
        if !matching_user_indices.contains(&self.selected_user) {
            self.selected_user = matching_user_indices.first().copied().unwrap_or(usize::MAX);
        }
        let rows = matching_user_indices
            .iter()
            .map(|user_index| {
                let user = &database.users[*user_index];
                vec![
                    TableCell::text(&user.name),
                    TableCell::text(&user.full_name),
                    TableCell::text(&user.role),
                ]
            })
            .collect::<Vec<_>>();
        let selected = matching_user_indices
            .iter()
            .position(|user_index| *user_index == self.selected_user);
        if let Some(row) = render_table(
            user_interface,
            "users-table",
            &[
                ("Пользователь", 0.30),
                ("Полное имя", 0.43),
                ("Набор прав", 0.27),
            ],
            &rows,
            selected,
            (user_interface.available_height() * 0.28).clamp(110., 205.),
        ) {
            self.selected_user = matching_user_indices[row];
        }
        let Some(user) = database.users.get(self.selected_user) else {
            user_interface.label("Пользователи не найдены");
            return;
        };
        user_interface.add_space(12.);
        user_interface.separator();
        user_interface.heading(&user.full_name);
        user_interface.label(format!(
            "{} · {} · интерфейс: {}",
            user.name, user.role, user.interface
        ));
        if let Some(index) = database
            .roles
            .iter()
            .position(|permission_set| permission_set.name == user.role)
            && user_interface.button("Все права пользователя →").clicked()
        {
            self.selected_role = index;
            self.selected_tab = 2;
            self.search_query.clear();
        }
        user_interface.label(RichText::new("Корректировка справочников").strong());
        if let Some(role) = database
            .roles
            .iter()
            .find(|permission_set| permission_set.name == user.role)
        {
            let rows = database
                .objects
                .iter()
                .filter(|metadata_object| metadata_object.kind == 3)
                .map(|metadata_object| {
                    let (access_decision, source) = role.access(metadata_object, 4);
                    vec![
                        TableCell::text(&metadata_object.name),
                        TableCell::text(&metadata_object.synonym),
                        TableCell::access(access_decision, source),
                    ]
                })
                .collect::<Vec<_>>();
            render_table(
                user_interface,
                "user-rights",
                &[
                    ("Справочник", 0.36),
                    ("Синоним", 0.43),
                    ("Корректировка", 0.21),
                ],
                &rows,
                None,
                user_interface.available_height() - 4.,
            );
        } else {
            user_interface
                .colored_label(AMBER, "Назначенный набор не найден. Права не определены.");
        }
    }
    fn comparison(&mut self, user_interface: &mut egui::Ui) {
        user_interface.heading("Сравнение прав");
        user_interface.horizontal(|user_interface| {
            if user_interface
                .button("Открыть старый JSON-снимок…")
                .clicked()
                && let Some(snapshot_path) = rfd::FileDialog::new()
                    .add_filter("Снимок Rights77", &["json"])
                    .pick_file()
            {
                match std::fs::metadata(&snapshot_path)
                    .map_err(|error| error.to_string())
                    .and_then(|snapshot_metadata| {
                        if snapshot_metadata.len() > 64 * 1024 * 1024 {
                            Err("Снимок больше 64 МБ".into())
                        } else {
                            std::fs::read(&snapshot_path).map_err(|error| error.to_string())
                        }
                    })
                    .and_then(|snapshot_bytes| {
                        serde_json::from_slice::<InformationDatabase>(&snapshot_bytes)
                            .map_err(|error| error.to_string())
                    }) {
                    Ok(database) => {
                        self.baseline = Some(database);
                        self.right_role_index = 0;
                    }
                    Err(error) => self.error = Some(error),
                }
            }
            if self.baseline.is_some() && user_interface.button("Сбросить снимок").clicked()
            {
                self.baseline = None;
            }
        });
        let Some(database) = &self.database else {
            return;
        };
        let comparison_database = self.baseline.as_ref().unwrap_or(database);
        if self.baseline.is_some() {
            user_interface.label(
                RichText::new(format!("Справа: снимок {}", comparison_database.source))
                    .color(AMBER),
            );
        }
        user_interface.horizontal(|user_interface| {
            egui::ComboBox::from_id_salt("compare-a")
                .width(250.)
                .selected_text(
                    database
                        .roles
                        .get(self.left_role_index)
                        .map(|permission_set| permission_set.name.as_str())
                        .unwrap_or("Выберите набор"),
                )
                .show_ui(user_interface, |user_interface| {
                    for (role_index, permission_set) in database.roles.iter().enumerate() {
                        user_interface.selectable_value(
                            &mut self.left_role_index,
                            role_index,
                            &permission_set.name,
                        );
                    }
                });
            user_interface.label("↔");
            egui::ComboBox::from_id_salt("compare-b")
                .width(250.)
                .selected_text(
                    comparison_database
                        .roles
                        .get(self.right_role_index)
                        .map(|permission_set| permission_set.name.as_str())
                        .unwrap_or("Выберите набор"),
                )
                .show_ui(user_interface, |user_interface| {
                    for (role_index, permission_set) in comparison_database.roles.iter().enumerate()
                    {
                        user_interface.selectable_value(
                            &mut self.right_role_index,
                            role_index,
                            &permission_set.name,
                        );
                    }
                });
            user_interface.checkbox(&mut self.only_differences, "Только различия");
        });
        user_interface.add(
            egui::TextEdit::singleline(&mut self.search_query)
                .hint_text("Поиск объекта")
                .desired_width(400.),
        );
        let (Some(left_role), Some(right_role)) = (
            database.roles.get(self.left_role_index),
            comparison_database.roles.get(self.right_role_index),
        ) else {
            return;
        };
        let mut objects_by_identity = std::collections::BTreeMap::new();
        for metadata_object in comparison_database
            .objects
            .iter()
            .chain(database.objects.iter())
        {
            objects_by_identity.insert(
                (metadata_object.kind, metadata_object.identifier),
                metadata_object,
            );
        }
        let mut rows = Vec::new();
        for metadata_object in objects_by_identity.values() {
            if !contains(&metadata_object.name, &self.search_query) {
                continue;
            }
            for action in {
                let mut actions = database.object_actions(metadata_object.kind);
                actions.extend(comparison_database.object_actions(metadata_object.kind));
                actions.sort_unstable();
                actions.dedup();
                actions
            } {
                let left = if database.objects.iter().any(|candidate_object| {
                    candidate_object.kind == metadata_object.kind
                        && candidate_object.identifier == metadata_object.identifier
                }) {
                    left_role.access(metadata_object, action)
                } else {
                    (AccessDecision::Unknown, "Объекта нет в текущей базе".into())
                };
                let right = if comparison_database.objects.iter().any(|candidate_object| {
                    candidate_object.kind == metadata_object.kind
                        && candidate_object.identifier == metadata_object.identifier
                }) {
                    right_role.access(metadata_object, action)
                } else {
                    (AccessDecision::Unknown, "Объекта нет в снимке".into())
                };
                if !self.only_differences || left.0 != right.0 {
                    rows.push((*metadata_object, action, left, right));
                }
            }
        }
        user_interface.label(format!(
            "Строк сравнения: {} · сопоставление объектов по типу и ID",
            rows.len()
        ));
        let cells = rows
            .iter()
            .map(|(metadata_object, action, left, right)| {
                vec![
                    TableCell::text(format!(
                        "{} · {}",
                        kind_name(metadata_object.kind),
                        metadata_object.name
                    )),
                    TableCell::text(action_name(*action)),
                    TableCell::access(left.0, left.1.clone()),
                    TableCell::access(right.0, right.1.clone()),
                ]
            })
            .collect::<Vec<_>>();
        render_table(
            user_interface,
            "compare-rights",
            &[
                ("Объект", 0.35),
                ("Действие", 0.33),
                (&left_role.name, 0.16),
                (&right_role.name, 0.16),
            ],
            &cells,
            None,
            user_interface.available_height() - 4.,
        );
    }
    fn diagnostics(&self, user_interface: &mut egui::Ui) {
        user_interface.heading("Диагностика чтения");
        if let Some(database) = &self.database {
            user_interface.label(format!("Источник: {}", database.source));
            user_interface.label(format!("Размер MD: {} байт", database.metadata_file_bytes));
            user_interface.label("Контейнер: OLE Compound File · Метаданные: CP1251 · Наборы прав: бинарный формат 6");
            user_interface.separator();
            for warning in &database.warnings {
                user_interface.colored_label(AMBER, warning);
            }
            user_interface.separator();
            user_interface.label("Разрешено / запрещено: распознано значение 1 / 0. Не определено: нет подтверждённого правила или формат не распознан.");
            user_interface.label("Если явное правило отсутствует, используется общее правило того же типа с ID=0. Неизвестные значения не считаются запретом.");
            user_interface.label("Пароли и их хеши не сохраняются в модели, снимках или CSV. Исходные файлы открываются только на чтение.");
            user_interface.add_space(12.);
            user_interface.hyperlink_to(
                "Описание структуры MD и прав",
                "https://1c.alterplast.ru/1cv7md/index.html",
            );
        }
    }
}
impl eframe::App for RightsApplication {
    fn ui(&mut self, user_interface: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_database_load();
        let context = user_interface.ctx().clone();
        if let Some((output_directory, frame_count)) = &mut self.render_verification {
            let output_directory = output_directory.clone();
            let captured = context.input(|input_state| {
                input_state.events.iter().find_map(|event| {
                    if let egui::Event::Screenshot { image, .. } = event {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            });
            if let Some(captured_image) = captured {
                let bytes: Vec<_> = captured_image
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_array())
                    .collect();
                let _ = std::fs::create_dir_all(&output_directory);
                if let Err(error) = image::save_buffer(
                    output_directory.join(format!("tab-{}.png", self.selected_tab)),
                    &bytes,
                    captured_image.size[0] as u32,
                    captured_image.size[1] as u32,
                    image::ColorType::Rgba8,
                ) {
                    self.error = Some(error.to_string());
                }
                self.selected_tab += 1;
                *frame_count = 0;
                if self.selected_tab >= 6 {
                    context.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
            }
            if self.database.is_some() && self.pending_database_load.is_none() {
                *frame_count += 1;
                if *frame_count == 8 {
                    context.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                        egui::UserData::default(),
                    ));
                }
            }
            context.request_repaint_after(std::time::Duration::from_millis(60));
        }
        egui::Panel::bottom("status").show(user_interface, |user_interface| {
            user_interface.horizontal(|user_interface| {
                if self.pending_database_load.is_some() {
                    user_interface.spinner();
                }
                user_interface.label(RichText::new(&self.status).color(MUTED));
            });
        });
        egui::Panel::top("header").show(user_interface, |user_interface| {
            user_interface.add_space(8.);
            self.header(user_interface, &context);
        });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(22, 28, 39))
                    .inner_margin(16.),
            )
            .show(user_interface, |user_interface| {
                if self.pending_database_load.is_some() {
                    user_interface
                        .colored_label(BLUE, "Чтение файлов… Дождитесь обновления результата.");
                }
                if let Some(error) = self.error.clone() {
                    egui::Frame::group(user_interface.style()).show(
                        user_interface,
                        |user_interface| {
                            user_interface.colored_label(AMBER, error);
                            if user_interface.button("Закрыть сообщение").clicked()
                            {
                                self.error = None;
                            }
                        },
                    );
                }
                match self.selected_tab {
                    0 => {
                        egui::ScrollArea::vertical()
                            .show(user_interface, |user_interface| {
                                self.overview(user_interface)
                            })
                            .inner
                    }
                    1 => self.access_overview(user_interface),
                    2 => self.roles(user_interface),
                    3 => self.users(user_interface),
                    4 => self.comparison(user_interface),
                    _ => {
                        egui::ScrollArea::vertical()
                            .show(user_interface, |user_interface| {
                                self.diagnostics(user_interface)
                            })
                            .inner
                    }
                }
            });
        if self.pending_database_load.is_some() {
            context.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}
