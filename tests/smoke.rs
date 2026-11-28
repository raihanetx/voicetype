use voicetype::version;

#[test]
fn version_is_not_empty() {
    assert!(!version().is_empty());
}
