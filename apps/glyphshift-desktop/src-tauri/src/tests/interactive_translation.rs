use super::*;
use crate::interactive_translation::InteractiveTranslationArmRequest;

#[test]
fn selected_dictionary_translation_preserves_the_acquired_source_contract() {
    let (mut application, calls, software_id, _root) = workflow_application();
    let result = application
        .run_interactive_translation_request(
            &InteractiveTranslationArmRequest::new(software_id.clone(), "dictionary.product"),
            DesktopPoint::new(-20, 30),
            &DesktopAcquisitionCancellation::new(),
        )
        .expect("interactive translation result");

    assert_eq!(
        calls.lock().expect("runtime call log").point_acquisitions,
        vec![(
            software_id,
            1,
            Box::<str>::from("windows.uia.acquire"),
            DesktopPoint::new(-20, 30),
        )]
    );
    let value = serde_json::to_value(result).expect("serialize translation result");
    assert_eq!(value["blocks"][0]["source"], "Open");
    assert_eq!(value["blocks"][0]["translation"], "打开");
    assert_eq!(value["blocks"][0]["translationState"], "translated");
    assert_eq!(value["blocks"][0]["origin"], "dictionary");
    assert_eq!(value["partial"], false);
    let encoded = value.to_string().to_ascii_lowercase();
    for forbidden in ["pid", "grant", "token", "path", "worker", "executable"] {
        assert!(!encoded.contains(forbidden));
    }
}

#[test]
fn dictionary_miss_keeps_the_original_text_and_marks_translation_missing() {
    let (mut application, _calls, software_id, _root) = workflow_application();
    application
        .backend
        .create_dictionary(DictionaryCreate::new(
            "dictionary.empty",
            "空词典",
            "en-US",
            "zh-CN",
        ))
        .expect("create empty dictionary");

    let result = application
        .run_interactive_translation_request(
            &InteractiveTranslationArmRequest::new(software_id, "dictionary.empty"),
            DesktopPoint::new(10, 20),
            &DesktopAcquisitionCancellation::new(),
        )
        .expect("missing translation result");

    let value = serde_json::to_value(result).expect("serialize translation result");
    assert_eq!(value["blocks"][0]["source"], "Open");
    assert!(value["blocks"][0].get("translation").is_none());
    assert_eq!(value["blocks"][0]["translationState"], "missing");
    assert!(value["blocks"][0].get("origin").is_none());
    assert_eq!(value["partial"], true);
}

#[test]
fn pre_cancelled_translation_fails_with_the_acquisition_cancellation_code() {
    let (mut application, _calls, software_id, _root) = workflow_application();
    let cancellation = DesktopAcquisitionCancellation::new();
    cancellation.cancel();

    assert_eq!(
        application.run_interactive_translation_request(
            &InteractiveTranslationArmRequest::new(software_id, "dictionary.product"),
            DesktopPoint::new(10, 20),
            &cancellation,
        ),
        Err(CommandError::new("acquisition.cancelled"))
    );
}
