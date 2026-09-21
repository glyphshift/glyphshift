use glyphshift_domain::Generation;
use glyphshift_translation::TranslationSnapshot;

#[test]
fn bulk_location_entries_match_sequential_entry_updates() {
    let seed = TranslationSnapshot::empty(Generation::new(7)).with_entry_for_adapters(
        "menu",
        "Open",
        "旧值",
        ["adapter.synthetic"],
    );
    let sequential = seed
        .clone()
        .with_entry("menu", "Open", "打开")
        .with_entry("menu", "Close", "关闭");
    let bulk = seed.with_entries_at_location(
        "menu",
        [("Open", "打开"), ("Close", "关闭")],
    );

    assert_eq!(bulk, sequential);
}
