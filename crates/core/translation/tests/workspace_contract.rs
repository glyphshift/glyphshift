use glyphshift_domain::Generation;
use glyphshift_translation::{
    ApplyResult, EventId, SourceDocument, SourceId, SourceLayer, SourceRevision,
    TranslationWorkspace, WorkspaceChange, WorkspaceContext, WorkspaceEntry, WorkspaceError,
    WorkspaceEventKind, WorkspaceLocation, WorkspaceOrigin, WorkspaceRejection,
};

fn workspace() -> TranslationWorkspace {
    TranslationWorkspace::new(
        "org.example.editor",
        "zh-CN",
        [WorkspaceLocation::new("menu", "Menu")],
    )
}

#[test]
fn wsp_001_applies_a_ui_change_and_advances_revision_and_generation_once() {
    let mut workspace = workspace();

    let applied = workspace
        .apply(
            WorkspaceChange::upsert(WorkspaceEntry::new("menu", "Open", "打开")),
            SourceRevision::new(0),
        )
        .expect("valid UI change should apply");

    assert_eq!(
        applied,
        ApplyResult::Applied {
            revision: SourceRevision::new(1),
            generation: Generation::new(1),
        }
    );
    assert_eq!(
        workspace.snapshot().lookup("menu", "Open").as_deref(),
        Some("打开")
    );
}

#[test]
fn wsp_003_rejects_a_stale_base_without_overwriting_or_advancing_generation() {
    let mut workspace = workspace();
    workspace
        .apply(
            WorkspaceChange::upsert(WorkspaceEntry::new("menu", "Open", "打开")),
            SourceRevision::new(0),
        )
        .expect("first change should apply");

    let rejection = workspace
        .apply(
            WorkspaceChange::upsert(WorkspaceEntry::new("menu", "Open", "覆盖")),
            SourceRevision::new(0),
        )
        .expect_err("stale base revision must conflict");

    assert_eq!(
        rejection,
        WorkspaceError::Conflict {
            current: SourceRevision::new(1),
        }
    );
    assert_eq!(workspace.revision(), SourceRevision::new(1));
    assert_eq!(workspace.snapshot().generation(), Generation::new(1));
    assert_eq!(
        workspace.snapshot().lookup("menu", "Open").as_deref(),
        Some("打开")
    );
}

#[test]
fn wsp_002_rescans_an_external_source_through_the_same_revision_generation_pipeline() {
    let mut workspace = workspace();

    let applied = workspace
        .rescan(SourceDocument::new(
            SourceId::new("package-a"),
            SourceLayer::Package,
            [WorkspaceEntry::new("menu", "Open", "打开")],
        ))
        .expect("valid external source should apply");

    assert_eq!(
        applied,
        ApplyResult::Applied {
            revision: SourceRevision::new(1),
            generation: Generation::new(1),
        }
    );
    assert_eq!(
        workspace.snapshot().lookup("menu", "Open").as_deref(),
        Some("打开")
    );
}

#[test]
fn wsp_004_005_009_keep_last_good_skip_noops_and_recover_after_a_bad_rescan() {
    let mut workspace = workspace();
    let source_id = SourceId::new("package-a");
    let stable = SourceDocument::new(
        source_id.clone(),
        SourceLayer::Package,
        [WorkspaceEntry::new("menu", "Open", "打开")],
    );
    workspace
        .rescan(stable.clone())
        .expect("stable source should apply");

    assert_eq!(
        workspace.rescan(stable),
        Ok(ApplyResult::NoChange {
            revision: SourceRevision::new(1),
            generation: Generation::new(1),
        })
    );
    assert_eq!(
        workspace.rescan(SourceDocument::rejected(
            source_id.clone(),
            SourceLayer::Package,
            WorkspaceRejection::Malformed("invalid schema".into()),
        )),
        Err(WorkspaceError::Rejected(WorkspaceRejection::Malformed(
            "invalid schema".into()
        )))
    );
    assert_eq!(workspace.revision(), SourceRevision::new(1));
    assert_eq!(workspace.snapshot().generation(), Generation::new(1));
    assert_eq!(
        workspace.snapshot().lookup("menu", "Open").as_deref(),
        Some("打开")
    );

    assert_eq!(
        workspace.rescan(SourceDocument::new(
            source_id,
            SourceLayer::Package,
            [WorkspaceEntry::new("menu", "Open", "开启")],
        )),
        Ok(ApplyResult::Applied {
            revision: SourceRevision::new(2),
            generation: Generation::new(2),
        })
    );
    assert_eq!(
        workspace.snapshot().lookup("menu", "Open").as_deref(),
        Some("开启")
    );
}

