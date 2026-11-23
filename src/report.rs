use crate::{InformationDatabase, action_name, kind_name};
pub fn save_export(
    database: &InformationDatabase,
    path: &std::path::Path,
    export_json: bool,
) -> Result<(), String> {
    let extension = if export_json { "json" } else { "csv" };
    if path
        .extension()
        .is_none_or(|file_extension| !file_extension.eq_ignore_ascii_case(extension))
    {
        return Err(format!(
            "Для этого экспорта требуется расширение .{extension}"
        ));
    }
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };
    let parent = absolute
        .parent()
        .ok_or("Не указан каталог экспорта")?
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let source = std::path::Path::new(&database.source)
        .parent()
        .ok_or("Неизвестен каталог базы")?
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let export_directory = parent.to_string_lossy().to_lowercase();
    let source_directory = source.to_string_lossy().to_lowercase();
    if export_directory == source_directory
        || export_directory.starts_with(&(source_directory + std::path::MAIN_SEPARATOR_STR))
    {
        return Err("Сохраните отчёт вне каталога исходной базы".into());
    }
    if absolute.exists()
        && std::fs::symlink_metadata(&absolute)
            .map_err(|error| error.to_string())?
            .file_type()
            .is_symlink()
    {
        return Err("Экспорт по символической ссылке не поддерживается".into());
    }
    let data = if export_json {
        serde_json::to_string_pretty(database).map_err(|error| error.to_string())?
    } else {
        generate_csv(database)
    };
    std::fs::write(absolute, data).map_err(|error| error.to_string())
}
pub fn escape_csv_cell(value: &str) -> String {
    let safe = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.into()
    };
    format!("\"{}\"", safe.replace('"', "\"\""))
}
pub fn generate_csv(database: &InformationDatabase) -> String {
    let mut csv_output =
        "\u{feff}Набор прав;Пользователи;Тип;Объект;ID;Действие;Код действия;Доступ;Источник\r\n"
            .to_owned();
    for role in &database.roles {
        let users = database
            .users
            .iter()
            .filter(|user| user.role == role.name)
            .map(|user| user.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        for metadata_object in &database.objects {
            for action in database.object_actions(metadata_object.kind) {
                let (access_decision, source) = role.access(metadata_object, action);
                let row = [
                    role.name.clone(),
                    users.clone(),
                    kind_name(metadata_object.kind),
                    metadata_object.name.clone(),
                    metadata_object.identifier.to_string(),
                    action_name(action),
                    action.to_string(),
                    access_decision.label().into(),
                    source,
                ];
                csv_output.push_str(
                    &row.iter()
                        .map(|cell_text| escape_csv_cell(cell_text))
                        .collect::<Vec<_>>()
                        .join(";"),
                );
                csv_output.push_str("\r\n");
            }
        }
    }
    csv_output
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn csv_escapes_and_neutralizes() {
        assert_eq!(escape_csv_cell("=1+1"), "\"'=1+1\"");
        assert_eq!(escape_csv_cell("a\"b"), "\"a\"\"b\"");
    }
}
