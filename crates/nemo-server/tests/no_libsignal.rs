//! MIT crates in this workspace must not link libsignal (ADR-0028).

#[test]
fn nemo_server_does_not_depend_on_libsignal_or_core() {
    let lock = include_str!("../../../Cargo.lock");
    let section = package_block(lock, "nemo-server").expect("nemo-server in Cargo.lock");
    assert!(
        !section.contains("libsignal"),
        "nemo-server must not depend on libsignal:\n{section}"
    );
    assert!(
        !section.contains("nemo-core"),
        "nemo-server must not depend on nemo-core:\n{section}"
    );
}

fn package_block<'a>(lock: &'a str, name: &str) -> Option<&'a str> {
    let header = format!("name = \"{name}\"");
    let start = lock.find(&header)?;
    let rest = &lock[start..];
    let end = rest.find("\n[[package]]").unwrap_or(rest.len());
    Some(&rest[..end])
}
