use glyphshift_desktop_backend::{
    BackendError, DesktopBackend, DictionaryCreate, DictionaryDefaultFont, DictionaryEdit,
    DictionaryEntryFont, DictionaryRuleCreate, DictionaryRuleKey, ExecutableSelection,
    SoftwareEdit, WorkflowCreate, WorkflowEdit, WorkflowTargetCreate,
};
use glyphshift_workflow::ResolveError;
use std::fs;
use tempfile::tempdir;

#[test]
fn independent_dictionary_is_created_and_reopened_by_id() {
    let root = tempdir().expect("temporary product data");
    let mut backend = DesktopBackend::open(root.path()).expect("open empty V2 data");

    let created = backend
        .create_dictionary(
            DictionaryCreate::new("dictionary.interface-zh-cn", "界面汉化", "zh-CN")
                .with_description("用于创作软件界面汉化")
                .with_default_font(DictionaryDefaultFont::substitute("Product Sans"))
                .with_entries([DictionaryRuleCreate::replace("main-ui", "Open", "打开")
                    .with_font(DictionaryEntryFont::substitute("Entry Sans"))
                    .for_adapters(["example.synthetic.inline"])]),
        )
        .expect("create an independent dictionary");
    drop(backend);

    let reopened = DesktopBackend::open(root.path()).expect("reopen product data");
    let persisted = reopened
        .dictionary("dictionary.interface-zh-cn")
        .expect("read the dictionary by id after restart");
    let rule = &persisted.entries()[0];
    assert_eq!(persisted.description(), "用于创作软件界面汉化");

    assert_eq!(
        (
            &created,
            persisted.id(),
            persisted.name(),
            persisted.locale(),
            persisted.revision(),
            persisted.default_font(),
            rule.location(),
            rule.source(),
            rule.translation(),
            rule.font(),
            rule.adapter_ids(),
        ),
        (
            persisted,
            "dictionary.interface-zh-cn",
            "界面汉化",
            "zh-CN",
            1,
            &DictionaryDefaultFont::substitute("Product Sans"),
            "main-ui",
            "Open",
            Some("打开"),
            &DictionaryEntryFont::substitute("Entry Sans"),
            &["example.synthetic.inline".into()][..],
        )
    );
    assert!(
        !root.path().join("catalogs").exists(),
        "the independent Dictionary contract must not recreate the removed software Catalog store"
    );
}

#[test]
fn dictionary_hook_type_is_persisted_once_and_exposed_in_its_summary() {
    let root = tempdir().expect("temporary product data");
    let mut backend = DesktopBackend::open(root.path()).expect("open empty V2 data");

    let created = backend
        .create_dictionary(
            DictionaryCreate::new("dictionary.gdi", "菜单词典", "zh-CN")
                .for_adapter("example.synthetic.gdi")
                .with_entries([DictionaryRuleCreate::replace("main-ui", "File", "文件")]),
        )
        .expect("create a dictionary with one hook type");
    assert_eq!(created.hook_type_id(), Some("example.synthetic.gdi"));
    let updated = backend
        .upsert_dictionary_rule(
            created.id(),
            DictionaryRuleCreate::replace("main-ui", "Edit", "编辑"),
            None,
            created.revision(),
        )
        .expect("add a rule without dropping the dictionary hook type");
    assert_eq!(updated.hook_type_id(), Some("example.synthetic.gdi"));
    assert_eq!(
        backend.snapshot().dictionaries()[0].hook_type_id(),
        Some("example.synthetic.gdi")
    );
    drop(backend);

    let reopened = DesktopBackend::open(root.path()).expect("reopen product data");
    assert_eq!(
        reopened
            .dictionary("dictionary.gdi")
            .expect("persisted dictionary")
            .hook_type_id(),
        Some("example.synthetic.gdi")
    );
}

