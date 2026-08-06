use super::*;

#[test]
fn process_family_inventory_keeps_stable_tokens_and_surviving_members_after_root_exit() {
    let initial = vec![
        process(100, 1, Some(1_000), "Editor.exe"),
        process(110, 100, Some(1_100), "Renderer.exe"),
        process(120, 110, Some(1_200), "Worker.exe"),
        process(130, 999, Some(1_300), "Renderer.exe"),
        process(140, 100, None, "Worker.exe"),
    ];
    let reordered = vec![initial[2].clone(), initial[0].clone(), initial[1].clone()];
    let after_root_exit = vec![
        initial[2].clone(),
        initial[1].clone(),
        process(121, 110, Some(1_210), "Worker.exe"),
    ];
    let after_member_exit = vec![after_root_exit[0].clone(), after_root_exit[2].clone()];
    let mut controller =
        process_family_controller([initial, reordered, after_root_exit, after_member_exit]);

    let first = controller.inventory().expect("initial family inventory");
    assert_eq!(
        first
            .targets
            .iter()
            .map(|target| target.token.as_str())
            .collect::<Vec<_>>(),
        vec!["target:1", "target:2", "target:3"]
    );
    assert_eq!(first.targets[0].display_name, "Editor · 运行中");
    assert_eq!(first.targets[1].display_name, "Renderer · 运行中");
    assert_eq!(first.targets[2].display_name, "Worker · 运行中");

    let second = controller.inventory().expect("reordered family inventory");
    assert_eq!(
        second
            .targets
            .iter()
            .map(|target| target.token.as_str())
            .collect::<Vec<_>>(),
        vec!["target:1", "target:2", "target:3"]
    );

    let third = controller
        .inventory()
        .expect("surviving family member inventory");
    assert_eq!(
        third
            .targets
            .iter()
            .map(|target| target.token.as_str())
            .collect::<Vec<_>>(),
        vec!["target:2", "target:3", "target:4"]
    );
    controller
        .runtime_libraries
        .insert("target:2".into(), "synthetic-runtime.dll".into());

    let fourth = controller.inventory().expect("partial exit inventory");
    assert_eq!(
        fourth
            .targets
            .iter()
            .map(|target| target.token.as_str())
            .collect::<Vec<_>>(),
        vec!["target:3", "target:4"]
    );
    assert!(!controller.runtime_libraries.contains_key("target:2"));
    assert!(controller
        .prepare("target:2", &[WireFeature::TextReplace])
        .is_err());
    assert!(controller
        .prepare("target:3", &[WireFeature::TextReplace])
        .is_ok());
}

#[test]
fn process_family_inventory_never_reuses_a_token_after_process_id_reuse() {
    let mut controller = process_family_controller([
        vec![process(100, 1, Some(1_000), "Editor.exe")],
        Vec::new(),
        vec![process(100, 1, Some(2_000), "Editor.exe")],
    ]);

    let first = controller.inventory().expect("first process instance");
    assert_eq!(first.targets[0].token, "target:1");
    assert!(controller
        .inventory()
        .expect("process exit inventory")
        .targets
        .is_empty());
    let replacement = controller
        .inventory()
        .expect("replacement process instance");
    assert_eq!(replacement.targets[0].token, "target:2");
}

#[test]
fn same_executable_descendant_does_not_outrank_its_top_level_root() {
    let mut controller = process_family_controller([vec![
        process(100, 200, Some(1_100), "Editor.exe"),
        process(200, 1, Some(1_000), "Editor.exe"),
    ]]);

    let inventory = controller.inventory().expect("same-executable family");

    assert_eq!(inventory.targets.len(), 2);
    assert_eq!(
        controller.targets[&inventory.targets[0].token].process_id,
        200
    );
    assert_eq!(
        controller.targets[&inventory.targets[1].token].process_id,
        100
    );
}

#[test]
fn independent_same_executable_instances_remain_top_level_roots() {
    let mut controller = process_family_controller([Vec::new()]);

    let authorized = controller.authorized_processes(vec![
        process(100, 1, Some(1_000), "Editor.exe"),
        process(200, 1, Some(2_000), "Editor.exe"),
    ]);

    assert_eq!(authorized.len(), 2);
    assert!(authorized.iter().all(|process| process.is_root));
}
