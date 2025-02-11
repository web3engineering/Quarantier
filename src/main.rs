use axum::{
    body::Body,
    extract::State,
    http::{response, Request, StatusCode},
    response::Response,
    routing::post,
    Router,
};
use futures::future::select_all;
use reqwest::Client;
use serde_json;
use std::time::Duration;
use std::{sync::Arc};

const QUARANTINE_TOLERANCE: u64 = 7;
struct ServerConfig {
    servers: Vec<(String, Client)>,
    quarantine: tokio::sync::RwLock<Vec<String>>,
}

impl ServerConfig {
    fn new(server_urls: Vec<String>) -> Self {
        let servers = server_urls
            .into_iter()
            .map(|url| {
                // Create a persistent client for each server with custom configuration
                let client = Client::builder()
                    .timeout(Duration::from_secs(5))
                    .pool_max_idle_per_host(10) // Keep up to 10 idle connections per host
                    .pool_idle_timeout(Duration::from_secs(90))
                    .tcp_keepalive(Duration::from_secs(60))
                    .build()
                    .expect("Failed to create HTTP client");

                (url, client)
            })
            .collect();

        Self {
            servers,
            quarantine: tokio::sync::RwLock::new(Vec::new()),
        }
    }
}

async fn load_balance_handler(
    State(config): State<Arc<ServerConfig>>,
    request: Request<Body>,
) -> Result<Response<Body>, StatusCode> {
    // Clone the request body for multiple uses
    let body_bytes = axum::body::to_bytes(request.into_body(), usize::MAX)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut request_futures: Vec<_> = config
        .servers
        .iter()
        .map(|(server_url, client)| {
            let url = server_url.clone();
            let client = client.clone();
            let body = body_bytes.clone();

            client
                .post(&url)
                .body(body)
                .header("Content-Type", "application/json")
                .send()
            })
        .collect();

    let (tx, rx) = tokio::sync::oneshot::channel::<(u16, String)>();

    tokio::spawn(async move {
        let mut sender = Some(tx);
        let now = std::time::Instant::now();
        let mut recent_slots: Vec<(u64, String)> = Vec::with_capacity(request_futures.len());

        loop {
            let (response, _index, rest) = select_all(request_futures).await;
            if response.is_ok() {
                let response = response.unwrap();
                let host = response.url().clone();
                let status = response.status().as_u16();
                let body = response.text().await.unwrap();
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                    if let Some(result) = json.get("result") {
                        if result.is_object() {
                            if let Some(context) = result.get("context") {
                                if context.is_object() {
                                    if let Some(slot) = context.get("slot") {
                                        println!(
                                            "+ Slot on {} is {}",
                                            host,
                                            slot.as_u64().unwrap()
                                        );
                                        recent_slots
                                            .push((slot.as_u64().unwrap(), host.to_string()));
                                    }
                                } else {
                                    println!("Context is not an object");
                                }
                            } else {
                                println!("Context not found in result");
                            }
                        } else {
                            println!("Result is not an object");
                        }
                    } else {
                        println!("Result field not found");
                    }
                } else {
                    println!("Failed to parse response as JSON");
                }
                println!("+ Response from {} received in {:?}", host, now.elapsed());

                let quarantine = config.quarantine.read().await;
                if !quarantine.contains(&host.to_string()) {
                    if let Some(sender) = sender.take() {
                        sender.send((status, body)).unwrap();
                    }
                } else {
                    println!("+ Host {} is in quarantine, ignoring", host);
                }
            } else {
                println!("Failed to send request: {:?}", response);
            }
            if rest.is_empty() {
                if recent_slots.len() > 1 {
                    let latest_slot = recent_slots
                        .iter()
                        .max_by_key(|(slot, _host)| *slot)
                        .unwrap();
                    let slowest_hosts: Vec<String> = recent_slots
                        .iter()
                        .filter(|(slot, _host)| *slot + QUARANTINE_TOLERANCE < latest_slot.0)
                        .map(|(_slot, host)| host.clone())
                        .collect();
                    
                    let mut quarantine = config.quarantine.write().await;
                    quarantine.clear();
                    if slowest_hosts.len() > 0 {
                        println!("+ Slot {} is the latest slot", latest_slot.0);
                        println!("+ Removing slowest hosts: {:?}", slowest_hosts);
                        for host in slowest_hosts {
                            quarantine.push(host);
                        }
                    }
                }
                break;
            }
            request_futures = rest.into_iter().collect();
        }
        if let Some(sender) = sender.take() {
            sender.send((500, "No servers available".to_string())).unwrap();
        }
    });

    let (status, body) = rx.await.unwrap();

    println!("RETURNING Response status: {:?}, body: {:?}", status, body);
    let response = Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
    Ok(response)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run -- <PORT> <URL1> <URL2> ...");
        std::process::exit(1);
    }

    let port = &args[0];
    let server_urls = args[1..].to_vec();
    let server_config = Arc::new(ServerConfig::new(server_urls));

    let app = Router::new()
        .route("/", post(load_balance_handler))
        .with_state(server_config);

    let address = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("Load balancer listening on http://{}", address);

    axum::serve(listener, app).await.map_err(|e| e.into())
}
