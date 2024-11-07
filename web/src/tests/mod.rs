// src/tests/mod.rs
use crate::components::context::AppState;
use crate::components::routes::Route;
use crate::switch;

#[test]
fn test_app_compiles() {
    assert!(true);
}

#[test]
fn test_basic_state() {
    let state = AppState::default();
    assert!(state.is_loading.is_none());
}

#[test]
fn test_with_output() {
    println!("Running test with output");
    assert_eq!(2 + 2, 4);
}

// Test that route enum variants exist
#[test]
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

// Test route variants
#[test]
fn test_route_variants() {
    // Test route with parameter
    let person_route = Route::Person {
        name: "test_user".to_string(),
    };
    let shared_ep_route = Route::SharedEpisode {
        url_key: "test_key".to_string(),
    };

    // Verify parameter values
    match person_route {
        Route::Person { name } => assert_eq!(name, "test_user"),
        _ => panic!("Wrong route type"),
    }

    match shared_ep_route {
        Route::SharedEpisode { url_key } => assert_eq!(url_key, "test_key"),
        _ => panic!("Wrong route type"),
    }
}
