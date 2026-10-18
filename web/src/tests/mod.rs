use wasm_bindgen_test::*;
use crate::components::context::{AppState, PageLoadState};
use crate::pages::routes::Route;

wasm_bindgen_test_configure!(run_in_node_experimental);

#[wasm_bindgen_test]
fn test_app_compiles() {
    assert!(true);
}

#[wasm_bindgen_test]
fn test_basic_state() {
    let state = PageLoadState::default();
    assert!(state.is_loading.is_none());
}

#[wasm_bindgen_test]
fn test_with_output() {
    assert_eq!(2 + 2, 4);
}

#[wasm_bindgen_test]
fn test_route_variants_exist() {
    let routes = vec![
        Route::Home,
        Route::Login,
        Route::NotFound,
        Route::Settings,
        Route::Search,
        Route::Queue,
    ];
    assert!(!routes.is_empty());
}

#[wasm_bindgen_test]
fn test_route_variants() {
    let person_route = Route::Person {
        name: "test_user".to_string(),
    };
    let shared_ep_route = Route::SharedEpisode {
        url_key: "test_key".to_string(),
    };

    match person_route {
        Route::Person { name } => assert_eq!(name, "test_user"),
        _ => panic!("Wrong route type"),
    }

    match shared_ep_route {
        Route::SharedEpisode { url_key } => assert_eq!(url_key, "test_key"),
        _ => panic!("Wrong route type"),
    }
}

#[wasm_bindgen_test]
fn test_pkce_code_challenge_s256_rfc7636_vector() {
    // Test vector from RFC 7636 Appendix B.
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    let expected_challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
    assert_eq!(
        crate::requests::login_requests::pkce_code_challenge_s256(verifier),
        expected_challenge
    );
}

#[wasm_bindgen_test]
fn test_pkce_code_challenge_s256_is_url_safe_unpadded() {
    let challenge = crate::requests::login_requests::pkce_code_challenge_s256("some-code-verifier");
    // base64url of a 32-byte digest: 43 chars, unpadded, no '+' or '/'
    assert_eq!(challenge.len(), 43);
    assert!(!challenge.contains('='));
    assert!(!challenge.contains('+'));
    assert!(!challenge.contains('/'));
}
