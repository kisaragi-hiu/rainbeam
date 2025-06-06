use authbeam::api::profile::read_image;
use carp::{Graph, CarpGraph};
use crate::database::Database;
use axum::{
    body::{Body, Bytes},
    extract::{Query, State},
    http::HeaderMap,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use pathbufd::pathd;

pub fn routes(database: Database) -> Router {
    Router::new()
        .route("/lang", get(langfile_request))
        .route("/lang/set", post(set_langfile_request))
        .route("/ext/image", get(external_image_request))
        .route("/carpsvg", post(render_carpgraph))
        // ...
        .with_state(database.clone())
}

#[derive(Serialize, Deserialize)]
pub struct ExternalImageQuery {
    pub img: String,
}

/// Proxy an external image
pub async fn external_image_request(
    Query(props): Query<ExternalImageQuery>,
    State(database): State<Database>,
) -> impl IntoResponse {
    let image_url = &props.img;

    if image_url.starts_with(&database.config.host) {
        return (
            [("Content-Type", "image/svg+xml")],
            Body::from(read_image(
                pathd!("{}/images", database.config.static_dir),
                "default-banner.svg".to_string(),
            )),
        );
    }

    for host in database.config.blocked_hosts {
        if image_url.starts_with(&host) {
            return (
                [("Content-Type", "image/svg+xml")],
                Body::from(read_image(
                    pathd!("{}/images", database.config.static_dir),
                    "default-banner.svg".to_string(),
                )),
            );
        }
    }

    // get profile image
    if image_url.is_empty() {
        return (
            [("Content-Type", "image/svg+xml")],
            Body::from(read_image(
                pathd!("{}/images", database.config.static_dir),
                "default-banner.svg".to_string(),
            )),
        );
    }

    let guessed_mime = mime_guess::from_path(image_url)
        .first_raw()
        .unwrap_or("application/octet-stream");

    match database.auth.http.get(image_url).send().await {
        Ok(stream) => {
            let size = stream.content_length();
            if size.unwrap_or_default() > 10485760 {
                // return defualt image (content too big)
                return (
                    [("Content-Type", "image/svg+xml")],
                    Body::from(read_image(
                        pathd!("{}/images", database.config.static_dir),
                        "default-banner.svg".to_string(),
                    )),
                );
            }

            if let Some(ct) = stream.headers().get("Content-Type") {
                let ct = ct.to_str().unwrap();
                let bad_ct = vec!["text/html", "text/plain"];
                if (!ct.starts_with("image/") && !ct.starts_with("font/")) | bad_ct.contains(&ct) {
                    // if we got html, return default banner (likely an error page)
                    return (
                        [("Content-Type", "image/svg+xml")],
                        Body::from(read_image(
                            pathd!("{}/images", database.config.static_dir),
                            "default-banner.svg".to_string(),
                        )),
                    );
                }
            }

            (
                [(
                    "Content-Type",
                    if guessed_mime == "text/html" {
                        "text/plain"
                    } else {
                        guessed_mime
                    },
                )],
                Body::from_stream(stream.bytes_stream()),
            )
        }
        Err(_) => (
            [("Content-Type", "image/svg+xml")],
            Body::from(read_image(
                pathd!("{}/images", database.config.static_dir),
                "default-banner.svg".to_string(),
            )),
        ),
    }
}

#[derive(Serialize, Deserialize)]
pub struct LangFileQuery {
    #[serde(default)]
    pub id: String,
}

/// Get a langfile
pub async fn langfile_request(
    Query(props): Query<LangFileQuery>,
    State(database): State<Database>,
) -> impl IntoResponse {
    Json(database.lang(&props.id))
}

/// Set a langfile
pub async fn set_langfile_request(Query(props): Query<LangFileQuery>) -> impl IntoResponse {
    (
        {
            let mut headers = HeaderMap::new();

            headers.insert(
                "Set-Cookie",
                format!(
                    "net.rainbeam.langs.choice={}; SameSite=Lax; Secure; Path=/; HostOnly=true; HttpOnly=true; Max-Age={}",
                    props.id,
                    60* 60 * 24 * 365
                )
                .parse()
                .unwrap(),
            );

            headers
        },
        "Language changed",
    )
}

pub async fn render_carpgraph(data: Bytes) -> impl IntoResponse {
    Graph::from_bytes(data.to_vec()).to_svg()
}
