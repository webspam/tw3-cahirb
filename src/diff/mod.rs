mod normalize;
pub use normalize::NormalizedBlob;

mod report;
pub use report::BlobDiff;

use crate::parser::RsBlob;

pub fn diff_blobs(expected: &RsBlob, actual: &RsBlob) -> BlobDiff {
    BlobDiff::new(
        &NormalizedBlob::from_blob(expected),
        &NormalizedBlob::from_blob(actual),
    )
}
