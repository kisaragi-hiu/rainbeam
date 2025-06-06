use rainbeam::{
    database::Database,
    model::{RelationshipStatus, Question, Reaction, FullResponse, DatabaseError},
};
use rainbeam_shared::config::Config;
use authbeam::{
    simplify,
    model::{Profile, ProfileMetadata, Notification, FinePermission, IpBan, ItemType, ItemStatus},
};
use langbeam::LangFile;

use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse},
    routing::{get, Router},
};
use axum_extra::extract::CookieJar;
use ammonia::Builder;
use reqwest::StatusCode;
use reva_axum::Template;

use crate::ToHtml;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

use super::api;

pub mod market;
pub mod models;
pub mod profile;
pub mod search;
pub mod settings;

/// Escape a username's characters if we are unable to find a "good" character
///
/// A "good" character is any alphanumeric character.
pub fn escape_username(name: &String) -> String {
    // comb through chars, if we never find anything that is actually a letter,
    // go ahead and escape
    let mut found_good: bool = false;

    for char in name.chars() {
        if char.is_alphanumeric() {
            found_good = true;
            break;
        }
    }

    if !found_good {
        return "bad username".to_string();
    }

    // return given data
    name.to_owned()
}

#[derive(Template)]
#[template(path = "error.html")]
pub struct ErrorTemplate {
    pub config: Config,
    pub lang: LangFile,
    pub profile: Option<Box<Profile>>,
    pub message: String,
}

pub async fn not_found(State(database): State<Database>) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Html(DatabaseError::NotFound.to_html(database)),
    )
}

#[derive(Template)]
#[template(path = "homepage.html")]
struct HomepageTemplate {
    config: Config,
    lang: langbeam::LangFile,
    profile: Option<Box<Profile>>,
}

#[derive(Template)]
#[template(path = "timelines/timeline.html")]
struct TimelineTemplate {
    config: Config,
    lang: langbeam::LangFile,
    profile: Option<Box<Profile>>,
    unread: usize,
    notifs: usize,
    friends: Vec<(Box<Profile>, Box<Profile>)>,
    page: i32,
}

/// GET /
pub async fn homepage_request(
    jar: CookieJar,
    State(database): State<Database>,
    Query(props): Query<PaginatedQuery>,
) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => Some(ua),
            Err(_) => None,
        },
        None => None,
    };

    // timeline
    if let Some(ref ua) = auth_user {
        let unread = database.get_inbox_count_by_recipient(&ua.id).await;

        let notifs = database
            .auth
            .get_notification_count_by_recipient(&ua.id)
            .await;

        // ...
        return Html(
            TimelineTemplate {
                config: database.config.clone(),
                lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                    c.value_trimmed()
                } else {
                    ""
                }),
                profile: auth_user.clone(),
                unread,
                notifs,
                friends: database
                    .auth
                    .get_user_participating_relationships_of_status(
                        &ua.id,
                        RelationshipStatus::Friends,
                    )
                    .await
                    .unwrap(),
                page: props.page,
            }
            .render()
            .unwrap(),
        );
    }

    // homepage
    Html(
        HomepageTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            profile: auth_user,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "partials/timelines/timeline.html")]
struct PartialTimelineTemplate {
    config: Config,
    lang: langbeam::LangFile,
    profile: Option<Box<Profile>>,
    responses: Vec<FullResponse>,
    relationships: HashMap<String, RelationshipStatus>,
    is_powerful: bool,
    is_helper: bool,
}

/// GET /_app/timelines/timeline.html
pub async fn partial_timeline_request(
    jar: CookieJar,
    State(database): State<Database>,
    Query(props): Query<PaginatedQuery>,
) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => ua,
            Err(_) => return Html(DatabaseError::NotAllowed.to_html(database)),
        },
        None => return Html(DatabaseError::NotAllowed.to_html(database)),
    };

    let responses = match database
        .get_responses_by_following_paginated(&auth_user.id, props.page)
        .await
    {
        Ok(responses) => responses,
        Err(e) => return Html(e.to_html(database)),
    };

    let mut is_helper: bool = false;
    let is_powerful = {
        let group = match database.auth.get_group_by_id(auth_user.group).await {
            Ok(g) => g,
            Err(_) => return Html(DatabaseError::Other.to_html(database)),
        };

        if group.permissions.check_helper() {
            is_helper = true
        }

        group.permissions.check_manager()
    };

    // build relationships list
    let mut relationships: HashMap<String, RelationshipStatus> = HashMap::new();

    for response in &responses {
        if relationships.contains_key(&response.1.author.id) {
            continue;
        }

        if is_helper {
            // make sure staff can view your responses
            relationships.insert(response.1.author.id.clone(), RelationshipStatus::Friends);
            continue;
        }

        if response.1.author.id == auth_user.id {
            // make sure we can view our own responses
            relationships.insert(response.1.author.id.clone(), RelationshipStatus::Friends);
            continue;
        };

        relationships.insert(
            response.1.author.id.clone(),
            database
                .auth
                .get_user_relationship(&response.1.author.id, &auth_user.id)
                .await
                .0,
        );
    }

    // ...
    return Html(
        PartialTimelineTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            profile: Some(auth_user.clone()),
            responses,
            relationships,
            is_powerful,
            is_helper,
        }
        .render()
        .unwrap(),
    );
}

