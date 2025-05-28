use crate::database::Database;
use crate::model::{
    DatabaseError, FinePermission, NotificationCreate, RenderLayout, SetProfileBadges,
    SetProfileCoins, SetProfileGroup, SetProfileLabels, SetProfileLayout, SetProfileLinks,
    SetProfileMetadata, SetProfilePassword, SetProfileTier, SetProfileUsername, TOTPDisable,
    TokenContext, TokenPermission,
};
use crate::simplify;
use databeam::prelude::DefaultReturn;
use pathbufd::pathd;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue};
use axum::response::IntoResponse;
use axum::{
    extract::{Path, State},
    Json,
};
use axum_extra::extract::cookie::CookieJar;
use serde::Serialize;

use std::{fs::File, io::Read};

pub fn read_image(static_dir: String, image: String) -> Vec<u8> {
    let mut bytes = Vec::new();

    for byte in match File::open(format!("{static_dir}/{image}")) {
        Ok(f) => f,
        Err(_) => return bytes,
    }
    .bytes()
    {
        bytes.push(byte.unwrap())
    }

    bytes
}

/// Get a profile's avatar image
pub async fn avatar_request(
    Path(id): Path<String>,
    State(database): State<Database>,
) -> impl IntoResponse {
    // get user
    let auth_user = match database.get_profile(&id).await {
        Ok(ua) => ua,
        Err(_) => {
            return (
                [("Content-Type", "image/svg+xml")],
                Body::from(read_image(
                    pathd!("{}/images", database.config.static_dir),
                    "default-avatar.svg".to_string(),
                )),
            );
        }
    };

    // ...
    let avatar_url = match auth_user.metadata.kv.get("sparkler:avatar_url") {
        Some(r) => r,
        None => "",
    };

    if (avatar_url == "rb://") && !database.config.media_dir.to_string().is_empty() {
        return (
            [("Content-Type", "image/avif")],
            Body::from(read_image(
                pathd!("{}/avatars", database.config.media_dir),
                format!("{}.avif", &auth_user.id),
            )),
        );
    }

    if avatar_url.starts_with(&database.config.host) {
        return (
            [("Content-Type", "image/svg+xml")],
            Body::from(read_image(
                pathd!("{}/images", database.config.static_dir),
                "default-avatar.svg".to_string(),
            )),
        );
    }

    for host in database.config.blocked_hosts {
        if avatar_url.starts_with(&host) {
            return (
                [("Content-Type", "image/svg+xml")],
                Body::from(read_image(
                    pathd!("{}/images", database.config.static_dir),
                    "default-avatar.svg".to_string(),
                )),
            );
        }
    }

    // get profile image
    if avatar_url.is_empty() {
        return (
            [("Content-Type", "image/svg+xml")],
            Body::from(read_image(
                pathd!("{}/images", database.config.static_dir),
                "default-avatar.svg".to_string(),
            )),
        );
    }

    let guessed_mime = mime_guess::from_path(avatar_url)
        .first_raw()
        .unwrap_or("application/octet-stream");

    match database.http.get(avatar_url).send().await {
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
                "default-avatar.svg".to_string(),
            )),
        ),
    }
}

