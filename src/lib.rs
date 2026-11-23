pub mod reader;
pub mod report;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
/// An object identified by its metadata kind and identifier, not by its display name.
pub struct MetadataObject {
    pub kind: u32,
    #[serde(rename = "id")]
    pub identifier: u32,
    pub name: String,
    pub synonym: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
/// Explicit object permissions; identifier zero denotes the default for this object kind.
pub struct PermissionBlock {
    pub kind: u32,
    #[serde(rename = "id")]
    pub identifier: u32,
    /// Each pair contains an action identifier and its stored permission value (0 or 1).
    pub properties: Vec<(u32, u32)>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PermissionSet {
    pub name: String,
    pub description: String,
    pub stream: String,
    pub blocks: Vec<PermissionBlock>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatabaseUser {
    pub name: String,
    pub full_name: String,
    pub role: String,
    pub interface: String,
    pub directory: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
/// The information extracted from source files, also used as the portable JSON snapshot.
pub struct InformationDatabase {
    pub source: String,
    pub title: String,
    pub objects: Vec<MetadataObject>,
    pub roles: Vec<PermissionSet>,
    pub users: Vec<DatabaseUser>,
    pub warnings: Vec<String>,
    #[serde(rename = "md_bytes")]
    pub metadata_file_bytes: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessDecision {
    Allow,
    Deny,
    Unknown,
    NotApplicable,
}
impl AccessDecision {
    pub fn label(self) -> &'static str {
        match self {
            Self::Allow => "Разрешено",
            Self::Deny => "Запрещено",
            Self::Unknown => "Не определено",
            Self::NotApplicable => "—",
        }
    }
}
impl PermissionSet {
    pub fn access(&self, object: &MetadataObject, action: u32) -> (AccessDecision, String) {
        if !actions(object.kind).contains(&action)
            && !self.blocks.iter().any(|permission_block| {
                permission_block.kind == object.kind
                    && permission_block
                        .properties
                        .iter()
                        .any(|property| property.0 == action)
            })
        {
            return (
                AccessDecision::NotApplicable,
                "Действие не относится к этому типу объекта".into(),
            );
        }
        let explicit_block = self.blocks.iter().rev().find(|permission_block| {
            permission_block.kind == object.kind && permission_block.identifier == object.identifier
        });
        let default_block = self.blocks.iter().rev().find(|permission_block| {
            permission_block.kind == object.kind && permission_block.identifier == 0
        });
        for (block, origin) in [
            (explicit_block, "Явное право объекта"),
            (default_block, "Общее право типа (ID=0)"),
        ] {
            if let Some(permission_block) = block
                && let Some((_, value)) = permission_block
                    .properties
                    .iter()
                    .rev()
                    .find(|(action_identifier, _)| *action_identifier == action)
            {
                return (
                    match value {
                        0 => AccessDecision::Deny,
                        1 => AccessDecision::Allow,
                        _ => AccessDecision::Unknown,
                    },
                    format!(
                        "{origin}; тип 0x{:02X}, объект {}, действие 0x{action:02X}; {}",
                        permission_block.kind, permission_block.identifier, self.stream
                    ),
                );
            }
        }
        (
            AccessDecision::Unknown,
            "В файле нет распознанного значения для этого действия и общего правила".into(),
        )
    }
}
pub fn kind_name(kind: u32) -> String {
    match kind {
        1 => "Константа",
        3 => "Справочник",
        6 => "Регистр",
        9 => "Документ",
        11 => "Журнал документов",
        16 => "Отчёт",
        17 => "Конфигурация",
        18 => "Календарь",
        23 => "Журнал расчётов",
        24 => "Обработка",
        25 => "План счетов",
        26 => "Операция",
        27 => "Проводка",
        28 => "Типовая операция",
        _ => return format!("Тип 0x{kind:02X}"),
    }
    .into()
}
pub fn action_name(action: u32) -> String {
    match action {
        1 => "Чтение",
        2 => "Использование",
        3 => "Любые изменения",
        4 => "Корректировка (редактирование)",
        5 => "Ввод нового",
        6 => "Удаление",
        7 => "Изменение проведённых документов",
        8 => "Проведение документа",
        9 => "Проведение задним числом",
        10 => "Управление оперативными итогами",
        11 => "Административные функции",
        12 => "Просмотр полного журнала",
        13 => "Просмотр журнала «Прочие»",
        14 => "Выбор",
        15 => "Просмотр подчинённых документов",
        16 => "Монитор",
        17 => "Изменение без перепроведения",
        18 => "OLE Automation сервер",
        19 => "Расчёт",
        20 => "Изменение расчётного периода",
        25 => "Удаление помеченных объектов",
        26 => "Пометка на удаление",
        27 => "Любые внешние отчёты и обработки",
        28 => "Снятие пометки на удаление",
        29 => "Функции в табло и калькуляторе",
        30 => "Поиск ссылок на объекты",
        31 => "Общие внешние отчёты и обработки",
        32 => "Ввод операции без проверки проводок",
        33 => "Редактирование списка корректных проводок",
        34 => "Табло счетов",
        35 => "Управление бухгалтерскими итогами",
        36 => "Просмотр списка",
        37 => "Редактирование операции документа",
        38 => "Просмотр списка корректных проводок",
        39 => "Выключение проводок операции",
        40 => "Включение проводок операции",
        41 => "Групповое проведение документов",
        42 => "Сохранение / выгрузка данных",
        43 => "Монопольный режим",
        44 => "Автообмен в распределённой ИБ",
        45 => "Редактирование праздников",
        46 => "Использование Web-компоненты",
        _ => return format!("Действие 0x{action:02X}"),
    }
    .into()
}
pub fn actions(kind: u32) -> Vec<u32> {
    match kind {
        1 => vec![1, 4],
        3 => vec![1, 3, 4, 5, 6, 26, 28],
        6 => vec![1],
        9 => vec![1, 3, 4, 5, 6, 7, 8, 9, 14, 15, 17, 26, 28],
        11 => vec![1, 12, 13],
        16 | 24 => vec![2],
        17 => vec![
            11, 42, 10, 35, 16, 18, 25, 30, 27, 31, 29, 41, 34, 43, 44, 46,
        ],
        23 => vec![1, 3, 19, 20],
        18 => vec![1, 3, 45],
        25 => vec![1, 3, 4, 5, 6, 26, 28],
        26 => vec![1, 36, 32, 39, 40],
        27 => vec![36, 38, 33],
        28 => vec![1, 2, 3, 4, 5, 6],
        _ => vec![],
    }
}
impl InformationDatabase {
    pub fn object_actions(&self, kind: u32) -> Vec<u32> {
        let mut available_actions: std::collections::BTreeSet<_> =
            actions(kind).into_iter().collect();
        for permission_set in &self.roles {
            for permission_block in &permission_set.blocks {
                if permission_block.kind == kind {
                    available_actions.extend(
                        permission_block
                            .properties
                            .iter()
                            .map(|property| property.0),
                    );
                }
            }
        }
        available_actions.into_iter().collect()
    }
}
