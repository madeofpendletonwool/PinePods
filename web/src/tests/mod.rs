use wasm_bindgen_test::*;
use crate::components::context::AppState;
use crate::pages::routes::Route;

wasm_bindgen_test_configure!(run_in_node_experimental);

#[wasm_bindgen_test]
fn test_app_compiles() {
    assert!(true);
}

#[wasm_bindgen_test]
fn test_basic_state() {
    let state = AppState::default();
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