/// Get a profile's banner image
pub async fn banner_request(
    Path(id): Path<String>,
    State(database): State<Database>,
) -> impl IntoResponse {
    // get user
    let auth_user = match database.get_profile(&id).await {
        Ok(ua) => ua,
        Err(_) => {
            return (
                [("Content-Type", "image/svg+xml")],
                Body::from(read_image(
                    pathd!("{}/images", database.config.static_dir),
                    "default-banner.svg".to_string(),
                )),
            );
        }
    };

    // ...
    let banner_url = match auth_user.metadata.kv.get("sparkler:banner_url") {
        Some(r) => r,
        None => "",
    };

    if (banner_url == "rb://") && !database.config.media_dir.to_string().is_empty() {
        return (
            [("Content-Type", "image/avif")],
            Body::from(read_image(
                pathd!("{}/banners", database.config.media_dir),
                format!("{}.avif", &auth_user.id),
            )),
        );
    }

    if banner_url.starts_with(&database.config.host) {
        return (
            [("Content-Type", "image/svg+xml")],
            Body::from(read_image(
                pathd!("{}/images", database.config.static_dir),
                "default-banner.svg".to_string(),
            )),
        );
    }

    for host in database.config.blocked_hosts {
        if banner_url.starts_with(&host) {
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
    if banner_url.is_empty() {
        return (
            [("Content-Type", "image/svg+xml")],
            Body::from(read_image(
                pathd!("{}/images", database.config.static_dir),
                "default-banner.svg".to_string(),
            )),
        );
    }

    let guessed_mime = mime_guess::from_path(banner_url)
        .first_raw()
        .unwrap_or("application/octet-stream");

    match database.http.get(banner_url).send().await {
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

/// View a profile's information
pub async fn get_request(
    Path(id): Path<String>,
    State(database): State<Database>,
) -> impl IntoResponse {
    // get user
    let mut auth_user = match database.get_profile(&id).await {
        Ok(ua) => ua,
        Err(e) => return Json(e.to_json()),
    };

    // clean profile
    auth_user.clean();

    // return
    Json(DefaultReturn {
        success: true,
        message: auth_user.id.to_string(),
        payload: Some(auth_user),
    })
}

/// View a profile's information from auth token (no cleaning)
pub async fn get_from_token_request(
    Path(token): Path<String>,
    State(database): State<Database>,
) -> impl IntoResponse {
    // get user
    let auth_user = match database.get_profile_by_unhashed(&token).await {
        Ok(ua) => ua,
        Err(e) => return Json(e.to_json()),
    };

    // return
    Json(DefaultReturn {
        success: true,
        message: auth_user.username.to_string(),
        payload: Some(auth_user),
    })
}

/// Change a profile's tier
pub async fn update_tier_request(
    jar: CookieJar,
    Path(id): Path<String>,
    State(database): State<Database>,
    Json(props): Json<SetProfileTier>,
) -> impl IntoResponse {
    // get user from token
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => {
            let token = c.value_trimmed();

            match database.get_profile_by_unhashed(token).await {
                Ok(ua) => {
                    // check token permission
                    if !ua
                        .token_context_from_token(&token)
                        .can_do(TokenPermission::Moderator)
                    {
                        return Json(DatabaseError::NotAllowed.to_json());
                    }

                    // return
                    ua
                }
                Err(e) => return Json(e.to_json()),
            }
        }
        None => return Json(DatabaseError::NotAllowed.to_json()),
    };

    // check permission
    let group = match database.get_group_by_id(auth_user.group).await {
        Ok(g) => g,
        Err(e) => return Json(e.to_json()),
    };

    if !group.permissions.check(FinePermission::MANAGE_PROFILE_TIER) {
        // we must have the "Manager" permission to edit other users
        return Json(DefaultReturn {
            success: false,
            message: DatabaseError::NotAllowed.to_string(),
            payload: None,
        });
    }

    // get other user
    let other_user = match database.get_profile(&id).await {
        Ok(ua) => ua,
        Err(e) => return Json(e.to_json()),
    };

    // check permission
    let group = match database.get_group_by_id(other_user.group).await {
        Ok(g) => g,
        Err(e) => return Json(e.to_json()),
    };

    if group.permissions.check(FinePermission::MANAGE_PROFILE_TIER) {
        return Json(DefaultReturn {
            success: false,
            message: DatabaseError::NotAllowed.to_string(),
            payload: None,
        });
    }

    // push update
    // TODO: try not to clone
    if let Err(e) = database.update_profile_tier(&id, props.tier).await {
        return Json(e.to_json());
    }

    // return
    Json(DefaultReturn {
        success: true,
        message: "Acceptable".to_string(),
        payload: Some(props.tier),
    })
}

/// Change a profile's group
pub async fn update_group_request(
    jar: CookieJar,
    Path(id): Path<String>,
    State(database): State<Database>,
    Json(props): Json<SetProfileGroup>,
) -> impl IntoResponse {
    // get user from token
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database.get_profile_by_unhashed(c.value_trimmed()).await {
            Ok(ua) => ua,
            Err(e) => return Json(e.to_json()),
        },
        None => return Json(DatabaseError::NotAllowed.to_json()),
    };

    // check permission
    let our_group = match database.get_group_by_id(auth_user.group).await {
        Ok(g) => g,
        Err(e) => return Json(e.to_json()),
    };

    if !our_group
        .permissions
        .check(FinePermission::MANAGE_PROFILE_GROUP)
    {
        // we must have the "Manager" permission to edit other users
        return Json(DatabaseError::NotAllowed.to_json());
    }

    // get other user
    let other_user = match database.get_profile(&id).await {
        Ok(ua) => ua,
        Err(e) => return Json(e.to_json()),
    };

    // check permission
    let other_group = match database.get_group_by_id(other_user.group).await {
        Ok(g) => g,
        Err(e) => return Json(e.to_json()),
    };

    if other_group
        .permissions
        .check(FinePermission::MANAGE_PROFILE_GROUP)
    {
        return Json(DatabaseError::NotAllowed.to_json());
    }

    // check group
    if props.group != -1 {
        if let Err(e) = database.get_group_by_id(props.group).await {
            return Json(e.to_json());
        }

        if !our_group.permissions.check(FinePermission::PROMOTE_USERS) {
            // non-managers **cannot** promote people to helper
            return Json(DatabaseError::NotAllowed.to_json());
        }
    }

    // push update
    // TODO: try not to clone
    if let Err(e) = database
        .update_profile_group(&other_user.id, props.group)
        .await
    {
        return Json(e.to_json());
    }

    // return
    if let Err(e) = database
        .audit(
            &auth_user.id,
            &format!(
                "Changed user group: [{}](/+u/{})",
                other_user.id, other_user.id
            ),
        )
        .await
    {
        return Json(e.to_json());
    };

    Json(DefaultReturn {
        success: true,
        message: "Acceptable".to_string(),
        payload: Some(props.group),
    })
}

/// Change a profile's coins
pub async fn update_coins_request(
    jar: CookieJar,
    Path(id): Path<String>,
    State(database): State<Database>,
    Json(props): Json<SetProfileCoins>,
) -> impl IntoResponse {
    // get user from token
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database.get_profile_by_unhashed(c.value_trimmed()).await {
            Ok(ua) => ua,
            Err(e) => return Json(e.to_json()),
        },
        None => return Json(DatabaseError::NotAllowed.to_json()),
    };

    // check permission
    let group = match database.get_group_by_id(auth_user.group).await {
        Ok(g) => g,
        Err(e) => return Json(e.to_json()),
    };

    if !group.permissions.check(FinePermission::ECON_MASTER) {
        // we must have the "Manager" permission to edit other users
        return Json(DefaultReturn {
            success: false,
            message: DatabaseError::NotAllowed.to_string(),
            payload: None,
        });
    }

    // get other user
    let other_user = match database.get_profile(&id).await {
        Ok(ua) => ua,
        Err(e) => return Json(e.to_json()),
    };

    // push update
    // TODO: try not to clone
    if let Err(e) = database
        .update_profile_coins(&other_user.id, props.coins)
        .await
    {
        return Json(e.to_json());
    }

    // return
    if let Err(e) = database
        .audit(
            &auth_user.id,
            &format!(
                "Updated user coin balance: [{}](/+u/{})",
                other_user.id, other_user.id
            ),
        )
        .await
    {
        return Json(e.to_json());
    };

    Json(DefaultReturn {
        success: true,
        message: "Acceptable".to_string(),
        payload: Some(props.coins),
    })
}

/// Update the given user's session tokens
pub async fn update_tokens_request(
    jar: CookieJar,
    Path(id): Path<String>,
    State(database): State<Database>,
    Json(req): Json<super::me::UpdateTokens>,
) -> impl IntoResponse {
    // get user from token
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => {
            let token = c.value_trimmed();

            match database.get_profile_by_unhashed(token).await {
                Ok(ua) => {
                    // check token permission
                    if !ua
                        .token_context_from_token(&token)
                        .can_do(TokenPermission::Moderator)
                    {
                        return Json(DatabaseError::NotAllowed.to_json());
                    }

                    // return
                    ua
                }
                Err(e) => return Json(e.to_json()),
            }
        }
        None => return Json(DatabaseError::NotAllowed.to_json()),
    };

    let mut other = match database.get_profile(&id).await {
        Ok(o) => o,
        Err(e) => return Json(e.to_json()),
    };

    if auth_user.id == other.id {
        return Json(DefaultReturn {
            success: false,
            message: DatabaseError::NotAllowed.to_string(),
            payload: (),
        });
    }

    let group = match database.get_group_by_id(auth_user.group).await {
        Ok(g) => g,
        Err(e) => return Json(e.to_json()),
    };

    if !group.permissions.check(FinePermission::EDIT_USER) {
        // we must have the "Manager" permission to edit other users
        return Json(DefaultReturn {
            success: false,
            message: DatabaseError::NotAllowed.to_string(),
            payload: (),
        });
    }

    // check permission
    let group = match database.get_group_by_id(other.group).await {
        Ok(g) => g,
        Err(e) => return Json(e.to_json()),
    };

    if group.permissions.check(FinePermission::ADMINISTRATOR) {
        // we cannot manager other managers
        return Json(DefaultReturn {
            success: false,
            message: DatabaseError::NotAllowed.to_string(),
            payload: (),
        });
    }

    // for every token that doesn't have a context, insert the default context
    for (i, _) in other.tokens.clone().iter().enumerate() {
        if let None = other.token_context.get(i) {
            other.token_context.insert(i, TokenContext::default())
        }
    }

    // get diff
    let mut removed_indexes = Vec::new();

    for (i, token) in other.tokens.iter().enumerate() {
        if !req.tokens.contains(token) {
            removed_indexes.push(i);
        }
    }

    // edit dependent vecs
    for i in removed_indexes.clone() {
        if (other.ips.len() < i) | (other.ips.len() == 0) {
            break;
        }

        other.ips.remove(i);
    }

    for i in removed_indexes.clone() {
        if (other.token_context.len() < i) | (other.token_context.len() == 0) {
            break;
        }

        other.token_context.remove(i);
    }

    // return
    if let Err(e) = database
        .update_profile_tokens(&other.id, req.tokens, other.ips, other.token_context)
        .await
    {
        return Json(e.to_json());
    }

    Json(DefaultReturn {
        success: true,
        message: "Tokens updated!".to_string(),
        payload: (),
    })
}