#[derive(Template)]
#[template(path = "timelines/public_timeline.html")]
struct PublicTimelineTemplate {
    config: Config,
    lang: langbeam::LangFile,
    profile: Option<Box<Profile>>,
    unread: usize,
    notifs: usize,
    page: i32,
}

/// GET /public
pub async fn public_timeline_request(
    jar: CookieJar,
    State(database): State<Database>,
    Query(props): Query<PaginatedQuery>,
) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => ua,
            Err(_) => return Html(DatabaseError::NotAllowed.to_string()),
        },
        None => return Html(DatabaseError::NotAllowed.to_string()),
    };

    // timeline
    let unread = database.get_inbox_count_by_recipient(&auth_user.id).await;

    let notifs = database
        .auth
        .get_notification_count_by_recipient(&auth_user.id)
        .await;

    // ...
    return Html(
        PublicTimelineTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            profile: Some(auth_user),
            unread,
            notifs,
            page: props.page,
        }
        .render()
        .unwrap(),
    );
}

#[derive(Template)]
#[template(path = "partials/timelines/timeline.html")]
struct PartialPublicTimelineTemplate {
    config: Config,
    lang: langbeam::LangFile,
    profile: Option<Box<Profile>>,
    responses: Vec<FullResponse>,
    relationships: HashMap<String, RelationshipStatus>,
    is_powerful: bool,
    is_helper: bool,
}

/// GET /_app/timelines/public_timeline.html
pub async fn partial_public_timeline_request(
    jar: CookieJar,
    State(database): State<Database>,
    Query(props): Query<PaginatedQuery>,
) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => ua,
            Err(_) => return Html(DatabaseError::NotAllowed.to_html(database)),
        },
        None => return Html(DatabaseError::NotAllowed.to_html(database)),
    };

    let responses = match database.get_responses_paginated(props.page).await {
        Ok(responses) => responses,
        Err(e) => return Html(e.to_html(database)),
    };

    let mut is_helper: bool = false;
    let is_powerful = {
        let group = match database.auth.get_group_by_id(auth_user.group).await {
            Ok(g) => g,
            Err(_) => return Html(DatabaseError::Other.to_html(database)),
        };

        if group.permissions.check_helper() {
            is_helper = true
        }

        group.permissions.check_manager()
    };

    // build relationships list
    let mut relationships: HashMap<String, RelationshipStatus> = HashMap::new();

    for response in &responses {
        if relationships.contains_key(&response.1.author.id) {
            continue;
        }

        if is_helper {
            // make sure staff can view your responses
            relationships.insert(response.1.author.id.clone(), RelationshipStatus::Friends);
            continue;
        }

        if response.1.author.id == auth_user.id {
            // make sure we can view our own responses
            relationships.insert(response.1.author.id.clone(), RelationshipStatus::Friends);
            continue;
        };

        relationships.insert(
            response.1.author.id.clone(),
            database
                .auth
                .get_user_relationship(&response.1.author.id, &auth_user.id)
                .await
                .0,
        );
    }

    // ...
    return Html(
        PartialPublicTimelineTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            profile: Some(auth_user.clone()),
            responses,
            relationships,
            is_powerful,
            is_helper,
        }
        .render()
        .unwrap(),
    );
}

#[derive(Template)]
#[template(path = "partials/timelines/discover/responses_top.html")]
struct PartialTopResponsesTemplate {
    config: Config,
    lang: langbeam::LangFile,
    profile: Option<Box<Profile>>,
    responses: Vec<FullResponse>,
    relationships: HashMap<String, RelationshipStatus>,
    is_powerful: bool,
    is_helper: bool,
}

/// GET /_app/timelines/discover/responses_top.html
pub async fn partial_top_responses_request(
    jar: CookieJar,
    State(database): State<Database>,
) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => Some(ua),
            Err(_) => None,
        },
        None => None,
    };

    let responses = match database.get_top_reacted_responses(604_800_000).await {
        Ok(r) => r,
        Err(e) => return Html(e.to_html(database)),
    };

    let mut is_helper: bool = false;
    let is_powerful = if let Some(ref ua) = auth_user {
        let group = match database.auth.get_group_by_id(ua.group).await {
            Ok(g) => g,
            Err(_) => return Html(DatabaseError::Other.to_html(database)),
        };

        if group.permissions.check_helper() {
            is_helper = true
        }

        group.permissions.check_manager()
    } else {
        false
    };

    // build relationships list
    let mut relationships: HashMap<String, RelationshipStatus> = HashMap::new();

    if let Some(ref ua) = auth_user {
        for response in &responses {
            if relationships.contains_key(&response.1.author.id) {
                continue;
            }

            if is_helper {
                // make sure staff can view your responses
                relationships.insert(response.1.author.id.clone(), RelationshipStatus::Friends);
                continue;
            }

            if response.1.author.id == ua.id {
                // make sure we can view our own responses
                relationships.insert(response.1.author.id.clone(), RelationshipStatus::Friends);
                continue;
            };

            relationships.insert(
                response.1.author.id.clone(),
                database
                    .auth
                    .get_user_relationship(&response.1.author.id, &ua.id)
                    .await
                    .0,
            );
        }
    } else {
        // the posts timeline requires that we have an entry for every relationship,
        // since we don't have an account every single relationship should be unknown
        for response in &responses {
            if relationships.contains_key(&response.1.author.id) {
                continue;
            }

            relationships.insert(response.1.author.id.clone(), RelationshipStatus::Unknown);
        }
    }

    // ...
    return Html(
        PartialTopResponsesTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            profile: auth_user,
            responses,
            relationships,
            is_powerful,
            is_helper,
        }
        .render()
        .unwrap(),
    );
}

