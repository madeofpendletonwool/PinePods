use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use serde::Deserialize;
use std::env;
use dotenvy::dotenv;
use std::time::{SystemTime, UNIX_EPOCH};
use sha1::{Digest, Sha1};
use log::{info, error};
use actix_cors::Cors;

#[derive(Deserialize)]
struct SearchQuery {
    query: Option<String>,
    index: Option<String>,
    search_type: Option<String>,  // Added for specifying search type
}

async fn search_handler(query: web::Query<SearchQuery>) -> impl Responder {
    println!("search_handler called");

    // Check if the query parameters are empty and return 200 OK immediately if they are
    if query.query.is_none() && query.index.is_none() {
        println!("Empty query and index - returning 200 OK");
        return HttpResponse::Ok().body("Test connection successful");
    }

    let search_term = query.query.clone().unwrap_or_default();
    let index = query.index.clone().unwrap_or_default().to_lowercase();
    let search_type = query.search_type.clone().unwrap_or_else(|| "term".to_string());

    println!("Received search request - Query: {}, Index: {}, Type: {}", search_term, index, search_type);
    println!("Searching for: {}", search_term);
    let client = reqwest::Client::new();
    println!("Client created");

    let response = if index == "itunes" {
        // iTunes Search
        let itunes_search_url = format!("https://itunes.apple.com/search?term={}&media=podcast", search_term);
        println!("Using iTunes search URL: {}", itunes_search_url);

        client.get(&itunes_search_url).send().await
    } else {
        // Determine the correct Podcast Index API endpoint based on search_type
        let api_key = match env::var("API_KEY") {
            Ok(key) => key,
            Err(_) => {
                println!("API_KEY not set in the environment");
                return HttpResponse::InternalServerError().body("API_KEY not set");
            }
        };
        let api_secret = match env::var("API_SECRET") {
            Ok(secret) => secret,
            Err(_) => {
                println!("API_SECRET not set in the environment");
                return HttpResponse::InternalServerError().body("API_SECRET not set");
            }
        };

        let encoded_search_term = urlencoding::encode(&search_term);
        let podcast_search_url = match search_type.as_str() {
            "person" => format!("https://api.podcastindex.org/api/1.0/search/byperson?q={}", encoded_search_term),
            _ => format!("https://api.podcastindex.org/api/1.0/search/byterm?q={}", encoded_search_term),
        };

        println!("Using Podcast Index search URL: {}", podcast_search_url);

        let epoch_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string();
        let data_to_hash = format!("{}{}{}", api_key, api_secret, epoch_time);

        let mut hasher = Sha1::new();
        hasher.update(data_to_hash.as_bytes());
        let sha_1 = format!("{:x}", hasher.finalize());

        let mut headers = HeaderMap::new();
        headers.insert("X-Auth-Date", HeaderValue::from_str(&epoch_time).unwrap_or_else(|e| {
            error!("Failed to insert X-Auth-Date header: {:?}", e);
            std::process::exit(1);
        }));
        headers.insert("X-Auth-Key", HeaderValue::from_str(&api_key).unwrap_or_else(|e| {
            error!("Failed to insert X-Auth-Key header: {:?}", e);
            std::process::exit(1);
        }));
        headers.insert("Authorization", HeaderValue::from_str(&sha_1).unwrap_or_else(|e| {
            error!("Failed to insert Authorization header: {:?}", e);
            std::process::exit(1);
        }));
        headers.insert(USER_AGENT, HeaderValue::from_static("MyPodcastApp/1.0")); // Use your custom User-Agent here

        println!("Final Podcast Index URL: {}", podcast_search_url);

        client.get(&podcast_search_url).headers(headers).send().await
    };

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("Request succeeded");
                match resp.text().await {
                    Ok(body) => HttpResponse::Ok().content_type("application/json").body(body),
                    Err(_) => {
                        error!("Failed to parse response body");
                        HttpResponse::InternalServerError().body("Failed to parse response body")
                    }
                }
            } else {
                error!("Request failed with status code: {}", resp.status());
                println!("Request Headers: {:?}", resp.headers());
                HttpResponse::InternalServerError().body(format!("Request failed with status code: {}", resp.status()))
            }
        }
        Err(err) => {
            error!("Request error: {:?}", err);
            HttpResponse::InternalServerError().body("Request error occurred")
        }
    }
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    env_logger::init();

    println!("Starting the Actix Web server");

    HttpServer::new(|| {
        App::new()
            .wrap(Cors::default().allow_any_origin().allow_any_method().allow_any_header())
            .route("/api/search", web::get().to(search_handler))
    })
    .bind("0.0.0.0:5000")?
    .run()
    .await
}