/// Generate a new token and session (like logging in while already logged in)
pub async fn generate_token_request(
    jar: CookieJar,
    headers: HeaderMap,
    Path(id): Path<String>,
    State(database): State<Database>,
    Json(props): Json<TokenContext>,
) -> impl IntoResponse {
    // get user from token
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => {
            let token = c.value_trimmed();

            match database.get_profile_by_unhashed(token).await {
                Ok(ua) => {
                    // check token permission
                    if !ua
                        .token_context_from_token(&token)
                        .can_do(TokenPermission::Moderator)
                    {
                        return Json(DatabaseError::NotAllowed.to_json());
                    }

                    // return
                    ua
                }
                Err(e) => return Json(e.to_json()),
            }
        }
        None => return Json(DatabaseError::NotAllowed.to_json()),
    };

    let mut other = match database.get_profile(&id).await {
        Ok(o) => o,
        Err(e) => return Json(e.to_json()),
    };

    if auth_user.id == other.id {
        return Json(DefaultReturn {
            success: false,
            message: DatabaseError::NotAllowed.to_string(),
            payload: None,
        });
    }

    let group = match database.get_group_by_id(auth_user.group).await {
        Ok(g) => g,
        Err(e) => return Json(e.to_json()),
    };

    if !group.permissions.check(FinePermission::EDIT_USER) {
        // we must have the "Manager" permission to edit other users
        return Json(DefaultReturn {
            success: false,
            message: DatabaseError::NotAllowed.to_string(),
            payload: None,
        });
    }

    // check permission
    let group = match database.get_group_by_id(other.group).await {
        Ok(g) => g,
        Err(e) => return Json(e.to_json()),
    };

    if group.permissions.check(FinePermission::ADMINISTRATOR) {
        // we cannot manager other managers
        return Json(DefaultReturn {
            success: false,
            message: DatabaseError::NotAllowed.to_string(),
            payload: None,
        });
    }

    // for every token that doesn't have a context, insert the default context
    for (i, _) in other.tokens.clone().iter().enumerate() {
        if let None = other.token_context.get(i) {
            other.token_context.insert(i, TokenContext::default())
        }
    }

    // get real ip
    let real_ip = if let Some(ref real_ip_header) = database.config.real_ip_header {
        headers
            .get(real_ip_header.to_owned())
            .unwrap_or(&HeaderValue::from_static(""))
            .to_str()
            .unwrap_or("")
            .to_string()
    } else {
        String::new()
    };

    // check ip
    if database.get_ipban_by_ip(&real_ip).await.is_ok() {
        return Json(DefaultReturn {
            success: false,
            message: DatabaseError::NotAllowed.to_string(),
            payload: None,
        });
    }

    // ...
    let token = databeam::utility::uuid();
    let token_hashed = databeam::utility::hash(token.clone());

    other.tokens.push(token_hashed);
    other.ips.push(String::new()); // don't actually store ip, this endpoint is used by external apps
    other.token_context.push(props);

    database
        .update_profile_tokens(&other.id, other.tokens, other.ips, other.token_context)
        .await
        .unwrap();

    // return
    return Json(DefaultReturn {
        success: true,
        message: "Generated token!".to_string(),
        payload: Some(token),
    });
}