#[derive(Template)]
#[template(path = "partials/timelines/discover/questions_most.html")]
struct PartialTopAskersTemplate {
    lang: langbeam::LangFile,
    users: Vec<(usize, Box<Profile>)>,
}

/// GET /_app/timelines/discover/questions_most.html
pub async fn partial_top_askers_request(
    jar: CookieJar,
    State(database): State<Database>,
) -> impl IntoResponse {
    let users = match database.get_top_askers().await {
        Ok(r) => r,
        Err(e) => return Html(e.to_html(database)),
    };

    // ...
    return Html(
        PartialTopAskersTemplate {
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            users,
        }
        .render()
        .unwrap(),
    );
}

#[derive(Template)]
#[template(path = "partials/timelines/discover/responses_most.html")]
struct PartialTopRespondersTemplate {
    lang: langbeam::LangFile,
    users: Vec<(usize, Box<Profile>)>,
}

/// GET /_app/timelines/discover/responses_most.html
pub async fn partial_top_responders_request(
    jar: CookieJar,
    State(database): State<Database>,
) -> impl IntoResponse {
    let users = match database.get_top_responders().await {
        Ok(r) => r,
        Err(e) => return Html(e.to_html(database)),
    };

    // ...
    return Html(
        PartialTopRespondersTemplate {
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            users,
        }
        .render()
        .unwrap(),
    );
}

#[derive(Template)]
#[template(path = "timelines/discover.html")]
struct DiscoverTemplate {
    config: Config,
    lang: langbeam::LangFile,
    profile: Option<Box<Profile>>,
    unread: usize,
    notifs: usize,
}

/// GET /public
pub async fn discover_request(
    jar: CookieJar,
    State(database): State<Database>,
) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => ua,
            Err(_) => return Html(DatabaseError::NotAllowed.to_string()),
        },
        None => return Html(DatabaseError::NotAllowed.to_string()),
    };

    // timeline
    let unread = database.get_inbox_count_by_recipient(&auth_user.id).await;

    let notifs = database
        .auth
        .get_notification_count_by_recipient(&auth_user.id)
        .await;

    // ...
    return Html(
        DiscoverTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            profile: Some(auth_user),
            unread,
            notifs,
        }
        .render()
        .unwrap(),
    );
}

#[derive(Template)]
#[template(path = "general_markdown_text.html")]
pub struct MarkdownTemplate {
    config: Config,
    lang: langbeam::LangFile,
    profile: Option<Box<Profile>>,
    title: String,
    text: String,
}

/// GET /site/about
pub async fn about_request(jar: CookieJar, State(database): State<Database>) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => Some(ua),
            Err(_) => None,
        },
        None => None,
    };

    Html(
        MarkdownTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            profile: auth_user,
            title: "About".to_string(),
            text: rainbeam_shared::fs::read(format!(
                "{}/site/about.md",
                database.config.static_dir
            ))
            .unwrap_or(database.config.description),
        }
        .render()
        .unwrap(),
    )
}

/// GET /site/terms-of-service
pub async fn tos_request(jar: CookieJar, State(database): State<Database>) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => Some(ua),
            Err(_) => None,
        },
        None => None,
    };

    Html(
        MarkdownTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            profile: auth_user,
            title: "Terms of Service".to_string(),
            text: rainbeam_shared::fs::read(format!("{}/site/tos.md", database.config.static_dir))
                .unwrap_or(String::new()),
        }
        .render()
        .unwrap(),
    )
}

/// GET /site/privacy
pub async fn privacy_request(
    jar: CookieJar,
    State(database): State<Database>,
) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => Some(ua),
            Err(_) => None,
        },
        None => None,
    };

    Html(
        MarkdownTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            profile: auth_user,
            title: "Privacy Policy".to_string(),
            text: rainbeam_shared::fs::read(format!(
                "{}/site/privacy.md",
                database.config.static_dir
            ))
            .unwrap_or(String::new()),
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "fun/carp.html")]
struct CarpTemplate {
    config: Config,
    lang: langbeam::LangFile,
    profile: Option<Box<Profile>>,
}

/// GET /site/fun/carp
pub async fn carp_request(jar: CookieJar, State(database): State<Database>) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => Some(ua),
            Err(_) => None,
        },
        None => None,
    };

    Html(
        CarpTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            profile: auth_user,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "auth/login.html")]
struct LoginTemplate {
    config: Config,
    lang: langbeam::LangFile,
    profile: Option<Box<Profile>>,
}

