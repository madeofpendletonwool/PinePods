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
    search_type: Option<String>,
}

#[derive(Deserialize)]
struct PodcastQuery {
    id: String,
}

async fn search_handler(query: web::Query<SearchQuery>) -> impl Responder {
    println!("search_handler called");

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
        // Podcast Index API search
        let (api_key, api_secret) = match get_api_credentials() {
            Ok(creds) => creds,
            Err(response) => return response,
        };

        let encoded_search_term = urlencoding::encode(&search_term);
        println!("Encoded search term: {}", encoded_search_term);
        println!("Search type: {}", search_type);
        let podcast_search_url = match search_type.as_str() {
            "person" => {
                println!("Using /search/byperson endpoint");
                format!("https://api.podcastindex.org/api/1.0/search/byperson?q={}", encoded_search_term)
            },
            _ => {
                println!("Using /search/byterm endpoint");
                format!("https://api.podcastindex.org/api/1.0/search/byterm?q={}", encoded_search_term)
            },
        };

        println!("Using Podcast Index search URL: {}", podcast_search_url);

        let headers = match create_auth_headers(&api_key, &api_secret) {
            Ok(h) => h,
            Err(response) => return response,
        };

        println!("Final Podcast Index URL: {}", podcast_search_url);

        client.get(&podcast_search_url).headers(headers).send().await
    };

    handle_response(response).await
}

async fn podcast_handler(query: web::Query<PodcastQuery>) -> impl Responder {
    println!("podcast_handler called");

    let podcast_id = &query.id;
    let client = reqwest::Client::new();

    let (api_key, api_secret) = match get_api_credentials() {
        Ok(creds) => creds,
        Err(response) => return response,
    };

    let podcast_url = format!("https://api.podcastindex.org/api/1.0/podcasts/byfeedid?id={}", podcast_id);
    println!("Using Podcast Index URL: {}", podcast_url);

    let headers = match create_auth_headers(&api_key, &api_secret) {
        Ok(h) => h,
        Err(response) => return response,
    };

    let response = client.get(&podcast_url).headers(headers).send().await;
    handle_response(response).await
}

fn get_api_credentials() -> Result<(String, String), HttpResponse> {
    let api_key = match env::var("API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("API_KEY not set in the environment");
            return Err(HttpResponse::InternalServerError().body("API_KEY not set"));
        }
    };
    let api_secret = match env::var("API_SECRET") {
        Ok(secret) => secret,
        Err(_) => {
            println!("API_SECRET not set in the environment");
            return Err(HttpResponse::InternalServerError().body("API_SECRET not set"));
        }
    };
    Ok((api_key, api_secret))
}

fn create_auth_headers(api_key: &str, api_secret: &str) -> Result<HeaderMap, HttpResponse> {
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
    headers.insert("X-Auth-Key", HeaderValue::from_str(api_key).unwrap_or_else(|e| {
        error!("Failed to insert X-Auth-Key header: {:?}", e);
        std::process::exit(1);
    }));
    headers.insert("Authorization", HeaderValue::from_str(&sha_1).unwrap_or_else(|e| {
        error!("Failed to insert Authorization header: {:?}", e);
        std::process::exit(1);
    }));
    headers.insert(USER_AGENT, HeaderValue::from_static("PodPeopleDB/1.0"));

    Ok(headers)
}

async fn handle_response(response: Result<reqwest::Response, reqwest::Error>) -> HttpResponse {
    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("Request succeeded");
                match resp.text().await {
                    Ok(body) => {
                        println!("Response body: {:?}", body);
                        HttpResponse::Ok().content_type("application/json").body(body)
                    },
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
            .route("/api/podcast", web::get().to(podcast_handler))
    })
    .bind("0.0.0.0:5000")?
    .run()
    .await
}