/// Change a profile's password
pub async fn update_password_request(
    jar: CookieJar,
    Path(id): Path<String>,
    State(database): State<Database>,
    Json(props): Json<SetProfilePassword>,
) -> impl IntoResponse {
    // get user from token
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => {
            let token = c.value_trimmed();

            match database.get_profile_by_unhashed(token).await {
                Ok(ua) => {
                    // check token permission
                    if !ua
                        .token_context_from_token(&token)
                        .can_do(TokenPermission::ManageAccount)
                    {
                        return Json(DatabaseError::NotAllowed.to_json());
                    }

                    // return
                    ua
                }
                Err(e) => return Json(e.to_json()),
            }
        }
        None => return Json(DatabaseError::NotAllowed.to_json()),
    };

    // check permission
    let mut is_manager = false;
    if auth_user.id != id && auth_user.username != id {
        let group = match database.get_group_by_id(auth_user.group).await {
            Ok(g) => g,
            Err(e) => {
                return Json(DefaultReturn {
                    success: false,
                    message: e.to_string(),
                    payload: None,
                })
            }
        };

        if !group.permissions.check(FinePermission::EDIT_USER) {
            // we must have the "Manager" permission to edit other users
            return Json(DatabaseError::NotAllowed.to_json());
        } else {
            is_manager = true;
        }
    }

    // check user permissions
    // returning NotAllowed here will block them from editing their profile
    // we don't want to waste resources on rule breakers
    if auth_user.group == -1 {
        // group -1 (even if it exists) is for marking users as banned
        return Json(DefaultReturn {
            success: false,
            message: DatabaseError::NotAllowed.to_string(),
            payload: None,
        });
    }

    // push update
    // TODO: try not to clone
    if let Err(e) = database
        .update_profile_password(&id, &props.password, &props.new_password, !is_manager)
        .await
    {
        return Json(e.to_json());
    }

    // return
    Json(DefaultReturn {
        success: true,
        message: "Acceptable".to_string(),
        payload: Some(props.new_password),
    })
}