/// GET /login
pub async fn login_request(jar: CookieJar, State(database): State<Database>) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => Some(ua),
            Err(_) => None,
        },
        None => None,
    };

    Html(
        LoginTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            profile: auth_user,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "auth/sign_up.html")]
struct SignUpTemplate {
    config: Config,
    lang: langbeam::LangFile,
    profile: Option<Box<Profile>>,
}

/// GET /sign_up
pub async fn sign_up_request(
    jar: CookieJar,
    State(database): State<Database>,
) -> impl IntoResponse {
    if database.config.registration_enabled == false {
        return Html(DatabaseError::NotAllowed.to_html(database));
    }

    // ...
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => Some(ua),
            Err(_) => None,
        },
        None => None,
    };

    Html(
        SignUpTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            profile: auth_user,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Serialize, Deserialize)]
pub struct PaginatedQuery {
    #[serde(default)]
    pub page: i32,
}

#[derive(Serialize, Deserialize)]
pub struct NotificationsQuery {
    #[serde(default)]
    page: i32,
    #[serde(default)]
    profile: String,
}

#[derive(Serialize, Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    page: i32,
    #[serde(default)]
    q: String,
    #[serde(default)]
    tag: String,
}

#[derive(Serialize, Deserialize)]
pub struct MarketQuery {
    #[serde(default)]
    page: i32,
    #[serde(default)]
    q: String,
    #[serde(default)]
    status: ItemStatus,
    #[serde(default)]
    creator: String,
    #[serde(default)]
    customer: String,
    #[serde(default)]
    r#type: Option<ItemType>,
}

#[derive(Serialize, Deserialize)]
pub struct SearchHomeQuery {
    #[serde(default)]
    driver: i8,
}

#[derive(Serialize, Deserialize)]
pub struct ProfileQuery {
    #[serde(default)]
    pub page: i32,
    pub tag: Option<String>,
    pub q: Option<String>,
    #[serde(default)]
    pub password: String,
}

#[derive(Deserialize)]
pub struct PasswordQuery {
    #[serde(default)]
    pub password: String,
}

/// Escape profile colors
pub fn color_escape(color: &&&String) -> String {
    remove_tags(
        &color
            .replace(";", "")
            .replace("<", "&lt;")
            .replace(">", "%gt;")
            .replace("}", "")
            .replace("{", "")
            .replace("url(\"", "url(\"/api/v0/util/ext/image?img=")
            .replace("url('", "url('/api/v0/util/ext/image?img=")
            .replace("url(https://", "url(/api/v0/util/ext/image?img=https://"),
    )
}

/// Clean profile metadata
pub fn remove_tags(input: &str) -> String {
    Builder::default()
        .rm_tags(&["img", "a", "span", "p", "h1", "h2", "h3", "h4", "h5", "h6"])
        .clean(input)
        .to_string()
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("</script>", "</not-script")
}

/// Clean profile metadata
pub fn clean_metadata(metadata: &ProfileMetadata) -> String {
    remove_tags(&serde_json::to_string(&clean_metadata_raw(metadata)).unwrap())
}

/// Clean profile metadata
pub fn clean_metadata_raw(metadata: &ProfileMetadata) -> ProfileMetadata {
    // remove stupid characters
    let mut metadata = metadata.to_owned();

    for field in metadata.kv.clone() {
        metadata.kv.insert(
            field.0.to_string(),
            field
                .1
                .replace("<", "&lt;")
                .replace(">", "&gt;")
                .replace("url(\"", "url(\"/api/v0/util/ext/image?img=")
                .replace("url(https://", "url(/api/v0/util/ext/image?img=https://")
                .replace("<style>", "")
                .replace("</style>", ""),
        );
    }

    // ...
    metadata
}

/// Clean profile metadata short
pub fn clean_metadata_short(metadata: &ProfileMetadata) -> String {
    remove_tags(&serde_json::to_string(&clean_metadata_short_raw(metadata)).unwrap())
        .replace("\u{200d}", "")
        // how do you end up with these in your settings?
        .replace("\u{0010}", "")
        .replace("\u{0011}", "")
        .replace("\u{0012}", "")
        .replace("\u{0013}", "")
        .replace("\u{0014}", "")
}

/// Clean profile metadata short row
pub fn clean_metadata_short_raw(metadata: &ProfileMetadata) -> ProfileMetadata {
    // remove stupid characters
    let mut metadata = metadata.to_owned();

    for field in metadata.kv.clone() {
        metadata.kv.insert(
            field.0.to_string(),
            field
                .1
                .replace("<", "&lt;")
                .replace(">", "&gt;")
                .replace("<style>", "")
                .replace("</style>", ""),
        );
    }

    // ...
    metadata
}

#[derive(Template)]
#[template(path = "views/question.html")]
struct QuestionTemplate {
    config: Config,
    lang: langbeam::LangFile,
    profile: Option<Box<Profile>>,
    unread: usize,
    notifs: usize,
    question: Question,
    responses: Vec<FullResponse>,
    reactions: Vec<Reaction>,
    already_responded: bool,
    is_powerful: bool,
    is_helper: bool,
}

