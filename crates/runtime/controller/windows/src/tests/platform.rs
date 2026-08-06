use crate::platform::pe_architecture;

#[test]
fn pe_header_reports_only_the_windows_target_architectures_we_can_screen() {
    fn image(machine: u16) -> Vec<u8> {
        let mut bytes = vec![0_u8; 256];
        bytes[0..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&0x80_u32.to_le_bytes());
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
        bytes[0x84..0x86].copy_from_slice(&machine.to_le_bytes());
        bytes
    }

    assert_eq!(pe_architecture(&image(0x8664)), Some("x86_64"));
    assert_eq!(pe_architecture(&image(0x014c)), Some("x86"));
    assert_eq!(pe_architecture(&image(0xaa64)), Some("arm64"));
    assert_eq!(pe_architecture(b"not a PE image"), None);
}
