//! MIPS64 architecture support (placeholder for future)

use crate::arch::CpuFeatures;

pub fn detect_features() -> CpuFeatures {
    CpuFeatures {
        has_fpu: true,
        has_vmx: false,
        has_svm: false,
        has_sve: false,
        has_neon: false,
        has_msa: true,
        has_vector: false,
        cache_line_size: 64,
        physical_address_bits: 48,
        virtual_address_bits: 48,
    }
}