/// GET /@{}/q/{id}
pub async fn question_request(
    jar: CookieJar,
    Path((_, id)): Path<(String, String)>,
    State(database): State<Database>,
) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => Some(ua),
            Err(_) => None,
        },
        None => None,
    };

    let unread = if let Some(ref ua) = auth_user {
        database.get_inbox_count_by_recipient(&ua.id).await
    } else {
        0
    };

    let notifs = if let Some(ref ua) = auth_user {
        database
            .auth
            .get_notification_count_by_recipient(&ua.id)
            .await
    } else {
        0
    };

    let question = match database.get_question(id.clone()).await {
        Ok(ua) => ua,
        Err(e) => return Html(e.to_html(database)),
    };

    let responses = match database.get_responses_by_question(id.to_owned()).await {
        Ok(responses) => responses,
        Err(_) => return Html(DatabaseError::Other.to_html(database)),
    };

    let mut is_helper: bool = false;
    let is_powerful = if let Some(ref ua) = auth_user {
        let group = match database.auth.get_group_by_id(ua.group).await {
            Ok(g) => g,
            Err(_) => return Html(DatabaseError::Other.to_html(database)),
        };

        is_helper = group.permissions.check_helper();
        group.permissions.check_manager()
    } else {
        false
    };

    let reactions = match database.get_reactions_by_asset(id.clone()).await {
        Ok(r) => r,
        Err(e) => return Html(e.to_html(database)),
    };

    // ...
    Html(
        QuestionTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            already_responded: if let Some(ref ua) = auth_user {
                database
                    .get_response_by_question_and_author(&id, &ua.id)
                    .await
                    .is_ok()
            } else {
                false
            },
            profile: auth_user,
            unread,
            notifs,
            question,
            responses,
            is_powerful,
            is_helper,
            reactions,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "inbox.html")]
struct InboxTemplate {
    config: Config,
    lang: langbeam::LangFile,
    profile: Option<Box<Profile>>,
    unread: Vec<Question>,
    notifs: usize,
    anonymous_username: Option<String>,
    anonymous_avatar: Option<String>,
    is_helper: bool,
}

#[derive(Template)]
#[template(path = "partials/views/reactions.html")]
struct PartialReactionsTemplate {
    reactions: Vec<Reaction>,
}

#[derive(Deserialize)]
pub struct PartialReactionsProps {
    pub id: String,
}

/// GET /_app/components/short_reactions.html
pub async fn partial_reactions_request(
    State(database): State<Database>,
    Query(props): Query<PartialReactionsProps>,
) -> impl IntoResponse {
    Html(
        PartialReactionsTemplate {
            reactions: match database.get_reactions_by_asset(props.id).await {
                Ok(r) => r,
                Err(e) => return Html(e.to_string()),
            },
        }
        .render()
        .unwrap(),
    )
}

/// GET /inbox
pub async fn inbox_request(jar: CookieJar, State(database): State<Database>) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => ua,
            Err(_) => return Html(DatabaseError::NotAllowed.to_html(database)),
        },
        None => return Html(DatabaseError::NotAllowed.to_html(database)),
    };

    // mark all as read
    if !auth_user
        .metadata
        .is_true("rainbeam:do_not_clear_inbox_count_on_view")
    {
        simplify!(
            database
                .auth
                .update_profile_inbox_count(&auth_user.id, 0)
                .await;
            Err; Html(DatabaseError::Other.to_html(database))
        );
    }

    // ...
    let unread = match database.get_questions_by_recipient(&auth_user.id).await {
        Ok(unread) => unread,
        Err(_) => return Html(DatabaseError::Other.to_html(database)),
    };

    let notifs = database
        .auth
        .get_notification_count_by_recipient(&auth_user.id)
        .await;

    let is_helper: bool = {
        let group = match database.auth.get_group_by_id(auth_user.group).await {
            Ok(g) => g,
            Err(_) => return Html(DatabaseError::Other.to_html(database)),
        };

        group.permissions.check_helper()
    };

    Html(
        InboxTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            unread,
            notifs,
            anonymous_username: Some(
                auth_user
                    .metadata
                    .kv
                    .get("sparkler:anonymous_username")
                    .unwrap_or(&"anonymous".to_string())
                    .to_string(),
            ),
            anonymous_avatar: Some(
                auth_user
                    .metadata
                    .kv
                    .get("sparkler:anonymous_avatar")
                    .unwrap_or(&"/static/images/default-avatar.svg".to_string())
                    .to_string(),
            ),
            profile: Some(auth_user),
            is_helper,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "timelines/global_question_timeline.html")]
struct GlobalTimelineTemplate {
    config: Config,
    lang: langbeam::LangFile,
    profile: Option<Box<Profile>>,
    unread: usize,
    notifs: usize,
    questions: Vec<(Question, usize, usize)>,
    relationships: HashMap<String, RelationshipStatus>,
    is_helper: bool,
    page: i32,
}