/// Change a profile's username
pub async fn update_username_request(
    jar: CookieJar,
    Path(id): Path<String>,
    State(database): State<Database>,
    Json(props): Json<SetProfileUsername>,
) -> impl IntoResponse {
    // get user from token
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => {
            let token = c.value_trimmed();

            match database.get_profile_by_unhashed(token).await {
                Ok(ua) => {
                    // check token permission
                    if !ua
                        .token_context_from_token(&token)
                        .can_do(TokenPermission::ManageAccount)
                    {
                        return Json(DatabaseError::NotAllowed.to_json());
                    }

                    // return
                    ua
                }
                Err(e) => return Json(e.to_json()),
            }
        }
        None => return Json(DatabaseError::NotAllowed.to_json()),
    };

    // check permission
    if auth_user.id != id && auth_user.username != id {
        let group = match database.get_group_by_id(auth_user.group).await {
            Ok(g) => g,
            Err(e) => {
                return Json(DefaultReturn {
                    success: false,
                    message: e.to_string(),
                    payload: None,
                })
            }
        };

        if !group.permissions.check(FinePermission::EDIT_USER) {
            // we must have the "Manager" permission to edit other users
            return Json(DatabaseError::NotAllowed.to_json());
        }
    }

    // check user permissions
    // returning NotAllowed here will block them from editing their profile
    // we don't want to waste resources on rule breakers
    if auth_user.group == -1 {
        // group -1 (even if it exists) is for marking users as banned
        return Json(DefaultReturn {
            success: false,
            message: DatabaseError::NotAllowed.to_string(),
            payload: None,
        });
    }

    // push update
    // TODO: try not to clone
    if let Err(e) = database
        .update_profile_username(&id, &props.password, &props.new_name)
        .await
    {
        return Json(e.to_json());
    }

    // return
    Json(DefaultReturn {
        success: true,
        message: "Acceptable".to_string(),
        payload: Some(props.new_name),
    })
}

/// Update a user's metadata
pub async fn update_metdata_request(
    jar: CookieJar,
    Path(id): Path<String>,
    State(database): State<Database>,
    Json(props): Json<SetProfileMetadata>,
) -> impl IntoResponse {
    // get user from token
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => {
            let token = c.value_trimmed();

            match database.get_profile_by_unhashed(token).await {
                Ok(ua) => {
                    // check token permission
                    if !ua
                        .token_context_from_token(&token)
                        .can_do(TokenPermission::ManageProfile)
                    {
                        return Json(DatabaseError::NotAllowed.to_json());
                    }

                    // return
                    ua
                }
                Err(e) => return Json(e.to_json()),
            }
        }
        None => return Json(DatabaseError::NotAllowed.to_json()),
    };

    // check permission
    if auth_user.id != id && auth_user.username != id {
        let group = match database.get_group_by_id(auth_user.group).await {
            Ok(g) => g,
            Err(e) => {
                return Json(DefaultReturn {
                    success: false,
                    message: e.to_string(),
                    payload: (),
                })
            }
        };

        if !group
            .permissions
            .check(FinePermission::MANAGE_PROFILE_SETTINGS)
        {
            // we cannot manager other managers
            return Json(DatabaseError::NotAllowed.to_json());
        }
    }

    // check user permissions
    // returning NotAllowed here will block them from editing their profile
    // we don't want to waste resources on rule breakers
    if auth_user.group == -1 {
        // group -1 (even if it exists) is for marking users as banned
        return Json(DefaultReturn {
            success: false,
            message: DatabaseError::NotAllowed.to_string(),
            payload: (),
        });
    }

    // return
    match database.update_profile_metadata(&id, props.metadata).await {
        Ok(_) => Json(DefaultReturn {
            success: true,
            message: "Acceptable".to_string(),
            payload: (),
        }),
        Err(e) => Json(e.to_json()),
    }
}

