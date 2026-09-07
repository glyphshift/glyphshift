//! Architecture-neutral build metadata. The target loader still checks the real native ABI.
use super::*;
use glyphshift_adapter_native_abi::{FixedUtf8, NativeAdapterDescriptorV1};
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, SeekFrom};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeAdapterMetadata {
    pub adapter_id: String,
    pub version: [u16; 3],
    pub abi: [u16; 2],
    pub apply_model: u32,
    pub placement: u32,
    pub feature_bits: u64,
    pub platform_bits: u32,
    pub architecture_bits: u32,
    pub source_policy: u32,
}
impl NativeAdapterMetadata {
    /// # Safety
    /// The caller has already verified that this is a trusted, same-architecture artifact.
    pub unsafe fn inspect(path: &Path) -> Result<Self, NativeHostError> {
        let (library, api, _) = open_package(path)?;
        let d = api.descriptor;
        let policy = read_source_policy(&library)?;
        Ok(Self {
            adapter_id: d
                .adapter_id
                .as_str()
                .map_err(|_| NativeHostError::LoadFailed)?
                .into(),
            version: [d.version_major, d.version_minor, d.version_patch],
            abi: [d.abi_major, d.abi_minor],
            apply_model: d.apply_model,
            placement: d.placement,
            feature_bits: d.feature_bits,
            platform_bits: d.platform_bits,
            architecture_bits: d.architecture_bits,
            source_policy: match policy {
                SourceTextPolicy::Exact => 0,
                SourceTextPolicy::SpacePaddedSoftWrap => 1,
            },
        })
    }
    pub fn descriptor(&self) -> Result<AdapterDescriptor, NativeHostError> {
        if self.adapter_id.is_empty() || self.adapter_id.len() > 96 {
            return Err(NativeHostError::DescriptorSizeMismatch);
        }
        NativeAdapterDescriptorV1 {
            struct_size: std::mem::size_of::<NativeAdapterDescriptorV1>() as u32,
            adapter_id: FixedUtf8::new(&self.adapter_id),
            version_major: self.version[0],
            version_minor: self.version[1],
            version_patch: self.version[2],
            abi_major: self.abi[0],
            abi_minor: self.abi[1],
            apply_model: self.apply_model,
            placement: self.placement,
            feature_bits: self.feature_bits,
            platform_bits: self.platform_bits,
            architecture_bits: self.architecture_bits,
        }
        .to_descriptor()
        .map_err(NativeHostError::InvalidDescriptor)
    }
    pub fn source_policy(&self) -> Result<SourceTextPolicy, NativeHostError> {
        SourceTextPolicy::from_code(self.source_policy).ok_or(NativeHostError::InvalidSourcePolicy)
    }
}

/// Reads only bounded PE headers; this never maps executable code into the caller.
pub fn inspect_pe_architecture(path: &Path) -> std::io::Result<&'static str> {
    let mut file = std::fs::File::open(path)?;
    pe_architecture(&mut file)
}
fn pe_architecture(file: &mut (impl Read + Seek)) -> std::io::Result<&'static str> {
    let invalid = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "unsupported PE architecture",
        )
    };
    let mut dos = [0u8; 64];
    file.read_exact(&mut dos)?;
    if &dos[..2] != b"MZ" {
        return Err(invalid());
    }
    let offset = u32::from_le_bytes(dos[60..64].try_into().unwrap());
    if !(64..=1024 * 1024).contains(&offset) {
        return Err(invalid());
    }
    file.seek(SeekFrom::Start(u64::from(offset)))?;
    let mut header = [0u8; 26];
    file.read_exact(&mut header)?;
    if &header[..4] != b"PE\0\0" {
        return Err(invalid());
    }
    let machine = u16::from_le_bytes([header[4], header[5]]);
    let magic = u16::from_le_bytes([header[24], header[25]]);
    match (machine, magic) {
        (0x14c, 0x10b) => Ok("x86"),
        (0x8664, 0x20b) => Ok("x86_64"),
        _ => Err(invalid()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn image(machine: u16, magic: u16) -> std::io::Cursor<Vec<u8>> {
        let mut bytes = vec![0; 90];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
        bytes[64..68].copy_from_slice(b"PE\0\0");
        bytes[68..70].copy_from_slice(&machine.to_le_bytes());
        bytes[88..90].copy_from_slice(&magic.to_le_bytes());
        std::io::Cursor::new(bytes)
    }
    #[test]
    fn accepts_matching_machine_and_optional_header() {
        assert_eq!(pe_architecture(&mut image(0x14c, 0x10b)).unwrap(), "x86");
        assert_eq!(
            pe_architecture(&mut image(0x8664, 0x20b)).unwrap(),
            "x86_64"
        );
    }
    #[test]
    fn rejects_mixed_and_unknown_pe() {
        assert!(pe_architecture(&mut image(0x14c, 0x20b)).is_err());
        assert!(pe_architecture(&mut image(0xaa64, 0x20b)).is_err());
        assert!(pe_architecture(&mut std::io::Cursor::new(vec![0; 10])).is_err());
    }
}
