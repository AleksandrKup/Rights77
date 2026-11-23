use rights77::{AccessDecision, reader, report};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

struct TemporaryDirectory {
    path: PathBuf,
}
impl TemporaryDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rights77-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}
impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
fn encode_windows_1251(text: &str) -> Vec<u8> {
    let (encoded_bytes, _, encoding_error) = encoding_rs::WINDOWS_1251.encode(text);
    assert!(!encoding_error);
    encoded_bytes.into_owned()
}
fn write_compound_file(path: &Path, streams: &[(&str, Vec<u8>)]) {
    let mut container = cfb::create(path).unwrap();
    for (name, bytes) in streams {
        let path = Path::new(name);
        if let Some(parent) = path.parent() {
            container.create_storage_all(parent).unwrap();
        }
        container
            .create_stream(path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }
}
fn native_permission_set_bytes() -> Vec<u8> {
    // Documented v6 header, one default catalogue block and one explicit exception.
    let mut encoded_bytes = 6u32.to_le_bytes().to_vec();
    encoded_bytes.push(0);
    encoded_bytes.extend(2u32.to_le_bytes());
    for words in [vec![3, 0, 2, 1, 1, 4, 0], vec![3, 84, 1, 4, 1]] {
        for word in words {
            encoded_bytes.extend((word as u32).to_le_bytes());
        }
    }
    encoded_bytes
}
fn create_database_fixture(root: &Path, users: bool) {
    write_compound_file(
        &root.join("1Cv7.MD"),
        &[
            (
                "/Metadata/Main MetaData Stream",
                encode_windows_1251(
                    "{ {\"TaskItem\",{\"1\",\"Тестовая база\",\"1\",\"\"}}, {\"SbCnts\",{\"84\",\"Номенклатура\",\"\",\"Товары\"}} }",
                ),
            ),
            (
                "/UserDef/Page.2/Container.Contents",
                encode_windows_1251(
                    "{\"Container.Contents\",{\"RightsEditorType\",\"Page.1\",\"Менеджер\",\"\"}}",
                ),
            ),
            ("/UserDef/Page.2/Page.1", native_permission_set_bytes()),
        ],
    );
    if users {
        std::fs::create_dir(root.join("UsrDef")).unwrap();
        let mut user = vec![0; 8];
        for text in [
            "HASH_NOT_FOR_EXPORT",
            "Иван Иванов",
            "./usr",
            "\0",
            "1",
            "0",
            "Полный",
            "Менеджер",
        ] {
            let bytes = encode_windows_1251(text);
            user.push(bytes.len() as u8);
            user.extend(bytes);
        }
        write_compound_file(
            &root.join("UsrDef/Users.USR"),
            &[
                (
                    "/Container.Contents",
                    encode_windows_1251(
                        "{\"Container.Contents\",{\"UserDefType\",\"Page.1\",\"Иванов\",\"\"}}",
                    ),
                ),
                ("/Page.1", user),
            ],
        );
    }
}
#[test]
fn complete_native_files_join_users_roles_and_defaults_without_mutation() {
    let temporary_directory = TemporaryDirectory::new();
    create_database_fixture(&temporary_directory.path, true);
    let before = std::fs::read(temporary_directory.path.join("1Cv7.MD")).unwrap();
    let users_before = std::fs::read(temporary_directory.path.join("UsrDef/Users.USR")).unwrap();
    let database = reader::load(&temporary_directory.path).unwrap();
    assert_eq!(database.users.len(), 1);
    assert_eq!(database.users[0].name, "Иванов");
    assert_eq!(database.users[0].role, "Менеджер");
    assert_eq!(database.users[0].full_name, "Иван Иванов");
    let object = database
        .objects
        .iter()
        .find(|metadata_object| metadata_object.identifier == 84)
        .unwrap();
    assert_eq!(object.name, "Номенклатура");
    assert_eq!(database.roles[0].access(object, 4).0, AccessDecision::Allow);
    assert_eq!(database.roles[0].access(object, 1).0, AccessDecision::Allow);
    let generate_csv = report::generate_csv(&database);
    assert!(generate_csv.contains("Иванов"));
    assert!(generate_csv.contains("Корректировка"));
    assert!(!generate_csv.contains("HASH_NOT_FOR_EXPORT"));
    assert!(
        !serde_json::to_string(&database)
            .unwrap()
            .contains("HASH_NOT_FOR_EXPORT")
    );
    assert_eq!(
        before,
        std::fs::read(temporary_directory.path.join("1Cv7.MD")).unwrap()
    );
    assert_eq!(
        users_before,
        std::fs::read(temporary_directory.path.join("UsrDef/Users.USR")).unwrap()
    );
    assert!(
        report::save_export(
            &database,
            &temporary_directory.path.join("report.csv"),
            false
        )
        .is_err()
    );
    assert!(
        report::save_export(&database, &temporary_directory.path.join("1Cv7.MD"), false).is_err()
    );
    let output = TemporaryDirectory::new();
    let target = output.path.join("report.csv");
    report::save_export(&database, &target, false).unwrap();
    assert!(
        std::fs::read_to_string(target)
            .unwrap()
            .contains("Номенклатура")
    );
}
#[test]
fn missing_users_is_explicit_not_fake() {
    let temporary_directory = TemporaryDirectory::new();
    create_database_fixture(&temporary_directory.path, false);
    let database = reader::load(&temporary_directory.path).unwrap();
    assert!(database.users.is_empty());
    assert!(
        database
            .warnings
            .iter()
            .any(|warning| warning.contains("USERS.USR отсутствует"))
    );
    assert_eq!(database.roles.len(), 1);
}
#[test]
fn invalid_container_returns_error() {
    let temporary_directory = TemporaryDirectory::new();
    std::fs::write(temporary_directory.path.join("1cv7.md"), b"not an OLE file").unwrap();
    assert!(
        reader::load(&temporary_directory.path)
            .unwrap_err()
            .contains("OLE")
    );
}
#[test]
fn local_demo_golden() {
    let Ok(path) = std::env::var("RIGHTS77_TEST_BASE") else {
        return;
    };
    let database = reader::load(Path::new(&path)).unwrap();
    assert_eq!(database.title, "Торговля+Склад, редакция 9.2");
    assert_eq!(database.roles.len(), 4);
    assert_eq!(database.objects.len(), 264);
    assert!(
        database
            .roles
            .iter()
            .all(|permission_set| permission_set.blocks.len() == 277)
    );
    assert!(
        database
            .objects
            .iter()
            .all(|metadata_object| !metadata_object.name.starts_with("Объект #"))
    );
    let object = database
        .objects
        .iter()
        .find(|metadata_object| metadata_object.kind == 3 && metadata_object.name == "Номенклатура")
        .unwrap();
    assert_eq!(object.identifier, 84);
    for role in &database.roles {
        let expected = if role.name == "ОператорККМ" {
            AccessDecision::Deny
        } else {
            AccessDecision::Allow
        };
        assert_eq!(role.access(object, 4).0, expected);
    }
    let roundtrip: rights77::InformationDatabase =
        serde_json::from_str(&serde_json::to_string(&database).unwrap()).unwrap();
    assert_eq!(roundtrip.roles.len(), 4);
}

#[test]
fn local_demo_native_users_match_configurator() {
    let path = std::env::var_os("RIGHTS77_TEST_BASE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "C:\\InfoBase7\\DemoDB".into());
    if std::env::var_os("RIGHTS77_TEST_BASE").is_none() {
        return; // Explicit opt-in: this checks users created in the Configurator.
    }
    let database = reader::load(&path).expect("Не удалось прочитать локальную демо-базу");
    assert_eq!(database.users.len(), 4);
    assert_eq!(database.roles.len(), 4);
    assert!(
        database
            .users
            .iter()
            .all(|user| user.name.starts_with("Тест_"))
    );
    let item = database
        .objects
        .iter()
        .find(|metadata_object| metadata_object.name == "Номенклатура" && metadata_object.kind == 3)
        .unwrap();
    let mut allowed_user_names = Vec::new();
    let mut denied_user_names = Vec::new();
    for user in &database.users {
        let permission_set = database
            .roles
            .iter()
            .find(|permission_set| permission_set.name == user.role)
            .unwrap();
        match permission_set.access(item, 4).0 {
            AccessDecision::Allow => allowed_user_names.push(&user.name),
            AccessDecision::Deny => denied_user_names.push(&user.name),
            _ => panic!("Неопределённое право для {}", user.name),
        }
    }
    assert_eq!(allowed_user_names.len(), 3);
    assert_eq!(denied_user_names.len(), 1);
    assert!(denied_user_names.iter().all(|name| name.contains("Кассир")));
    let generate_csv = report::generate_csv(&database);
    for user in &database.users {
        assert!(generate_csv.contains(&user.name));
    }
    for (name, full_name, role) in [
        (
            "Тест_Администратор",
            "Анна Смирнова (тест Rights77)",
            "Администратор",
        ),
        ("Тест_Менеджер", "Иван Петров (тест Rights77)", "Менеджер"),
        (
            "Тест_Продавец",
            "Ольга Васильева (тест Rights77)",
            "Продавец",
        ),
        (
            "Тест_Кассир",
            "Елена Кузнецова (тест Rights77)",
            "ОператорККМ",
        ),
    ] {
        let user = database
            .users
            .iter()
            .find(|user| user.name == name)
            .unwrap();
        assert_eq!(user.full_name, full_name);
        assert_eq!(user.role, role);
        assert_eq!(user.interface, role);
        assert!(user.directory.is_empty());
    }
    assert_eq!(database.objects.len(), 264);
}

#[test]
fn every_truncated_rights_page_is_rejected_and_unknown_value_remains_unknown() {
    let mut bytes = native_permission_set_bytes();
    for end in 0..bytes.len() {
        assert!(
            reader::parse_permission_set(&bytes[..end], "Role".into(), "Page.1".into()).is_err(),
            "accepted truncated page at {end}"
        );
    }
    let last_value_offset = bytes.len() - 4;
    bytes[last_value_offset..].copy_from_slice(&9u32.to_le_bytes());
    let permission_set =
        reader::parse_permission_set(&bytes, "Role".into(), "Page.1".into()).unwrap();
    let metadata_object = rights77::MetadataObject {
        kind: 3,
        identifier: 84,
        name: "Номенклатура".into(),
        synonym: String::new(),
    };
    assert_eq!(
        permission_set.access(&metadata_object, 4).0,
        AccessDecision::Unknown
    );
}

#[test]
fn malformed_user_is_retained_with_unknown_role() {
    let temporary_directory = TemporaryDirectory::new();
    create_database_fixture(&temporary_directory.path, true);
    let mut container = cfb::open_rw(temporary_directory.path.join("UsrDef/Users.USR")).unwrap();
    container
        .create_stream("/Page.1")
        .unwrap()
        .write_all(b"broken")
        .unwrap();
    drop(container);
    let database = reader::load(&temporary_directory.path).unwrap();
    assert_eq!(database.users.len(), 1);
    assert_eq!(database.users[0].name, "Иванов");
    assert!(database.users[0].role.contains("Не определён"));
    assert!(
        database
            .warnings
            .iter()
            .any(|warning| warning.contains("Права не определены"))
    );
}

#[test]
fn long_user_strings_and_missing_role_are_handled() {
    let temporary_directory = TemporaryDirectory::new();
    create_database_fixture(&temporary_directory.path, true);
    let mut bytes = vec![0; 8];
    let long_full_name = "Я".repeat(300);
    for text in [
        "HASH",
        &long_full_name,
        "./usr",
        "\0",
        "1",
        "0",
        "Full",
        "НетТакогоНабора",
    ] {
        let encoded_bytes = encode_windows_1251(text);
        if encoded_bytes.len() >= 255 {
            bytes.push(255);
            bytes.extend((encoded_bytes.len() as u16).to_le_bytes());
        } else {
            bytes.push(encoded_bytes.len() as u8);
        }
        bytes.extend(encoded_bytes);
    }
    let mut container = cfb::open_rw(temporary_directory.path.join("UsrDef/Users.USR")).unwrap();
    container
        .create_stream("/Page.1")
        .unwrap()
        .write_all(&bytes)
        .unwrap();
    drop(container);
    let database = reader::load(&temporary_directory.path).unwrap();
    assert_eq!(database.users[0].full_name, long_full_name);
    assert!(
        database
            .warnings
            .iter()
            .any(|warning| warning.contains("не найден в MD"))
    );
}