/// Patch a user's metadata
pub async fn patch_metdata_request(
    jar: CookieJar,
    Path(id): Path<String>,
    State(database): State<Database>,
    Json(props): Json<SetProfileMetadata>,
) -> impl IntoResponse {
    // get user from token
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => {
            let token = c.value_trimmed();

            match database.get_profile_by_unhashed(token).await {
                Ok(ua) => {
                    // check token permission
                    if !ua
                        .token_context_from_token(&token)
                        .can_do(TokenPermission::ManageProfile)
                    {
                        return Json(DatabaseError::NotAllowed.to_json());
                    }

                    // return
                    ua
                }
                Err(e) => return Json(e.to_json()),
            }
        }
        None => return Json(DatabaseError::NotAllowed.to_json()),
    };

    // get other user
    let other_user = match database.get_profile(&id).await {
        Ok(ua) => ua,
        Err(e) => return Json(e.to_json()),
    };

    // check permission
    if auth_user.id != id && auth_user.username != id {
        let group = match database.get_group_by_id(auth_user.group).await {
            Ok(g) => g,
            Err(e) => {
                return Json(DefaultReturn {
                    success: false,
                    message: e.to_string(),
                    payload: (),
                })
            }
        };

        if !group
            .permissions
            .check(FinePermission::MANAGE_PROFILE_SETTINGS)
        {
            // we must have the "Manager" permission to edit other users
            return Json(DatabaseError::NotAllowed.to_json());
        }
    }

    // check user permissions
    // returning NotAllowed here will block them from editing their profile
    // we don't want to waste resources on rule breakers
    if auth_user.group == -1 {
        // group -1 (even if it exists) is for marking users as banned
        return Json(DefaultReturn {
            success: false,
            message: DatabaseError::NotAllowed.to_string(),
            payload: (),
        });
    }

    // patch metadata
    let mut metadata = other_user.metadata.clone();

    for kv in props.metadata.kv {
        metadata.kv.insert(kv.0, kv.1);
    }

    if props.metadata.policy_consent != metadata.policy_consent {
        metadata.policy_consent = props.metadata.policy_consent;
    }

    // return
    match database.update_profile_metadata(&id, metadata).await {
        Ok(_) => Json(DefaultReturn {
            success: true,
            message: "Acceptable".to_string(),
            payload: (),
        }),
        Err(e) => Json(e.to_json()),
    }
}

/// Update a user's badges
pub async fn update_badges_request(
    jar: CookieJar,
    Path(id): Path<String>,
    State(database): State<Database>,
    Json(props): Json<SetProfileBadges>,
) -> impl IntoResponse {
    // get user from token
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => {
            let token = c.value_trimmed();

            match database.get_profile_by_unhashed(token).await {
                Ok(ua) => {
                    // check token permission
                    if !ua
                        .token_context_from_token(&token)
                        .can_do(TokenPermission::Moderator)
                    {
                        return Json(DatabaseError::NotAllowed.to_json());
                    }

                    // return
                    ua
                }
                Err(e) => return Json(e.to_json()),
            }
        }
        None => return Json(DatabaseError::NotAllowed.to_json()),
    };

    // check permission
    let group = match database.get_group_by_id(auth_user.group).await {
        Ok(g) => g,
        Err(e) => return Json(e.to_json()),
    };

    if !group
        .permissions
        .check(FinePermission::MANAGE_PROFILE_SETTINGS)
    {
        // we must have the "Helper" permission to edit other users' badges
        return Json(DefaultReturn {
            success: false,
            message: DatabaseError::NotAllowed.to_string(),
            payload: (),
        });
    }

    // return
    match database.update_profile_badges(&id, props.badges).await {
        Ok(_) => Json(DefaultReturn {
            success: true,
            message: "Acceptable".to_string(),
            payload: (),
        }),
        Err(e) => Json(e.to_json()),
    }
}

/// Update a user's labels
pub async fn update_labels_request(
    jar: CookieJar,
    Path(id): Path<String>,
    State(database): State<Database>,
    Json(props): Json<SetProfileLabels>,
) -> impl IntoResponse {
    // get user from token
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => {
            let token = c.value_trimmed();

            match database.get_profile_by_unhashed(token).await {
                Ok(ua) => {
                    // check token permission
                    if !ua
                        .token_context_from_token(&token)
                        .can_do(TokenPermission::Moderator)
                    {
                        return Json(DatabaseError::NotAllowed.to_json());
                    }

                    // return
                    ua
                }
                Err(e) => return Json(e.to_json()),
            }
        }
        None => return Json(DatabaseError::NotAllowed.to_json()),
    };

    // check permission
    let group = match database.get_group_by_id(auth_user.group).await {
        Ok(g) => g,
        Err(e) => return Json(e.to_json()),
    };

    if !group
        .permissions
        .check(FinePermission::MANAGE_PROFILE_SETTINGS)
    {
        // we must have the "Helper" permission to edit other users' badges
        return Json(DefaultReturn {
            success: false,
            message: DatabaseError::NotAllowed.to_string(),
            payload: (),
        });
    }

    // return
    match database.update_profile_labels(&id, props.labels).await {
        Ok(_) => Json(DefaultReturn {
            success: true,
            message: "Acceptable".to_string(),
            payload: (),
        }),
        Err(e) => Json(e.to_json()),
    }
}