#[test]
fn dictionary_rule_crud_uses_the_dictionary_revision_and_complete_rule_key() {
    let root = tempdir().expect("temporary product data");
    let mut backend = DesktopBackend::open(root.path()).expect("open empty V2 data");
    let dictionary = backend
        .create_dictionary(DictionaryCreate::new(
            "dictionary.entries",
            "条目 CRUD",
            "zh-CN",
        ))
        .expect("create empty dictionary");
    let inserted = backend
        .upsert_dictionary_rule(
            dictionary.id(),
            DictionaryRuleCreate::replace("main-ui", "Open", "打开")
                .for_adapters(["adapter.alpha"]),
            None,
            dictionary.revision(),
        )
        .expect("insert rule");
    let original_key = DictionaryRuleKey::new("main-ui", "Open").for_adapters(["adapter.alpha"]);
    let updated = backend
        .upsert_dictionary_rule(
            dictionary.id(),
            DictionaryRuleCreate::replace("main-ui", "Open file", "打开文件")
                .with_context("window", "document")
                .with_font(DictionaryEntryFont::substitute("Document Sans"))
                .for_adapters(["adapter.beta"]),
            Some(original_key),
            inserted.revision(),
        )
        .expect("replace complete rule");
    let updated_key = DictionaryRuleKey::new("main-ui", "Open file")
        .with_context("window", "document")
        .for_adapters(["adapter.beta"]);
    let deleted = backend
        .delete_dictionary_rules(dictionary.id(), [updated_key], updated.revision())
        .expect("batch delete rule");
    drop(backend);

    let reopened = DesktopBackend::open(root.path()).expect("reopen product data");
    assert_eq!(
        (
            inserted.revision(),
            updated.revision(),
            deleted.revision(),
            reopened
                .dictionary("dictionary.entries")
                .expect("persisted dictionary")
                .entries(),
        ),
        (2, 3, 4, &[][..])
    );
}

#[test]
fn dictionary_preserves_keep_text_context_and_explicit_unchanged_font() {
    let root = tempdir().expect("temporary product data");
    let mut backend = DesktopBackend::open(root.path()).expect("open empty V2 data");
    backend
        .create_dictionary(
            DictionaryCreate::new("dictionary.context", "语境字体词典", "zh-CN")
                .with_default_font(DictionaryDefaultFont::substitute("Default Sans"))
                .with_entries([DictionaryRuleCreate::keep("main-ui", "Radius")
                    .with_context("tool", "blur")
                    .with_font(DictionaryEntryFont::Unchanged)
                    .for_adapters(["example.synthetic.inline"])]),
        )
        .expect("create context dictionary");
    drop(backend);

    let reopened = DesktopBackend::open(root.path()).expect("reopen product data");
    let rule = &reopened
        .dictionary("dictionary.context")
        .expect("context dictionary")
        .entries()[0];
    assert_eq!(
        (
            rule.translation(),
            rule.context()
                .map(|context| (context.kind(), context.key())),
            rule.font(),
            rule.adapter_ids(),
        ),
        (
            None,
            Some(("tool", "blur")),
            &DictionaryEntryFont::Unchanged,
            &["example.synthetic.inline".into()][..],
        )
    );
}

#[test]
fn workflow_update_uses_revision_cas_and_copy_has_independent_identity() {
    let root = tempdir().expect("temporary product data");
    let selection_root = tempdir().expect("temporary executable selection");
    let executable = selection_root.path().join("WorkflowEditor.exe");
    fs::write(&executable, b"synthetic executable identity").expect("selected executable");
    let mut backend = DesktopBackend::open(root.path()).expect("open empty V2 data");
    let software_id = backend
        .add_software(ExecutableSelection::new(&executable))
        .expect("add software")
        .software()[0]
        .id()
        .to_owned();
    backend
        .create_dictionary(
            DictionaryCreate::new("dictionary.workflow", "工作流词典", "zh-CN")
                .with_entries([DictionaryRuleCreate::replace("main-ui", "Open", "打开")]),
        )
        .expect("create dictionary");
    let created = backend
        .create_workflow(
            WorkflowCreate::new("workflow.editable", "可编辑工作流")
                .with_description("默认创作组合")
                .with_targets([WorkflowTargetCreate::new(
                    software_id.as_str(),
                    ["dictionary.workflow"],
                )]),
        )
        .expect("create workflow");
    backend
        .enable_workflow(created.id())
        .expect("enable workflow");

    let updated = backend
        .update_workflow(
            WorkflowEdit::new(created.id(), "已更新工作流", created.revision())
                .with_description("更新后的创作组合")
                .with_targets([WorkflowTargetCreate::new(
                    software_id.as_str(),
                    ["dictionary.workflow"],
                )]),
        )
        .expect("update enabled workflow");
    assert_eq!(
        backend.update_workflow(
            WorkflowEdit::new(created.id(), "陈旧更新", created.revision()).with_targets([
                WorkflowTargetCreate::new(software_id.as_str(), ["dictionary.workflow"]),
            ]),
        ),
        Err(BackendError::RevisionConflict { current: 2 })
    );
    let copied = backend
        .copy_workflow(updated.id(), "workflow.copy", "工作流副本")
        .expect("copy workflow definition");
    assert_eq!(updated.description(), "更新后的创作组合");
    assert_eq!(copied.description(), "更新后的创作组合");
    drop(backend);

    let reopened = DesktopBackend::open(root.path()).expect("reopen product data");
    assert_eq!(
        (
            reopened
                .workflow("workflow.editable")
                .expect("updated workflow"),
            copied.id(),
            copied.name(),
            copied.revision(),
            reopened.enabled_workflow_ids(),
        ),
        (
            updated,
            "workflow.copy",
            "工作流副本",
            1,
            &["workflow.editable".into()][..],
        )
    );
}

