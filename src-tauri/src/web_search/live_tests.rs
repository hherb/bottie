//! Opt-in live checks for the fixed native web-search adapters.

use super::{BraveSearchProvider, ExaSearchProvider, WebSearchProvider, connection_test_request};

const BRAVE_KEY_ENV: &str = "BOTTIE_LIVE_BRAVE_SEARCH_API_KEY";
const EXA_KEY_ENV: &str = "BOTTIE_LIVE_EXA_SEARCH_API_KEY";

#[test]
#[ignore = "requires an explicit throwaway Brave Search API key"]
fn live_brave_connection_probe_uses_the_native_adapter() {
    let api_key = std::env::var(BRAVE_KEY_ENV).expect("set the opt-in Brave test key");
    let provider = BraveSearchProvider::new(api_key).expect("build the fixed Brave adapter");
    let response = tauri::async_runtime::block_on(provider.search(connection_test_request()))
        .expect("complete one bounded Brave probe");

    assert_eq!(response.provider_id(), "brave");
    assert!(response.results().len() <= 1);
}

#[test]
#[ignore = "requires an explicit throwaway Exa Search API key"]
fn live_exa_connection_probe_uses_the_native_adapter() {
    let api_key = std::env::var(EXA_KEY_ENV).expect("set the opt-in Exa test key");
    let provider = ExaSearchProvider::new(api_key).expect("build the fixed Exa adapter");
    let response = tauri::async_runtime::block_on(provider.search(connection_test_request()))
        .expect("complete one bounded Exa probe");

    assert_eq!(response.provider_id(), "exa");
    assert!(response.results().len() <= 1);
}
