//! Reproduces the chunk explosion seen in large applications: many splits
//! that pairwise share tiny symbols. See `build.rs` for the shape of the
//! generated code and `repro.sh` for splitting it and counting the output.

use wasm_split_helpers::wasm_split;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

#[inline(never)]
fn count_upper(text: &str) -> usize {
    text.chars().filter(|c| c.is_ascii_uppercase()).count()
}

#[cfg(test)]
mod tests {
    #[cfg(not(target_family = "wasm"))]
    use tokio::test;
    #[cfg(target_family = "wasm")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    #[test]
    pub async fn every_split_computes_its_share() {
        let results = crate::call_all().await;
        assert_eq!(results.len(), crate::SPLITS);
        for (i, (actual, expected)) in results.iter().zip(crate::EXPECTED).enumerate() {
            assert_eq!(*actual, expected, "split_{i} returned the wrong sum");
        }
    }
}