#[test]
fn workflow_activation_validates_then_atomically_replaces_software_ownership() {
    let root = tempdir().expect("temporary product data");
    let selection_root = tempdir().expect("temporary executable selection");
    let executable = selection_root.path().join("SyntheticEditor.exe");
    fs::write(&executable, b"synthetic executable identity").expect("selected executable");
    let mut backend = DesktopBackend::open(root.path()).expect("open empty V2 data");
    let software_id = backend
        .add_software(ExecutableSelection::new(&executable))
        .expect("add software")
        .software()[0]
        .id()
        .to_owned();
    backend
        .create_dictionary(
            DictionaryCreate::new("dictionary.zh-cn", "界面词典", "zh-CN")
                .with_entries([DictionaryRuleCreate::replace("main-ui", "Open", "打开")]),
        )
        .expect("create dictionary");
    for workflow_id in ["workflow.primary", "workflow.alternate"] {
        backend
            .create_workflow(WorkflowCreate::new(workflow_id, workflow_id).with_targets([
                WorkflowTargetCreate::new(software_id.as_str(), ["dictionary.zh-cn"]),
            ]))
            .expect("create workflow");
    }

    backend
        .enable_workflow("workflow.primary")
        .expect("enable the first workflow");
    let active_dictionary = backend
        .dictionary("dictionary.zh-cn")
        .expect("active dictionary")
        .clone();
    assert_eq!(
        backend.update_dictionary(
            DictionaryEdit::new(
                active_dictionary.id(),
                active_dictionary.name(),
                "en-US",
                active_dictionary.revision(),
            )
            .with_entries([DictionaryRuleCreate::replace("main-ui", "Open", "Open",)]),
        ),
        Err(BackendError::WorkflowRejected(
            ResolveError::LocaleMismatch {
                software_id: software_id.clone().into(),
                dictionary_id: "dictionary.zh-cn".into(),
            }
        ))
    );
    assert_eq!(
        backend
            .dictionary("dictionary.zh-cn")
            .expect("last valid dictionary")
            .revision(),
        1
    );
    let conflict = backend.enable_workflow("workflow.alternate");
    assert_eq!(
        conflict,
        Err(BackendError::SoftwareOccupied {
            software_id: software_id.clone().into(),
            workflow_id: "workflow.primary".into(),
        })
    );
    assert_eq!(backend.enabled_workflow_ids(), &["workflow.primary".into()]);

    backend
        .replace_workflow_activation("workflow.alternate")
        .expect("explicitly replace the conflicting intent");
    drop(backend);

    let reopened = DesktopBackend::open(root.path()).expect("reopen product data");
    assert_eq!(
        reopened.enabled_workflow_ids(),
        &["workflow.alternate".into()]
    );
    assert_eq!(
        reopened
            .workflow("workflow.alternate")
            .expect("persisted workflow")
            .targets()[0]
            .dictionary_ids(),
        &["dictionary.zh-cn".into()]
    );
}

#[test]
fn workflow_activation_preserves_the_empty_dictionary_rejection_reason() {
    let root = tempdir().expect("temporary product data");
    let selection_root = tempdir().expect("temporary executable selection");
    let executable = selection_root.path().join("EmptyDictionaryEditor.exe");
    fs::write(&executable, b"synthetic executable identity").expect("selected executable");
    let mut backend = DesktopBackend::open(root.path()).expect("open empty V2 data");
    let software_id = backend
        .add_software(ExecutableSelection::new(&executable))
        .expect("add software")
        .software()[0]
        .id()
        .to_owned();
    backend
        .create_dictionary(DictionaryCreate::new("dictionary.empty", "空词典", "zh-CN"))
        .expect("create empty dictionary");
    backend
        .create_workflow(
            WorkflowCreate::new("workflow.empty", "空词典工作流").with_targets([
                WorkflowTargetCreate::new(software_id.as_str(), ["dictionary.empty"]),
            ]),
        )
        .expect("create workflow");

    assert_eq!(
        backend.enable_workflow("workflow.empty"),
        Err(BackendError::WorkflowRejected(
            ResolveError::NoEffectiveRules {
                software_id: software_id.into(),
            },
        ))
    );
    assert!(backend.enabled_workflow_ids().is_empty());
}

