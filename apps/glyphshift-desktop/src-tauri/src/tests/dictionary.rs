use super::*;

#[test]
fn dictionary_distribution_errors_keep_stable_product_codes() {
    let cases = [
        (
            DictionaryDistributionError::CatalogUnavailable,
            "dictionary.catalog_unavailable",
        ),
        (
            DictionaryDistributionError::ReleaseMissing,
            "dictionary.release_missing",
        ),
        (
            DictionaryDistributionError::DigestMismatch,
            "dictionary.artifact_digest_mismatch",
        ),
        (
            DictionaryDistributionError::InvalidSignature,
            "dictionary.signature_invalid",
        ),
        (
            DictionaryDistributionError::UntrustedPublisher,
            "dictionary.publisher_untrusted",
        ),
        (
            DictionaryDistributionError::InvalidPayload,
            "dictionary.payload_invalid",
        ),
        (
            DictionaryDistributionError::LocalChangesConflict,
            "dictionary.local_changes_conflict",
        ),
        (
            DictionaryDistributionError::StorageFailure,
            "dictionary.installation_storage_failure",
        ),
    ];
    for (error, expected_code) in cases {
        let value = serde_json::to_value(dictionary_distribution_error(error))
            .expect("serialize command error");
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["code"], expected_code);
    }
}

#[test]
fn unconfigured_catalog_reports_offline_without_affecting_the_local_library() {
    let (mut application, _calls, _software_id, _data_root) = workflow_application();
    let before = application.snapshot().configuration.dictionaries().to_vec();

    let error = application
        .query_dictionary_catalog(DictionaryCatalogQueryRequest {
            text: "menu".into(),
            source_locale: Some("en-US".into()),
            target_locale: Some("zh-CN".into()),
            tag: None,
            cursor: None,
            page_size: Some(20),
            requested_presentation_locale: "zh-CN".into(),
        })
        .expect_err("catalog remains explicitly offline without configuration");
    let value = serde_json::to_value(error).expect("serialize command error");

    assert_eq!(value["code"], "dictionary.catalog_unavailable");
    assert_eq!(application.snapshot().configuration.dictionaries(), before);
}

#[test]
fn catalog_query_and_install_return_presentation_and_refreshed_installation_summary() {
    let (mut application, _calls, _software_id, data_root) = workflow_application();
    application.dictionary_distribution = fixture_dictionary_distribution(data_root.path());

    let page = application
        .query_dictionary_catalog(DictionaryCatalogQueryRequest {
            text: "Catalog".into(),
            source_locale: Some("en-US".into()),
            target_locale: Some("zh-CN".into()),
            tag: Some("menus".into()),
            cursor: None,
            page_size: Some(20),
            requested_presentation_locale: "zh-CN".into(),
        })
        .expect("query configured catalog");
    assert_eq!(page.releases.len(), 1);
    assert_eq!(page.releases[0].name.as_ref(), "目录词典");
    assert_eq!(
        page.releases[0].effective_presentation_locale.as_ref(),
        "zh-CN"
    );

    let snapshot = application
        .install_dictionary_release(DictionaryCatalogInstallRequest {
            catalog_id: "glyphshift.official".into(),
            dictionary_id: "dictionary.catalog".into(),
            release_version: "1.2.0".into(),
            replacement: DictionaryReplacementRequest::RejectExisting,
        })
        .expect("install catalog release");
    let dictionary = snapshot
        .configuration
        .dictionaries()
        .iter()
        .find(|dictionary| dictionary.id() == "dictionary.catalog")
        .expect("installed dictionary summary");
    assert_eq!(dictionary.installation().state(), "verified");
    assert_eq!(
        dictionary.installation().verified_publisher(),
        Some("publisher.example")
    );
}

#[test]
fn dictionary_file_exchange_uses_one_portable_json_without_a_publish_service() {
    let (mut application, _calls, _software_id, data_root) = workflow_application();
    let input = data_root.path().join("portable-import.json");
    let output = data_root.path().join("portable-export.json");
    let package = glyphshift_dictionary_package::DictionaryPackage::create(
        glyphshift_dictionary_package::DictionaryCreate::new(
            "dictionary.exchange",
            "Exchange Dictionary",
            "en-US",
            "zh-CN",
        )
        .with_release_version("1.0.0")
        .with_entries([glyphshift_dictionary_package::DictionaryEntryCreate::new(
            "Save", "保存",
        )]),
    )
    .expect("dictionary package");
    fs::write(&input, package.encode_json().expect("encode dictionary"))
        .expect("write import fixture");

    let snapshot = application
        .import_dictionary_file(input)
        .expect("import dictionary file");
    assert!(snapshot
        .configuration
        .dictionaries()
        .iter()
        .any(|dictionary| dictionary.id() == "dictionary.exchange"));
    application
        .export_dictionary_file("dictionary.exchange", output.clone())
        .expect("export dictionary file");
    let exported = fs::read_to_string(output).expect("read export");
    let reopened = glyphshift_dictionary_package::DictionaryPackage::decode_json(
        &exported,
        Some("dictionary.exchange"),
    )
    .expect("portable export");
    assert_eq!(reopened.view().entries()[0].translation(), "保存");
}

