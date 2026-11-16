use std::{collections::BTreeMap, env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=../VERSION");
    let source = fs::read_to_string("../VERSION")
        .expect("INFRA-AV/VERSION is required in the source bundle");
    let rows: BTreeMap<_, _> = source
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.trim().starts_with('#'))
        .map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            assert_eq!(fields.len(), 2, "invalid VERSION row");
            (fields[0], fields[1])
        })
        .collect();
    let version = |name| {
        let value = rows
            .get(name)
            .expect("required VERSION component is missing");
        let parts: Vec<_> = value.split('.').collect();
        assert!(
            parts.len() == 3
                && parts[0] == "10"
                && parts[1].parse::<u64>().is_ok()
                && parts[2].len() == 12
                && parts[2].bytes().all(|b| b.is_ascii_digit()),
            "invalid product version"
        );
        *value
    };
    let output = format!(
        "pub const VERSION: &str = {:?};\npub const BACKEND_VERSION: &str = {:?};\n",
        version("infra-av"),
        version("backend")
    );
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("product-version.rs"),
        output,
    )
    .unwrap();
}