#[test]
fn dictionary_definition_update_is_atomic_and_revision_checked() {
    let root = tempdir().expect("temporary product data");
    let mut backend = DesktopBackend::open(root.path()).expect("open empty V2 data");
    let created = backend
        .create_dictionary(
            DictionaryCreate::new("dictionary.shared", "共享词典", "zh-CN")
                .with_entries([DictionaryRuleCreate::replace("main-ui", "Open", "打开")]),
        )
        .expect("create dictionary");

    let updated = backend
        .update_dictionary(
            DictionaryEdit::new(created.id(), "共享界面词典", "zh-CN", created.revision())
                .with_default_font(DictionaryDefaultFont::substitute("Shared Sans"))
                .with_entries([DictionaryRuleCreate::replace("main-ui", "Save", "保存")
                    .with_font(DictionaryEntryFont::Unchanged)]),
        )
        .expect("replace the dictionary definition");
    let stale = backend.update_dictionary(
        DictionaryEdit::new(created.id(), "陈旧写入", "zh-CN", created.revision())
            .with_entries([DictionaryRuleCreate::replace("main-ui", "Close", "关闭")]),
    );
    drop(backend);

    let reopened = DesktopBackend::open(root.path()).expect("reopen product data");
    let persisted = reopened
        .dictionary(created.id())
        .expect("updated dictionary");
    assert_eq!(stale, Err(BackendError::RevisionConflict { current: 2 }));
    assert_eq!(
        (
            &updated,
            persisted.name(),
            persisted.revision(),
            persisted.entries()[0].source(),
            persisted.entries()[0].font(),
        ),
        (
            persisted,
            "共享界面词典",
            2,
            "Save",
            &DictionaryEntryFont::Unchanged,
        )
    );
}

#[test]
fn referenced_assets_are_protected_until_the_workflow_is_disabled_and_deleted() {
    let root = tempdir().expect("temporary product data");
    let selection_root = tempdir().expect("temporary executable selection");
    let executable = selection_root.path().join("ProtectedEditor.exe");
    fs::write(&executable, b"synthetic executable identity").expect("selected executable");
    let mut backend = DesktopBackend::open(root.path()).expect("open empty V2 data");
    let software_id = backend
        .add_software(ExecutableSelection::new(&executable))
        .expect("add software")
        .software()[0]
        .id()
        .to_owned();
    backend
        .create_dictionary(
            DictionaryCreate::new("dictionary.protected", "受保护词典", "zh-CN")
                .with_entries([DictionaryRuleCreate::replace("main-ui", "Open", "打开")]),
        )
        .expect("create dictionary");
    backend
        .create_workflow(
            WorkflowCreate::new("workflow.protected", "受保护工作流").with_targets([
                WorkflowTargetCreate::new(software_id.as_str(), ["dictionary.protected"]),
            ]),
        )
        .expect("create workflow");
    backend
        .enable_workflow("workflow.protected")
        .expect("enable workflow");

    assert_eq!(
        backend.delete_dictionaries(["dictionary.protected"]),
        Err(BackendError::DictionaryReferenced {
            dictionary_id: "dictionary.protected".into(),
            workflow_ids: vec!["workflow.protected".into()],
        })
    );
    assert_eq!(
        backend.remove_software(&software_id),
        Err(BackendError::SoftwareReferenced {
            software_id: software_id.clone().into(),
            workflow_ids: vec!["workflow.protected".into()],
        })
    );
    assert_eq!(
        backend.delete_workflows(["workflow.protected"]),
        Err(BackendError::WorkflowEnabled("workflow.protected".into()))
    );

    backend
        .disable_workflow("workflow.protected")
        .expect("disable workflow");
    backend
        .delete_workflows(["workflow.protected"])
        .expect("delete workflow");
    backend
        .delete_dictionaries(["dictionary.protected"])
        .expect("delete dictionary");
    backend
        .remove_software(&software_id)
        .expect("delete software");
    drop(backend);

    let reopened = DesktopBackend::open(root.path()).expect("reopen product data");
    assert!(matches!(
        reopened.dictionary("dictionary.protected"),
        Err(BackendError::UnknownDictionary(_))
    ));
    assert!(matches!(
        reopened.workflow("workflow.protected"),
        Err(BackendError::UnknownWorkflow(_))
    ));
    assert!(reopened.snapshot().software().is_empty());
}