#[test]
fn dictionary_file_exchange_errors_keep_stable_product_codes() {
    let duplicate = serde_json::to_value(dictionary_import_error(
        BackendError::DuplicateDictionary("dictionary.duplicate".into()),
    ))
    .expect("serialize duplicate error");
    let invalid = serde_json::to_value(dictionary_import_error(BackendError::InvalidArtifact(
        "dictionary-import-json",
    )))
    .expect("serialize invalid error");
    let missing = serde_json::to_value(dictionary_export_error(BackendError::UnknownDictionary(
        "dictionary.missing".into(),
    )))
    .expect("serialize missing error");

    assert_eq!(duplicate["code"], "dictionary.import_duplicate");
    assert_eq!(duplicate["args"]["dictionaryId"], "dictionary.duplicate");
    assert_eq!(invalid["code"], "dictionary.import_invalid");
    assert_eq!(missing["code"], "dictionary.not_found");
}

#[test]
fn dictionary_and_workflow_details_are_loaded_by_product_id() {
    let (application, _calls, software_id, _data_root) = workflow_application();

    let dictionary = application
        .dictionary_detail("dictionary.product")
        .expect("load dictionary detail");
    let workflow = application
        .workflow_detail("workflow.product")
        .expect("load workflow detail");

    assert_eq!(dictionary.id(), "dictionary.product");
    assert_eq!(dictionary.entries()[0].source(), "Open");
    assert_eq!(dictionary.entries()[0].translation(), "打开");
    assert_eq!(workflow.id(), "workflow.product");
    assert_eq!(workflow.targets()[0].software_id(), software_id.as_ref());
    assert_eq!(
        workflow.targets()[0].dictionary_ids(),
        &[Box::<str>::from("dictionary.product")]
    );
}

#[test]
fn dictionary_delete_reports_the_workflows_and_probes_that_reference_it() {
    let (mut application, _calls, software_id, _data_root) = workflow_application();
    application
        .create_probe_run(ProbeRunCreateRequest {
            id: "probe.dictionary-reference".into(),
            name: "词典定位探针".into(),
            software_id,
            adapter_ids: vec![TEST_ADAPTER_ID.into()],
            live_preview_enabled: false,
            dictionary: ProbeDictionaryBindingRequest::Existing {
                dictionary_id: "dictionary.product".into(),
            },
        })
        .expect("create a probe that references the dictionary");

    let error = application
        .delete_dictionaries(&[Box::<str>::from("dictionary.product")])
        .expect_err("referenced dictionary must be preserved");
    let serialized = serde_json::to_value(error).expect("serialize dictionary reference error");

    assert_eq!(serialized["code"], "dictionary.referenced");
    assert_eq!(
        serialized["args"]["workflowNames"],
        serde_json::json!(["产品工作流"])
    );
    assert_eq!(
        serialized["args"]["probeNames"],
        serde_json::json!(["词典定位探针"])
    );
}

#[test]
fn dictionary_crud_reconciles_every_enabled_workflow_that_uses_the_dictionary() {
    let (mut application, _calls, _software_id, _data_root) = workflow_application();
    let created = application
        .create_dictionary(
            DictionaryCreate::new("dictionary.secondary", "备用词典", "en-US", "zh-CN")
                .with_entries([DictionaryEntryCreate::new("Close", "关闭")]),
        )
        .expect("create dictionary");
    assert_eq!(
        created
            .configuration
            .dictionaries()
            .iter()
            .map(|dictionary| (dictionary.id(), dictionary.entry_count()))
            .collect::<Vec<_>>(),
        vec![("dictionary.product", 1), ("dictionary.secondary", 1)]
    );
    let enabled = application
        .enable_workflow("workflow.product", false)
        .expect("enable workflow");
    let previous_generation = enabled.runtime.targets[0]
        .applied_generation
        .expect("initial generation");

    let updated = application
        .update_dictionary(
            DictionaryEdit::new("dictionary.product", "产品词典 2", "en-US", "zh-CN", 1)
                .with_entries([DictionaryEntryCreate::new("Open", "开启")]),
        )
        .expect("update active dictionary");

    assert_eq!(
        updated
            .configuration
            .dictionaries()
            .iter()
            .find(|dictionary| dictionary.id() == "dictionary.product")
            .map(|dictionary| (dictionary.name(), dictionary.revision())),
        Some(("产品词典 2", 2))
    );
    assert!(
        updated.workflow_runtime_status["workflow.product"].targets[0]
            .applied_generation
            .is_some_and(|generation| generation > previous_generation)
    );

    let deleted = application
        .delete_dictionaries(&[Box::<str>::from("dictionary.secondary")])
        .expect("delete unreferenced dictionary");
    assert_eq!(
        deleted
            .configuration
            .dictionaries()
            .iter()
            .map(|dictionary| dictionary.id())
            .collect::<Vec<_>>(),
        vec!["dictionary.product"]
    );
}
