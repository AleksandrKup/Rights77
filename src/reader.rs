#[cfg(test)]
use crate::AccessDecision;
use crate::{DatabaseUser, InformationDatabase, MetadataObject, PermissionBlock, PermissionSet};
use std::{
    collections::{BTreeMap, HashSet},
    fs::File,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};
type ReadResult<Value> = std::result::Result<Value, String>;
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_STREAM_BYTES: u64 = 32 * 1024 * 1024;
const MAX_METADATA_DEPTH: usize = 128;
const MAX_METADATA_NODES: usize = 1_000_000;
const PERMISSION_SET_VERSION: u32 = 6;
const MAX_PERMISSION_BLOCKS: usize = 100_000;
const MAX_BLOCK_ACTIONS: usize = 256;

#[derive(Debug, Clone)]
pub enum MetadataNode {
    Text(String),
    List(Vec<MetadataNode>),
}
impl MetadataNode {
    pub fn text(&self) -> &str {
        if let Self::Text(text) = self {
            text
        } else {
            ""
        }
    }
    pub fn list(&self) -> &[MetadataNode] {
        if let Self::List(children) = self {
            children
        } else {
            &[]
        }
    }
}
pub fn parse_braces(input: &str) -> ReadResult<MetadataNode> {
    struct BraceParser<'input> {
        input_bytes: &'input [u8],
        position: usize,
        node_count: usize,
    }
    impl BraceParser<'_> {
        fn node(&mut self, depth: usize) -> ReadResult<MetadataNode> {
            if depth > MAX_METADATA_DEPTH || self.node_count > MAX_METADATA_NODES {
                return Err("Слишком сложная структура потока".into());
            }
            self.node_count += 1;
            while self.position < self.input_bytes.len()
                && (self.input_bytes[self.position].is_ascii_whitespace()
                    || self.input_bytes[self.position] == b',')
            {
                self.position += 1;
            }
            let next_character = *self
                .input_bytes
                .get(self.position)
                .ok_or("Неожиданный конец текста")?;
            if next_character == b'{' {
                self.position += 1;
                let mut children = Vec::new();
                loop {
                    while self.position < self.input_bytes.len()
                        && (self.input_bytes[self.position].is_ascii_whitespace()
                            || self.input_bytes[self.position] == b',')
                    {
                        self.position += 1;
                    }
                    if self.input_bytes.get(self.position) == Some(&b'}') {
                        self.position += 1;
                        break;
                    }
                    children.push(self.node(depth + 1)?);
                }
                return Ok(MetadataNode::List(children));
            }
            if next_character == b'"' {
                self.position += 1;
                let mut quoted_bytes = Vec::new();
                loop {
                    let character = *self
                        .input_bytes
                        .get(self.position)
                        .ok_or("Незакрытая строка")?;
                    self.position += 1;
                    if character == b'"' {
                        if self.input_bytes.get(self.position) == Some(&b'"') {
                            self.position += 1;
                            quoted_bytes.push(character);
                        } else {
                            break;
                        }
                    } else {
                        quoted_bytes.push(character);
                    }
                }
                return String::from_utf8(quoted_bytes)
                    .map(MetadataNode::Text)
                    .map_err(|_| "Неверный UTF-8".into());
            }
            let start = self.position;
            while self.position < self.input_bytes.len()
                && !b",{}\r\n".contains(&self.input_bytes[self.position])
            {
                self.position += 1;
            }
            if start == self.position {
                return Err("Неожиданный символ в структуре".into());
            }
            Ok(MetadataNode::Text(
                String::from_utf8_lossy(&self.input_bytes[start..self.position])
                    .trim()
                    .into(),
            ))
        }
    }
    let start = input
        .find('{')
        .ok_or("Не найдена структура в фигурных скобках")?;
    let mut parser = BraceParser {
        input_bytes: input.as_bytes(),
        position: start,
        node_count: 0,
    };
    let node = parser.node(0)?;
    if parser.input_bytes[parser.position..]
        .iter()
        .any(|character| !character.is_ascii_whitespace() && *character != 0)
    {
        return Err("Лишние данные после структуры".into());
    }
    Ok(node)
}
fn decode_windows_1251(bytes: &[u8]) -> String {
    encoding_rs::WINDOWS_1251
        .decode(bytes)
        .0
        .trim_end_matches('\0')
        .into()
}
fn read_file(path: &Path) -> ReadResult<Vec<u8>> {
    let file = File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if file.metadata().map_err(|error| error.to_string())?.len() > MAX_FILE_BYTES {
        return Err("Файл больше допустимых 256 МБ".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err("Файл вырос во время чтения".into());
    }
    Ok(bytes)
}
type CompoundDocument = cfb::CompoundFile<Cursor<Vec<u8>>>;
fn open_compound_document(path: &Path) -> ReadResult<CompoundDocument> {
    cfb::CompoundFile::open(Cursor::new(read_file(path)?)).map_err(|error| {
        format!(
            "Невозможно прочитать OLE-контейнер {}: {error}",
            path.display()
        )
    })
}
fn normalize_stream_path(stream_path: &Path) -> String {
    stream_path
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_lowercase()
}
fn collect_stream_paths(container: &CompoundDocument) -> BTreeMap<String, PathBuf> {
    container
        .walk()
        .filter(|entry| entry.is_stream())
        .map(|entry| (normalize_stream_path(entry.path()), entry.path().into()))
        .collect()
}
fn read_stream(
    container: &mut CompoundDocument,
    collect_stream_paths: &BTreeMap<String, PathBuf>,
    name: &str,
) -> ReadResult<Vec<u8>> {
    let stream_path = collect_stream_paths
        .get(&name.to_lowercase())
        .ok_or_else(|| format!("Отсутствует поток {name}"))?;
    let source_stream = container
        .open_stream(stream_path)
        .map_err(|entry| entry.to_string())?;
    if source_stream.len() > MAX_STREAM_BYTES {
        return Err(format!("Поток {name} превышает 32 МБ"));
    }
    let mut stream_bytes = Vec::new();
    source_stream
        .take(MAX_STREAM_BYTES + 1)
        .read_to_end(&mut stream_bytes)
        .map_err(|entry| entry.to_string())?;
    Ok(stream_bytes)
}
fn find_file(directory: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(directory)
        .ok()?
        .flatten()
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(name)
        })
        .map(|entry| entry.path())
}
struct BinaryReader<'input> {
    bytes: &'input [u8],
    position: usize,
}
impl<'input> BinaryReader<'input> {
    fn read_bytes(&mut self, byte_count: usize) -> ReadResult<&'input [u8]> {
        let end = self
            .position
            .checked_add(byte_count)
            .ok_or("Переполнение длины")?;
        let slice = self
            .bytes
            .get(self.position..end)
            .ok_or("Обрезанный бинарный поток")?;
        self.position = end;
        Ok(slice)
    }
    fn read_byte(&mut self) -> ReadResult<u8> {
        Ok(self.read_bytes(1)?[0])
    }
    fn read_u32(&mut self) -> ReadResult<u32> {
        Ok(u32::from_le_bytes(self.read_bytes(4)?.try_into().unwrap()))
    }
    fn read_string(&mut self) -> ReadResult<String> {
        let mut string_length = self.read_byte()? as usize;
        if string_length == 255 {
            string_length = u16::from_le_bytes(self.read_bytes(2)?.try_into().unwrap()) as usize;
            if string_length == 65535 {
                string_length = self.read_u32()? as usize;
            }
        }
        if string_length > MAX_STREAM_BYTES as usize {
            return Err("Слишком длинная строка".into());
        }
        Ok(decode_windows_1251(self.read_bytes(string_length)?))
    }
}
pub fn parse_permission_set(
    bytes: &[u8],
    name: String,
    stream_name: String,
) -> ReadResult<PermissionSet> {
    let mut binary_reader = BinaryReader { bytes, position: 0 };
    if binary_reader.read_u32()? != PERMISSION_SET_VERSION {
        return Err("Неизвестная версия набора прав (ожидалась 6)".into());
    }
    let description = binary_reader.read_string()?;
    let block_count = binary_reader.read_u32()? as usize;
    if block_count > MAX_PERMISSION_BLOCKS {
        return Err("Слишком много блоков прав".into());
    }
    let mut blocks = Vec::new();
    let mut seen_blocks = HashSet::new();
    for _ in 0..block_count {
        let kind = binary_reader.read_u32()?;
        let identifier = binary_reader.read_u32()?;
        let property_count = binary_reader.read_u32()? as usize;
        if property_count > MAX_BLOCK_ACTIONS || !seen_blocks.insert((kind, identifier)) {
            return Err("Некорректные или дублирующиеся блоки прав".into());
        }
        let mut properties = Vec::new();
        let mut seen_actions = HashSet::new();
        for _ in 0..property_count {
            let action = binary_reader.read_u32()?;
            let value = binary_reader.read_u32()?;
            if !seen_actions.insert(action) {
                return Err("Дублирующееся действие".into());
            }
            properties.push((action, value));
        }
        blocks.push(PermissionBlock {
            kind,
            identifier,
            properties,
        });
    }
    if binary_reader.position != bytes.len() {
        return Err("Лишние данные в конце набора прав".into());
    }
    Ok(PermissionSet {
        name,
        description,
        stream: stream_name,
        blocks,
    })
}
fn collect_objects(
    node: &MetadataNode,
    objects_by_identity: &mut BTreeMap<(u32, u32), MetadataObject>,
    title: &mut String,
) {
    let children = node.list();
    let Some(first) = children.first() else {
        return;
    };
    let kind = match first.text() {
        "Consts" => 1,
        "SbCnts" => 3,
        "Registers" => 6,
        "Documents" => 9,
        "Journals" | "Journalisters" => 11,
        "Reports" | "ReportList" => 16,
        "TaskItem" => 17,
        "CalcJournals" => 23,
        "Calendars" => 18,
        "CalcVars" => 24,
        "Plans" => 25,
        "TypOpersDef" => 28,
        _ => 0,
    };
    if kind != 0 {
        for child in &children[1..] {
            let child_fields = child.list();
            if child_fields.len() < 2 {
                continue;
            }
            if let Ok(identifier) = child_fields[0].text().parse::<u32>() {
                let name = child_fields[1].text().to_owned();
                if name.is_empty() {
                    continue;
                }
                let synonym = child_fields
                    .get(3)
                    .map(|synonym_node| synonym_node.text().to_owned())
                    .unwrap_or_default();
                if kind == 17 {
                    *title = name.clone();
                }
                objects_by_identity.insert(
                    (kind, identifier),
                    MetadataObject {
                        kind,
                        identifier,
                        name,
                        synonym,
                    },
                );
            }
        }
    }
    for child_fields in children {
        if matches!(child_fields, MetadataNode::List(_)) {
            collect_objects(child_fields, objects_by_identity, title);
        }
    }
}
pub fn parse_user(bytes: &[u8], name: String) -> ReadResult<DatabaseUser> {
    // Native 7.7 user page: 8-byte header followed by eight MFC-sized CP1251 strings.
    let mut binary_reader = BinaryReader { bytes, position: 0 };
    binary_reader.read_bytes(8)?;
    let mut fields = Vec::new();
    for _ in 0..8 {
        fields.push(binary_reader.read_string()?);
    }
    if binary_reader.position != bytes.len()
        && bytes[binary_reader.position..]
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err("Неподдерживаемый хвост записи пользователя".into());
    }
    // Password/hash fields are deliberately neither retained nor serialized.
    Ok(DatabaseUser {
        name,
        full_name: fields[1].clone(),
        directory: fields[2].clone(),
        interface: fields[6].clone(),
        role: fields[7].clone(),
    })
}
pub fn load(input: &Path) -> ReadResult<InformationDatabase> {
    let metadata_path = if input.is_dir() {
        find_file(input, "1cv7.md").ok_or("В папке нет 1CV7.MD")?
    } else {
        input.to_owned()
    };
    let metadata_path = metadata_path
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let base_directory = metadata_path
        .parent()
        .ok_or("У файла нет родительской папки")?;
    let metadata_file_bytes = std::fs::metadata(&metadata_path)
        .map_err(|error| error.to_string())?
        .len();
    let mut metadata_container = open_compound_document(&metadata_path)?;
    let metadata_stream_paths = collect_stream_paths(&metadata_container);
    let metadata_text = decode_windows_1251(&read_stream(
        &mut metadata_container,
        &metadata_stream_paths,
        "Metadata/Main MetaData Stream",
    )?);
    let tree = parse_braces(&metadata_text)?;
    let mut objects = BTreeMap::new();
    let mut title = String::new();
    collect_objects(&tree, &mut objects, &mut title);
    let container_contents = parse_braces(&decode_windows_1251(&read_stream(
        &mut metadata_container,
        &metadata_stream_paths,
        "UserDef/Page.2/Container.Contents",
    )?))?;
    let mut roles = Vec::new();
    let mut warnings = Vec::new();
    if base_directory.join("TEST-USERS.txt").exists() {
        warnings.push("Демо-пример: пользователи созданы искусственно для проверки просмотрщика. Не предназначены для входа в 1С.".into());
    }
    for child in container_contents.list().iter().skip(1) {
        let record_fields = child.list();
        if record_fields.len() < 3 {
            return Err("Повреждён список наборов прав".into());
        }
        let name = record_fields[2].text().to_string();
        let path = format!("UserDef/Page.2/{}", record_fields[1].text());
        let role = parse_permission_set(
            &read_stream(&mut metadata_container, &metadata_stream_paths, &path)?,
            name.clone(),
            path,
        )
        .map_err(|error| format!("Набор «{name}»: {error}"))?;
        for block in &role.blocks {
            if block.identifier != 0 {
                objects
                    .entry((block.kind, block.identifier))
                    .or_insert_with(|| MetadataObject {
                        kind: block.kind,
                        identifier: block.identifier,
                        name: format!("Объект #{}", block.identifier),
                        synonym: String::new(),
                    });
            }
            for (action, value) in &block.properties {
                if *value > 1 {
                    warnings.push(format!(
                        "«{name}»: неизвестное значение {value} действия 0x{action:X}"
                    ));
                }
            }
        }
        roles.push(role);
    }
    objects.insert(
        (17, 0),
        MetadataObject {
            kind: 17,
            identifier: 0,
            name: "Конфигурация".into(),
            synonym: title.clone(),
        },
    );
    objects.remove(&(17, 1));
    let mut users = Vec::new();
    if let Some(users_path) =
        find_file(base_directory, "usrdef").and_then(|directory| find_file(&directory, "users.usr"))
    {
        let mut users_container = open_compound_document(&users_path)?;
        let user_stream_paths = collect_stream_paths(&users_container);
        let container_contents = parse_braces(&decode_windows_1251(&read_stream(
            &mut users_container,
            &user_stream_paths,
            "Container.Contents",
        )?))?;
        for child in container_contents.list().iter().skip(1) {
            let record_fields = child.list();
            if record_fields.len() < 3 {
                return Err("Повреждён список пользователей".into());
            }
            let name = record_fields[2].text().to_owned();
            match read_stream(
                &mut users_container,
                &user_stream_paths,
                record_fields[1].text(),
            )
            .and_then(|user_bytes| parse_user(&user_bytes, name.clone()))
            {
                Ok(user) => {
                    if !roles
                        .iter()
                        .any(|permission_set| permission_set.name == user.role)
                    {
                        warnings.push(format!(
                            "Пользователь «{name}»: набор «{}» не найден в MD",
                            user.role
                        ));
                    }
                    users.push(user);
                }
                Err(error) => {
                    warnings.push(format!(
                        "Пользователь «{name}»: {error}. Права не определены"
                    ));
                    users.push(DatabaseUser {
                        name,
                        full_name: String::new(),
                        role: "Не определён (ошибка чтения)".into(),
                        interface: String::new(),
                        directory: String::new(),
                    });
                }
            }
        }
    } else {
        warnings.push("Файл USRDEF\\USERS.USR отсутствует. Именованные пользователи не загружены; наборы прав доступны для анализа.".into());
    }
    let unnamed_object_count = objects
        .values()
        .filter(|metadata_object| metadata_object.name.starts_with("Объект #"))
        .count();
    if unnamed_object_count > 0 {
        warnings.push(format!(
            "Для {unnamed_object_count} объектов не удалось сопоставить имя: показаны исходные тип и ID."
        ));
    }
    roles.sort_by_key(|permission_set| permission_set.name.to_lowercase());
    users.sort_by_key(|user| user.name.to_lowercase());
    let mut objects: Vec<_> = objects.into_values().collect();
    objects
        .sort_by_key(|metadata_object| (metadata_object.kind, metadata_object.name.to_lowercase()));
    Ok(InformationDatabase {
        source: metadata_path.to_string_lossy().into(),
        title,
        objects,
        roles,
        users,
        warnings,
        metadata_file_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn brackets_preserve_quotes_and_cyrillic() {
        let parsed_node = parse_braces("{\"root\",{\"12\",\"Товар \"\"А\"\"\"}}").unwrap();
        assert_eq!(parsed_node.list()[1].list()[1].text(), "Товар \"А\"");
        assert!(parse_braces("{\"x\"").is_err());
        assert!(parse_braces("{\"x\"}junk").is_err());
    }
    #[test]
    fn rights_defaults_overrides_and_unknown() {
        let metadata_object = MetadataObject {
            kind: 3,
            identifier: 84,
            name: "Товар".into(),
            synonym: String::new(),
        };
        let role = PermissionSet {
            name: "Роль".into(),
            description: String::new(),
            stream: String::new(),
            blocks: vec![
                PermissionBlock {
                    kind: 3,
                    identifier: 0,
                    properties: vec![(1, 1), (2, 0)],
                },
                PermissionBlock {
                    kind: 3,
                    identifier: 84,
                    properties: vec![(2, 1)],
                },
            ],
        };
        assert_eq!(role.access(&metadata_object, 1).0, AccessDecision::Allow);
        assert_eq!(role.access(&metadata_object, 2).0, AccessDecision::Allow);
        assert_eq!(role.access(&metadata_object, 3).0, AccessDecision::Unknown);
        assert_eq!(
            role.access(&metadata_object, 8).0,
            AccessDecision::NotApplicable
        );
    }
    #[test]
    fn truncated_rights_rejected() {
        assert!(
            parse_permission_set(&[6, 0, 0, 0, 0, 1, 0, 0, 0], "X".into(), "p".into()).is_err()
        );
    }
    #[test]
    fn user_fields_and_hash_not_exported() {
        let mut user_bytes = vec![0; 8];
        for field_text in [
            "SECRET_HASH",
            "Full name",
            "./usr",
            "\0",
            "1",
            "0",
            "Full",
            "Manager",
        ] {
            user_bytes.push(field_text.len() as u8);
            user_bytes.extend(field_text.as_bytes());
        }
        let user = parse_user(&user_bytes, "Ivan".into()).unwrap();
        assert_eq!(user.role, "Manager");
        assert_eq!(user.interface, "Full");
        assert!(
            !serde_json::to_string(&user)
                .unwrap()
                .contains("SECRET_HASH")
        );
        assert!(parse_user(&user_bytes[..12], "Ivan".into()).is_err());
    }
}