/// GET /inbox/global/following
pub async fn global_timeline_request(
    jar: CookieJar,
    State(database): State<Database>,
    Query(query): Query<PaginatedQuery>,
) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => ua,
            Err(_) => return Html(DatabaseError::NotAllowed.to_html(database)),
        },
        None => return Html(DatabaseError::NotAllowed.to_html(database)),
    };

    let unread = database.get_inbox_count_by_recipient(&auth_user.id).await;

    let notifs = database
        .auth
        .get_notification_count_by_recipient(&auth_user.id)
        .await;

    let questions = match database
        .get_global_questions_by_following_paginated(&auth_user.id, query.page)
        .await
    {
        Ok(r) => r,
        Err(e) => return Html(e.to_html(database)),
    };

    let is_helper = {
        let group = match database.auth.get_group_by_id(auth_user.group).await {
            Ok(g) => g,
            Err(_) => return Html(DatabaseError::Other.to_html(database)),
        };

        group.permissions.check_helper()
    };

    // build relationships list
    let mut relationships: HashMap<String, RelationshipStatus> = HashMap::new();

    for question in &questions {
        if relationships.contains_key(&question.0.author.id) {
            continue;
        }

        if is_helper {
            // make sure staff can view your questions
            relationships.insert(question.0.author.id.clone(), RelationshipStatus::Friends);
            continue;
        }

        if question.0.author.id == auth_user.id {
            // make sure we can view our own responses
            relationships.insert(question.0.author.id.clone(), RelationshipStatus::Friends);
            continue;
        };

        relationships.insert(
            question.0.author.id.clone(),
            database
                .auth
                .get_user_relationship(&question.0.author.id, &auth_user.id)
                .await
                .0,
        );
    }

    // ...
    Html(
        GlobalTimelineTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            profile: Some(auth_user),
            unread,
            notifs,
            questions,
            relationships,
            is_helper,
            page: query.page,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "timelines/public_global_question_timeline.html")]
struct PublicGlobalTimelineTemplate {
    config: Config,
    lang: langbeam::LangFile,
    profile: Option<Box<Profile>>,
    unread: usize,
    notifs: usize,
    questions: Vec<(Question, usize, usize)>,
    relationships: HashMap<String, RelationshipStatus>,
    is_helper: bool,
    page: i32,
}

/// GET /inbox/global
pub async fn public_global_timeline_request(
    jar: CookieJar,
    State(database): State<Database>,
    Query(query): Query<PaginatedQuery>,
) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => ua,
            Err(_) => return Html(DatabaseError::NotAllowed.to_html(database)),
        },
        None => return Html(DatabaseError::NotAllowed.to_html(database)),
    };

    let unread = database.get_inbox_count_by_recipient(&auth_user.id).await;

    let notifs = database
        .auth
        .get_notification_count_by_recipient(&auth_user.id)
        .await;

    let is_helper = {
        let group = match database.auth.get_group_by_id(auth_user.group).await {
            Ok(g) => g,
            Err(_) => return Html(DatabaseError::Other.to_html(database)),
        };

        group.permissions.check_helper()
    };

    let questions = match database.get_global_questions_paginated(query.page).await {
        Ok(r) => r,
        Err(e) => return Html(e.to_html(database)),
    };

    // build relationships list
    let mut relationships: HashMap<String, RelationshipStatus> = HashMap::new();

    for question in &questions {
        if relationships.contains_key(&question.0.author.id) {
            continue;
        }

        if is_helper {
            // make sure staff can view your questions
            relationships.insert(question.0.author.id.clone(), RelationshipStatus::Friends);
            continue;
        }

        if question.0.author.id == auth_user.id {
            // make sure we can view our own responses
            relationships.insert(question.0.author.id.clone(), RelationshipStatus::Friends);
            continue;
        };

        relationships.insert(
            question.0.author.id.clone(),
            database
                .auth
                .get_user_relationship(&question.0.author.id, &auth_user.id)
                .await
                .0,
        );
    }

    // ...
    let is_helper = {
        let group = match database.auth.get_group_by_id(auth_user.group).await {
            Ok(g) => g,
            Err(_) => return Html(DatabaseError::Other.to_html(database)),
        };

        group.permissions.check_helper()
    };

    Html(
        PublicGlobalTimelineTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            profile: Some(auth_user),
            unread,
            notifs,
            questions,
            relationships,
            is_helper,
            page: query.page,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "notifications.html")]
struct NotificationsTemplate {
    config: Config,
    lang: langbeam::LangFile,
    profile: Option<Box<Profile>>,
    unread: usize,
    notifs: Vec<Notification>,
    page: i32,
    pid: String,
}

/// GET /inbox/notifications
pub async fn notifications_request(
    jar: CookieJar,
    State(database): State<Database>,
    Query(props): Query<NotificationsQuery>,
) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => ua,
            Err(_) => return Html(DatabaseError::NotAllowed.to_html(database)),
        },
        None => return Html(DatabaseError::NotAllowed.to_html(database)),
    };

    // mark all as read
    simplify!(
        database
            .auth
            .update_profile_notification_count(&auth_user.id, 0)
            .await;
        Err; Html(DatabaseError::Other.to_html(database))
    );

    // ...
    let is_helper = {
        let group = match database.auth.get_group_by_id(auth_user.group).await {
            Ok(g) => g,
            Err(_) => return Html(DatabaseError::Other.to_html(database)),
        };

        group.permissions.check_helper()
    };

    let unread = database.get_inbox_count_by_recipient(&auth_user.id).await;

    let pid = if is_helper && !props.profile.is_empty() {
        // use the given profile value if we gave one and we are a helper
        &props.profile
    } else {
        // otherwise, use the current user
        &auth_user.id
    };

    let notifs = match database
        .auth
        .get_notifications_by_recipient_paginated(&pid, props.page)
        .await
    {
        Ok(r) => r,
        Err(_) => return Html(DatabaseError::Other.to_html(database)),
    };

    Html(
        NotificationsTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            pid: pid.to_string(),
            profile: Some(auth_user),
            unread,
            notifs,
            page: props.page,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "reports.html")]