#[test]
fn selected_executable_keeps_local_path_and_editable_version_identity() {
    let root = tempdir().expect("temporary product data");
    let selection_root = tempdir().expect("temporary executable selection");
    let executable = selection_root.path().join("MotionCanvas.exe");
    fs::write(&executable, b"synthetic executable identity").expect("selected executable");
    let mut backend = DesktopBackend::open(root.path()).expect("open empty V2 data");

    let added = backend
        .add_software(ExecutableSelection::new(&executable))
        .expect("add software");
    assert_eq!(added.software().len(), 1);
    assert_eq!(added.selected_software_id(), Some(added.software()[0].id()));
    assert_eq!(added.software()[0].name(), "MotionCanvas");
    assert_eq!(added.software()[0].executable_name(), "MotionCanvas.exe");
    assert_eq!(added.software()[0].executable_path(), executable.to_str());
    assert_eq!(added.software()[0].version(), "待检测");
    assert!(!added.software()[0].translation_capability().enabled());
    let runtime = backend
        .runtime_spec(added.software()[0].id())
        .expect("generic executable runtime spec");
    assert_eq!(runtime.executable_names()[0].as_ref(), "MotionCanvas.exe");
    assert_eq!(
        runtime.executable_paths()[0].as_ref(),
        executable.to_str().unwrap()
    );
    assert!(runtime.requirements().is_empty());
    assert_eq!(runtime.publication().route().operators().len(), 1);

    let versioned_executable = selection_root.path().join("MotionCanvas2026.exe");
    fs::write(&versioned_executable, b"new executable identity").expect("versioned executable");
    let updated = backend
        .update_software(
            SoftwareEdit::new(
                added.software()[0].id(),
                "MotionCanvas 2026",
                &versioned_executable,
            )
            .with_description("用于动态图形项目"),
        )
        .expect("update local software identity");
    assert_eq!(updated.software()[0].name(), "MotionCanvas 2026");
    assert_eq!(updated.software()[0].description(), "用于动态图形项目");
    assert_eq!(
        updated.software()[0].executable_name(),
        "MotionCanvas2026.exe"
    );
    assert_eq!(
        backend
            .runtime_spec(updated.software()[0].id())
            .expect("updated Runtime spec")
            .executable_paths()[0]
            .as_ref(),
        versioned_executable.to_str().unwrap()
    );

    let reopened = DesktopBackend::open(root.path()).expect("reopen product data");
    assert_eq!(reopened.snapshot(), updated);
}

#[test]
fn software_registration_rejects_non_executables() {
    let root = tempdir().expect("temporary product data");
    let selection_root = tempdir().expect("temporary executable selection");
    let document = selection_root.path().join("notes.txt");
    fs::write(&document, b"not executable").expect("selected document");
    let mut backend = DesktopBackend::open(root.path()).expect("open empty V2 data");

    assert!(backend
        .add_software(ExecutableSelection::new(document))
        .is_err());
    assert!(backend.snapshot().software().is_empty());
}

#[test]
fn software_removal_deletes_only_glyphshift_owned_records_and_updates_selection() {
    let root = tempdir().expect("temporary product data");
    let selection_root = tempdir().expect("temporary executable selection");
    let first = selection_root.path().join("MotionCanvas.exe");
    let second = selection_root.path().join("VectorStudio.exe");
    fs::write(&first, b"first synthetic executable").expect("first executable");
    fs::write(&second, b"second synthetic executable").expect("second executable");
    let mut backend = DesktopBackend::open(root.path()).expect("open empty V2 data");
    let first_snapshot = backend
        .add_software(ExecutableSelection::new(&first))
        .expect("add first software");
    let first_id = first_snapshot.software()[0].id().to_owned();
    let second_snapshot = backend
        .add_software(ExecutableSelection::new(&second))
        .expect("add second software");
    let second_id = second_snapshot
        .software()
        .iter()
        .find(|software| software.id() != first_id)
        .expect("second software")
        .id()
        .to_owned();

    let removed = backend
        .remove_software(&second_id)
        .expect("remove selected software");
    assert_eq!(removed.software().len(), 1);
    assert_eq!(removed.selected_software_id(), Some(first_id.as_str()));
    assert!(
        first.is_file(),
        "removing a record must not delete the user executable"
    );
    assert!(
        second.is_file(),
        "removing a record must not delete the user executable"
    );

    let reopened = DesktopBackend::open(root.path()).expect("reopen persisted V2 data");
    assert_eq!(reopened.snapshot(), removed);
}
