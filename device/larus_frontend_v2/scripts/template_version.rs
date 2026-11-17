// this file was generated

use corelib::{HwVersion, SwVersion};

pub const SW_VERSION: SwVersion = SwVersion {
    version: [0xff, 0xff, 0xff, 0xff],
};

#[allow(unused)]
pub const GIT_COMIT_HASH: &str = "@git_hash@";

#[allow(unused)]
pub const GIT_COMMIT_TIME: &str = "@git_time@";

#[allow(unused)]
pub const GIT_TAG_INFO: &str = "@git_tag@";

// The hardware version identifies the display variant. The SD card update only accepts an
// image with the same manufacturer.major.minor (corelib HwVersion::is_compatible), so an image
// for the other display is ignored. 1.3.2.x = small round series display,
// 1.3.3.x = large 2.76" display (feature "display_large"). Keep in sync with make_variant.sh.
#[cfg(not(feature = "display_large"))]
pub const HW_VERSION: HwVersion = HwVersion {
    version: [1, 3, 2, 0],
};

#[cfg(feature = "display_large")]
pub const HW_VERSION: HwVersion = HwVersion {
    version: [1, 3, 3, 0],
};

/// Marker inside the firmware image: "LARUS-HW-ID:" followed by the hardware version.
/// scripts/pack.py reads it from the built binary and refuses to pack the image with a
/// different hardware version in its header - so the file name and header can never claim
/// another display variant than the code that is actually inside.
pub static LARUS_HW_ID: [u8; 16] = [
    b'L', b'A', b'R', b'U', b'S', b'-', b'H', b'W', b'-', b'I', b'D', b':',
    HW_VERSION.version[0],
    HW_VERSION.version[1],
    HW_VERSION.version[2],
    HW_VERSION.version[3],
];

/// Hardware version read from the marker (volatile, so the marker always stays in the image)
pub fn hw_version_from_marker() -> HwVersion {
    let id = unsafe { core::ptr::read_volatile(&LARUS_HW_ID) };
    HwVersion {
        version: [id[12], id[13], id[14], id[15]],
    }
}