/// Update a user's links
pub async fn update_links_request(
    jar: CookieJar,
    Path(id): Path<String>,
    State(database): State<Database>,
    Json(props): Json<SetProfileLinks>,
) -> impl IntoResponse {
    // get user from token
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => {
            let token = c.value_trimmed();

            match database.get_profile_by_unhashed(token).await {
                Ok(ua) => {
                    // check token permission
                    if !ua
                        .token_context_from_token(&token)
                        .can_do(TokenPermission::Moderator)
                    {
                        return Json(DatabaseError::NotAllowed.to_json());
                    }

                    // return
                    ua
                }
                Err(e) => return Json(e.to_json()),
            }
        }
        None => return Json(DatabaseError::NotAllowed.to_json()),
    };

    // check permission
    let group = match database.get_group_by_id(auth_user.group).await {
        Ok(g) => g,
        Err(e) => return Json(e.to_json()),
    };

    if (auth_user.id != id)
        && !group
            .permissions
            .check(FinePermission::MANAGE_PROFILE_SETTINGS)
    {
        return Json(DatabaseError::NotAllowed.to_json());
    }

    // return
    match database.update_profile_links(&id, props.links).await {
        Ok(_) => Json(DefaultReturn {
            success: true,
            message: "Acceptable".to_string(),
            payload: (),
        }),
        Err(e) => Json(e.to_json()),
    }
}

/// Update a user's layout
pub async fn update_layout_request(
    jar: CookieJar,
    Path(id): Path<String>,
    State(database): State<Database>,
    Json(props): Json<SetProfileLayout>,
) -> impl IntoResponse {
    // get user from token
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => {
            let token = c.value_trimmed();

            match database.get_profile_by_unhashed(token).await {
                Ok(ua) => {
                    // check token permission
                    if !ua
                        .token_context_from_token(&token)
                        .can_do(TokenPermission::Moderator)
                    {
                        return Json(DatabaseError::NotAllowed.to_json());
                    }

                    // return
                    ua
                }
                Err(e) => return Json(e.to_json()),
            }
        }
        None => return Json(DatabaseError::NotAllowed.to_json()),
    };

    // check permission
    let group = match database.get_group_by_id(auth_user.group).await {
        Ok(g) => g,
        Err(e) => return Json(e.to_json()),
    };

    if (auth_user.id != id)
        && !group
            .permissions
            .check(FinePermission::MANAGE_PROFILE_SETTINGS)
    {
        return Json(DatabaseError::NotAllowed.to_json());
    }

    // return
    match database.update_profile_layout(&id, props.layout).await {
        Ok(_) => Json(DefaultReturn {
            success: true,
            message: "Acceptable".to_string(),
            payload: (),
        }),
        Err(e) => Json(e.to_json()),
    }
}

#[derive(Serialize)]
struct LayoutRenderResult {
    pub block: String,
    pub tree: String,
}

/// Render a layout (in block form).
pub async fn render_layout_request(Json(props): Json<RenderLayout>) -> impl IntoResponse {
    Json(LayoutRenderResult {
        block: props.layout.render_block(),
        tree: props.layout.render_tree(),
    })
}

/// Delete another user
pub async fn delete_request(
    jar: CookieJar,
    Path(id): Path<String>,
    State(database): State<Database>,
) -> impl IntoResponse {
    // get user from token
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => match database.get_profile_by_unhashed(c.value_trimmed()).await {
            Ok(ua) => ua,
            Err(e) => return Json(e.to_json()),
        },
        None => return Json(DatabaseError::NotAllowed.to_json()),
    };

    // check permission
    if auth_user.username != id {
        let group = match database.get_group_by_id(auth_user.group).await {
            Ok(g) => g,
            Err(e) => {
                return Json(DefaultReturn {
                    success: false,
                    message: e.to_string(),
                    payload: (),
                })
            }
        };

        // get other user
        let other_user = match database.get_profile_by_id(&id).await {
            Ok(ua) => ua,
            Err(e) => return Json(e.to_json()),
        };

        if !group.permissions.check(FinePermission::DELETE_USER) {
            // we must have the "Manager" permission to edit other users
            return Json(DatabaseError::NotAllowed.to_json());
        } else {
            let actor_id = auth_user.id;
            simplify!(
                database
                .create_notification(
                    NotificationCreate {
                        title: format!("[{actor_id}](/+u/{actor_id})"),
                        content: format!("Deleted a profile: @{}", other_user.username),
                        address: format!("/+u/{actor_id}"),
                        recipient: "*(audit)".to_string(), // all staff, audit
                    },
                    None,
                )
                .await; Err; Json(DatabaseError::Other.to_json())
            );
        }

        // check permission
        let group = match database.get_group_by_id(other_user.group).await {
            Ok(g) => g,
            Err(e) => {
                return Json(DefaultReturn {
                    success: false,
                    message: e.to_string(),
                    payload: (),
                })
            }
        };

        if group.permissions.check(FinePermission::DELETE_USER) {
            // we cannot manager other managers
            return Json(DatabaseError::NotAllowed.to_json());
        }
    }

    // check user permissions
    // returning NotAllowed here will block them from editing their profile
    // we don't want to waste resources on rule breakers
    if auth_user.group == -1 {
        // group -1 (even if it exists) is for marking users as banned
        return Json(DefaultReturn {
            success: false,
            message: DatabaseError::NotAllowed.to_string(),
            payload: (),
        });
    }

    // return
    match database.delete_profile_by_id(&id).await {
        Ok(_) => Json(DefaultReturn {
            success: true,
            message: "Acceptable".to_string(),
            payload: (),
        }),
        Err(e) => Json(e.to_json()),
    }
}