struct ReportsTemplate {
    config: Config,
    lang: langbeam::LangFile,
    profile: Option<Box<Profile>>,
    unread: usize,
    reports: Vec<Notification>,
}

/// GET /inbox/reports
pub async fn reports_request(
    jar: CookieJar,
    State(database): State<Database>,
) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => ua,
            Err(_) => return Html(DatabaseError::NotAllowed.to_html(database)),
        },
        None => return Html(DatabaseError::NotAllowed.to_html(database)),
    };

    // check permission
    let group = match database.auth.get_group_by_id(auth_user.group).await {
        Ok(g) => g,
        Err(_) => return Html(DatabaseError::NotFound.to_html(database)),
    };

    if !group.permissions.check(FinePermission::VIEW_REPORTS) {
        // we must be a manager to do this
        return Html(DatabaseError::NotAllowed.to_html(database));
    }

    // ...
    let unread = database.get_inbox_count_by_recipient(&auth_user.id).await;

    let reports = match database.auth.get_notifications_by_recipient("*").await {
        Ok(r) => r,
        Err(_) => return Html(DatabaseError::Other.to_html(database)),
    };

    Html(
        ReportsTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            profile: Some(auth_user),
            unread,
            reports,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "audit.html")]
struct AuditTemplate {
    config: Config,
    lang: langbeam::LangFile,
    profile: Option<Box<Profile>>,
    unread: usize,
    logs: Vec<Notification>,
    page: i32,
}

/// GET /inbox/audit
pub async fn audit_log_request(
    jar: CookieJar,
    State(database): State<Database>,
    Query(props): Query<PaginatedQuery>,
) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => ua,
            Err(_) => return Html(DatabaseError::NotAllowed.to_html(database)),
        },
        None => return Html(DatabaseError::NotAllowed.to_html(database)),
    };

    // check permission
    let group = match database.auth.get_group_by_id(auth_user.group).await {
        Ok(g) => g,
        Err(_) => return Html(DatabaseError::NotFound.to_html(database)),
    };

    if !group.permissions.check(FinePermission::VIEW_AUDIT_LOG) {
        // we must be a manager to do this
        return Html(DatabaseError::NotAllowed.to_html(database));
    }

    // ...
    let unread = database.get_inbox_count_by_recipient(&auth_user.id).await;

    let logs = match database
        .auth
        .get_notifications_by_recipient_paginated("*(audit)", props.page)
        .await
    {
        Ok(r) => r,
        Err(_) => return Html(DatabaseError::Other.to_html(database)),
    };

    Html(
        AuditTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            profile: Some(auth_user),
            unread,
            logs,
            page: props.page,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "ipbans.html")]
struct IpbansTemplate {
    config: Config,
    lang: langbeam::LangFile,
    profile: Option<Box<Profile>>,
    unread: usize,
    bans: Vec<IpBan>,
}

/// GET /inbox/audit/ipbans
pub async fn ipbans_request(jar: CookieJar, State(database): State<Database>) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => ua,
            Err(_) => return Html(DatabaseError::NotAllowed.to_html(database)),
        },
        None => return Html(DatabaseError::NotAllowed.to_html(database)),
    };

    // check permission
    let group = match database.auth.get_group_by_id(auth_user.group).await {
        Ok(g) => g,
        Err(_) => return Html(DatabaseError::NotFound.to_html(database)),
    };

    if !group.permissions.check(FinePermission::BAN_IP) {
        // we must be a manager to do this
        return Html(DatabaseError::NotAllowed.to_html(database));
    }

    // ...
    let unread = database.get_inbox_count_by_recipient(&auth_user.id).await;

    let bans = match database.auth.get_ipbans(auth_user.clone()).await {
        Ok(r) => r,
        Err(_) => return Html(DatabaseError::Other.to_html(database)),
    };

    Html(
        IpbansTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            profile: Some(auth_user),
            unread,
            bans,
        }
        .render()
        .unwrap(),
    )
}

#[derive(Template)]
#[template(path = "intents/report.html")]
struct ReportTemplate {
    config: Config,
    lang: langbeam::LangFile,
    profile: Option<Box<Profile>>,
}

/// GET /intents/report
pub async fn report_request(jar: CookieJar, State(database): State<Database>) -> impl IntoResponse {
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database
            .auth
            .get_profile_by_unhashed(c.value_trimmed())
            .await
        {
            Ok(ua) => Some(ua),
            Err(_) => None,
        },
        None => None,
    };

    Html(
        ReportTemplate {
            config: database.config.clone(),
            lang: database.lang(if let Some(c) = jar.get("net.rainbeam.langs.choice") {
                c.value_trimmed()
            } else {
                ""
            }),
            profile: auth_user,
        }
        .render()
        .unwrap(),
    )
}