#[test]
fn wsp_006_merges_builtin_package_and_user_layers_in_fixed_priority_order() {
    let mut workspace = workspace();
    workspace
        .rescan(SourceDocument::new(
            SourceId::new("builtin"),
            SourceLayer::Builtin,
            [WorkspaceEntry::new("menu", "Open", "Builtin")],
        ))
        .expect("builtin source should load");
    workspace
        .rescan(SourceDocument::new(
            SourceId::new("package-a"),
            SourceLayer::Package,
            [WorkspaceEntry::new("menu", "Open", "Package")],
        ))
        .expect("package source should load");
    workspace
        .apply(
            WorkspaceChange::upsert(WorkspaceEntry::new("menu", "Open", "User")),
            SourceRevision::new(2),
        )
        .expect("user edit should apply");

    assert_eq!(
        workspace.snapshot().lookup("menu", "Open").as_deref(),
        Some("User")
    );
}

#[test]
fn wsp_007_uses_a_deterministic_same_layer_tie_breaker_independent_of_rescan_order() {
    let package_a = SourceDocument::new(
        SourceId::new("package-a"),
        SourceLayer::Package,
        [WorkspaceEntry::new("menu", "Open", "A")],
    );
    let package_b = SourceDocument::new(
        SourceId::new("package-b"),
        SourceLayer::Package,
        [WorkspaceEntry::new("menu", "Open", "B")],
    );
    let mut forward = workspace();
    forward
        .rescan(package_a.clone())
        .expect("first package should load");
    forward
        .rescan(package_b.clone())
        .expect("second package should load");
    let mut reverse = workspace();
    reverse
        .rescan(package_b)
        .expect("first reverse package should load");
    reverse
        .rescan(package_a)
        .expect("second reverse package should load");

    assert_eq!(
        forward.snapshot().lookup("menu", "Open").as_deref(),
        Some("A")
    );
    assert_eq!(
        reverse.snapshot().lookup("menu", "Open").as_deref(),
        Some("A")
    );
}

#[test]
fn wsp_008_publishes_immutable_snapshots_with_deterministic_content_digests() {
    let mut primary = workspace();
    primary
        .rescan(SourceDocument::new(
            SourceId::new("package-a"),
            SourceLayer::Package,
            [WorkspaceEntry::new("menu", "Open", "打开")],
        ))
        .expect("first source should load");
    let first = primary.snapshot().clone();

    primary
        .rescan(SourceDocument::new(
            SourceId::new("package-a"),
            SourceLayer::Package,
            [WorkspaceEntry::new("menu", "Open", "开启")],
        ))
        .expect("updated source should publish a new snapshot");
    let second = primary.snapshot().clone();

    assert_eq!(first.generation(), Generation::new(1));
    assert_eq!(second.generation(), Generation::new(2));
    assert_ne!(first.digest(), second.digest());
    assert_eq!(first.lookup("menu", "Open").as_deref(), Some("打开"));
    assert_eq!(second.lookup("menu", "Open").as_deref(), Some("开启"));

    let mut equivalent = workspace();
    equivalent
        .rescan(SourceDocument::new(
            SourceId::new("package-a"),
            SourceLayer::Package,
            [WorkspaceEntry::new("menu", "Open", "开启")],
        ))
        .expect("equivalent source should load");
    assert_eq!(equivalent.snapshot().digest(), second.digest());
}