/// Get a profile's css
pub async fn css_request(
    Path(id): Path<String>,
    State(database): State<Database>,
) -> impl IntoResponse {
    // get user
    let auth_user = match database.get_profile(&id).await {
        Ok(ua) => ua,
        Err(_) => {
            return String::new();
        }
    };

    // ...
    let mut out: String = format!(
        "{}\n*, :root {{\n",
        auth_user
            .metadata
            .soft_get("rainbeam:market_theme_template")
    );

    for style in auth_user.metadata.kv.clone() {
        if !style.0.starts_with("sparkler:color_") {
            continue;
        }

        out.push_str(&format!(
            "    --{}: {};\n",
            style.0.replace("_", "-").replace("sparkler:", ""),
            style.1,
        ));
    }

    out.push_str(&auth_user.metadata.soft_get("sparkler:custom_css"));
    format!("{out}\n}}")
}

/// Enable TOTP for a user.
pub async fn enable_totp_request(
    jar: CookieJar,
    Path(id): Path<String>,
    State(database): State<Database>,
) -> impl IntoResponse {
    // get user from token
    let auth_user = match jar.get("__Secure-Token") {
        Some(c) => {
            let token = c.value_trimmed();

            match database.get_profile_by_unhashed(token).await {
                Ok(ua) => {
                    // check token permission
                    if !ua
                        .token_context_from_token(&token)
                        .can_do(TokenPermission::ManageAccount)
                    {
                        return Json(DatabaseError::NotAllowed.to_json());
                    }

                    // return
                    ua
                }
                Err(e) => return Json(e.to_json()),
            }
        }
        None => return Json(DatabaseError::NotAllowed.to_json()),
    };

    // update
    match database.enable_totp(auth_user, &id).await {
        Ok(t) => {
            return Json(DefaultReturn {
                success: true,
                message: "TOTP enabled".to_string(),
                payload: Some(t),
            })
        }
        Err(e) => return Json(e.to_json()),
    }
}

/// Disable TOTP for a user.
pub async fn disable_totp_request(
    jar: CookieJar,
    Path(id): Path<String>,
    State(database): State<Database>,
    Json(props): Json<TOTPDisable>,
) -> impl IntoResponse {
    // get user from token
    match jar.get("__Secure-Token") {
        Some(c) => {
            let token = c.value_trimmed();

            match database.get_profile_by_unhashed(token).await {
                Ok(ua) => {
                    // check token permission
                    if !ua
                        .token_context_from_token(&token)
                        .can_do(TokenPermission::ManageAccount)
                    {
                        return Json(DatabaseError::NotAllowed.to_json());
                    }

                    // return
                    ua
                }
                Err(e) => return Json(e.to_json()),
            }
        }
        None => return Json(DatabaseError::NotAllowed.to_json()),
    };

    // get profile
    let profile = match database.get_profile(&id).await {
        Ok(p) => p,
        Err(e) => return Json(e.to_json()),
    };

    // check totp
    if !database.check_totp(&profile, &props.totp) {
        return Json(DatabaseError::NotAllowed.to_json());
    }

    // disable
    if let Err(e) = database
        .update_profile_totp_secret(&profile.id, "", &Vec::new())
        .await
    {
        return Json(e.to_json());
    }

    // return
    Json(DefaultReturn {
        success: true,
        message: "TOTP disabled".to_string(),
        payload: (),
    })
}

/// Refresh TOTP recovery codes for a user.
pub async fn refresh_totp_recovery_codes_request(
    jar: CookieJar,
    Path(id): Path<String>,
    State(database): State<Database>,
    Json(props): Json<TOTPDisable>,
) -> impl IntoResponse {
    // get user from token
    match jar.get("__Secure-Token") {
        Some(c) => {
            let token = c.value_trimmed();

            match database.get_profile_by_unhashed(token).await {
                Ok(ua) => {
                    // check token permission
                    if !ua
                        .token_context_from_token(&token)
                        .can_do(TokenPermission::ManageAccount)
                    {
                        return Json(DatabaseError::NotAllowed.to_json());
                    }

                    // return
                    ua
                }
                Err(e) => return Json(e.to_json()),
            }
        }
        None => return Json(DatabaseError::NotAllowed.to_json()),
    };

    // get profile
    let profile = match database.get_profile(&id).await {
        Ok(p) => p,
        Err(e) => return Json(e.to_json()),
    };

    // check totp
    if !database.check_totp(&profile, &props.totp) {
        return Json(DatabaseError::NotAllowed.to_json());
    }

    // update
    let recovery = Database::generate_totp_recovery_codes();

    if let Err(e) = database
        .update_profile_totp_secret(&profile.id, &profile.totp, &recovery)
        .await
    {
        return Json(e.to_json());
    }

    // return
    Json(DefaultReturn {
        success: true,
        message: "TOTP disabled".to_string(),
        payload: Some(recovery),
    })
}