// ...
pub async fn routes(database: Database) -> Router {
    Router::new()
        .route("/", get(homepage_request))
        .route("/public", get(public_timeline_request))
        .route("/discover", get(discover_request))
        .route("/site/about", get(about_request))
        .route("/site/terms-of-service", get(tos_request))
        .route("/site/privacy", get(privacy_request))
        .route("/intents/report", get(report_request))
        .route("/site/fun/carp", get(carp_request))
        // inbox
        .route("/inbox", get(inbox_request))
        .route("/inbox/global", get(public_global_timeline_request))
        .route("/inbox/global/following", get(global_timeline_request))
        .route("/inbox/notifications", get(notifications_request))
        .route("/inbox/reports", get(reports_request)) // staff
        .route("/inbox/audit", get(audit_log_request)) // staff
        .route("/inbox/audit/ipbans", get(ipbans_request)) // staff
        // assets
        .route("/@{username}/q/{id}", get(question_request))
        .route(
            "/@{username}/r/{id}",
            get(models::response::response_request),
        )
        .route("/@{username}/c/{id}", get(models::comment::comment_request))
        // profiles
        .route("/@{username}/_app/warning", get(profile::warning_request))
        .route("/@{username}/mod", get(profile::mod_request)) // staff
        .route("/@{username}/questions", get(profile::questions_request))
        .route("/@{username}/questions/inbox", get(profile::inbox_request)) // staff
        .route(
            "/@{username}/questions/outbox",
            get(profile::outbox_request),
        ) // staff
        .route("/@{username}/following", get(profile::following_request))
        .route("/@{username}/followers", get(profile::followers_request))
        .route("/@{username}/friends", get(profile::friends_request))
        .route(
            "/@{username}/friends/requests",
            get(profile::friend_requests_request),
        )
        .route("/@{username}/friends/blocks", get(profile::blocks_request))
        .route("/@{username}/embed", get(profile::profile_embed_request))
        .route(
            "/@{username}/relationship/friend_accept",
            get(profile::friend_request),
        )
        .route(
            "/@{username}/_app/card.html",
            get(profile::render_card_request),
        )
        .route(
            "/@{username}/_app/feed.html",
            get(profile::partial_profile_request),
        )
        .route(
            "/@{username}/layout",
            get(profile::profile_layout_editor_request),
        )
        .route("/@{username}", get(profile::profile_request))
        .route("/{id}", get(api::profiles::expand_request))
        // settings
        .route("/settings", get(settings::account_settings))
        .route("/settings/sessions", get(settings::sessions_settings))
        .route("/settings/profile", get(settings::profile_settings))
        .route("/settings/theme", get(settings::theme_settings))
        .route("/settings/privacy", get(settings::privacy_settings))
        .route("/settings/coins", get(settings::coins_settings))
        // search
        .route("/search", get(search::search_homepage_request))
        .route("/search/responses", get(search::search_responses_request))
        .route("/search/questions", get(search::search_questions_request))
        .route("/search/users", get(search::search_users_request))
        // market
        .route("/market", get(market::homepage_request))
        .route("/market/new", get(market::create_request))
        .route("/market/item/{id}", get(market::item_request))
        .route(
            "/market/_app/theme_playground.html/{id}",
            get(market::theme_playground_request),
        )
        .route(
            "/market/_app/layout_playground.html/{id}",
            get(market::layout_playground_request),
        )
        // auth
        .route("/login", get(login_request))
        .route("/sign_up", get(sign_up_request))
        // expanders
        .route("/+q/{id}", get(api::questions::expand_request))
        .route("/question/{id}", get(api::questions::expand_request))
        .route("/+r/{id}", get(api::responses::expand_request))
        .route("/response/{id}", get(api::responses::expand_request))
        .route("/+c/{id}", get(api::comments::expand_request))
        .route("/comment/{id}", get(api::comments::expand_request))
        .route("/+u/{id}", get(api::profiles::expand_request))
        .route("/+i/{ip}", get(api::profiles::expand_ip_request))
        // partials
        .route(
            "/_app/components/comments.html",
            get(models::comment::partial_comments_request),
        )
        .route(
            "/_app/components/response_comments.html",
            get(models::comment::partial_response_comments_request),
        )
        .route(
            "/_app/components/response.html",
            get(models::response::partial_response_request),
        )
        .route(
            "/_app/components/short_reactions.html",
            get(partial_reactions_request),
        )
        .route(
            "/_app/timelines/timeline.html",
            get(partial_timeline_request),
        )
        .route(
            "/_app/timelines/public_timeline.html",
            get(partial_public_timeline_request),
        )
        .route(
            "/_app/timelines/discover/responses_top.html",
            get(partial_top_responses_request),
        )
        .route(
            "/_app/timelines/discover/questions_most.html",
            get(partial_top_askers_request),
        )
        .route(
            "/_app/timelines/discover/responses_most.html",
            get(partial_top_responders_request),
        )
        // ...
        .with_state(database)
}
