//! Authoritative static resource checks shared by Registry admission and online installation.
use glyphshift_dictionary_package::{DictionaryPackage, DICTIONARY_MEDIA_TYPE};
use glyphshift_plugin_package::{sha256, Package};
use serde_json::json;

pub fn inspect(kind: &str, bytes: &[u8]) -> Result<serde_json::Value, &'static str> {
    let mut report = match kind {
        "dictionary" => {
            let source = std::str::from_utf8(bytes).map_err(|_| "invalid_utf8")?;
            let package = DictionaryPackage::decode_publication_json(source, None)
                .map_err(|_| "invalid_dictionary")?;
            let view = package.view();
            let metadata = view.metadata();
            json!({"kind":kind,"packageId":package.id(),"version":metadata.release_version(),
                "mediaType":DICTIONARY_MEDIA_TYPE,"compatibility":{
                    "schema":"glyphshift.dictionary/3","sourceLocale":metadata.source_locale(),
                    "targetLocale":metadata.target_locale(),"entryCount":view.entries().len()}})
        }
        "adapter" => {
            let package = Package::from_bytes(bytes).map_err(|_| "invalid_gsp")?;
            let manifest = package.manifest();
            if manifest
                .variants
                .iter()
                .flat_map(|v| &v.adapters)
                .any(|a| a.native_metadata.adapter_id.starts_with("windows.uia."))
            {
                return Err("archive_only_adapter");
            }
            json!({"kind":kind,"packageId":manifest.package_id,
                "version":format!("{}.{}.{}",manifest.version[0],manifest.version[1],manifest.version[2]),
                "mediaType":"application/vnd.glyphshift.plugin+zip;version=1",
                "compatibility":{"schema":manifest.schema,"runtimeBundleSchema":manifest.runtime_bundle_schema,
                    "variants":manifest.variants}})
        }
        _ => return Err("unsupported_kind"),
    };
    report["schema"] = json!("glyphshift.resource-inspection/1");
    report["sha256"] = json!(sha256(bytes));
    report["size"] = json!(bytes.len());
    Ok(report)
}