#[test]
fn wsp_010_resumes_workspace_events_after_the_last_seen_event_id() {
    let mut workspace = workspace();
    workspace
        .apply(
            WorkspaceChange::upsert(WorkspaceEntry::new("menu", "Open", "打开")),
            SourceRevision::new(0),
        )
        .expect("first change should apply");
    workspace
        .apply(
            WorkspaceChange::upsert(WorkspaceEntry::new("menu", "Save", "保存")),
            SourceRevision::new(1),
        )
        .expect("second change should apply");

    let all = workspace.subscribe(EventId::new(0));
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].id(), EventId::new(1));
    assert_eq!(all[1].id(), EventId::new(2));
    assert_eq!(
        all[1].kind(),
        &WorkspaceEventKind::Published {
            revision: SourceRevision::new(2),
            generation: Generation::new(2),
        }
    );

    let resumed = workspace.subscribe(EventId::new(1));
    assert_eq!(resumed, vec![all[1].clone()]);
    assert!(workspace.subscribe(EventId::new(2)).is_empty());
}

#[test]
fn wsp_011_exposes_user_semantics_and_conflicts_without_runtime_routing_details() {
    let mut workspace = TranslationWorkspace::new(
        "org.example.editor",
        "zh-CN",
        [WorkspaceLocation::with_context(
            "parameter",
            "Parameter",
            "tool",
        )],
    );
    let contextual_entry = |translation| {
        WorkspaceEntry::new("parameter", "Radius", translation)
            .with_context(WorkspaceContext::new("tool", "blur", "Blur"))
    };
    workspace
        .rescan(SourceDocument::new(
            SourceId::new("package-a"),
            SourceLayer::Package,
            [contextual_entry("半径")],
        ))
        .expect("first contextual source should load");
    workspace
        .rescan(SourceDocument::new(
            SourceId::new("package-b"),
            SourceLayer::Package,
            [contextual_entry("范围")],
        ))
        .expect("conflicting contextual source should load");

    let view = workspace.view();

    assert_eq!(view.extension_id(), "org.example.editor");
    assert_eq!(view.locale(), "zh-CN");
    assert_eq!(view.entries().len(), 1);
    assert_eq!(view.entries()[0].location_label(), "Parameter");
    assert_eq!(
        view.entries()[0]
            .context()
            .expect("context should be visible")
            .label(),
        "Blur"
    );
    assert_eq!(view.entries()[0].source(), "Radius");
    assert_eq!(view.entries()[0].translation(), "半径");
    assert_eq!(
        view.entries()[0].origin(),
        &WorkspaceOrigin::Package(SourceId::new("package-a"))
    );
    assert_eq!(view.conflicts().len(), 1);
    assert_eq!(
        workspace
            .snapshot()
            .lookup_context("parameter", "tool", "blur", "Radius")
            .as_deref(),
        Some("半径")
    );

    assert_eq!(
        workspace.rescan(SourceDocument::new(
            SourceId::new("invalid-context"),
            SourceLayer::Package,
            [WorkspaceEntry::new("parameter", "Radius", "Invalid")
                .with_context(WorkspaceContext::new("effect", "blur", "Blur"))],
        )),
        Err(WorkspaceError::Rejected(
            WorkspaceRejection::InvalidContext {
                location: "parameter".into(),
            }
        ))
    );
}

#[test]
fn wsp_012_serializes_only_translation_catalog_semantics() {
    let mut workspace = TranslationWorkspace::new(
        "org.example.editor",
        "zh-CN",
        [WorkspaceLocation::with_context(
            "parameter",
            "Parameter",
            "tool",
        )],
    );
    workspace
        .rescan(SourceDocument::new(
            SourceId::new("package-a"),
            SourceLayer::Package,
            [WorkspaceEntry::new("parameter", "Radius", "半径")
                .for_adapters(["example.synthetic.inline"])
                .with_context(WorkspaceContext::new("tool", "blur", "Blur"))],
        ))
        .expect("contextual source should load");

    let catalog = workspace.serialize_catalog();

    assert!(catalog.contains("glyphshift.translation/1"));
    assert!(catalog.contains("org.example.editor"));
    assert!(catalog.contains("\"location\":\"parameter\""));
    assert!(catalog.contains("\"kind\":\"tool\""));
    assert!(catalog.contains("\"source\":\"Radius\""));
    assert!(catalog.contains("\"hooks\":[\"example.synthetic.inline\"]"));
    assert!(!catalog.contains("package-a"));
    assert!(!catalog.contains("adapter"));
    assert!(!catalog.contains("font"));
    assert!(!catalog.contains("priority"));
    assert!(!catalog.contains(":\\"));
}
