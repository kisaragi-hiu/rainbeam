//! Responds to API requests
use crate::database::Database;
use crate::model::DatabaseError;
use axum::routing::put;
use databeam::prelude::DefaultReturn;

use axum::response::IntoResponse;
use axum::{
    routing::{delete, get, post},
    Json, Router,
};

pub mod general;
pub mod ipbans;
pub mod ipblocks;
pub mod items;
pub mod labels;
pub mod me;
pub mod notifications;
pub mod profile;
pub mod relationships;
pub mod warnings;

pub async fn not_found() -> impl IntoResponse {
    Json(DefaultReturn::<u16> {
        success: false,
        message: DatabaseError::NotFound.to_string(),
        payload: 404,
    })
}

pub fn routes(database: Database) -> Router {
    Router::new()
        // relationships
        .route(
            "/relationships/follow/{id}",
            post(relationships::follow_request),
        )
        .route(
            "/relationships/friend/{id}",
            post(relationships::friend_request),
        )
        .route(
            "/relationships/block/{id}",
            post(relationships::block_request),
        )
        .route(
            "/relationships/current/{id}",
            delete(relationships::delete_request),
        )
        // profiles
        .route(
            "/profile/{id}/tokens/generate",
            post(profile::generate_token_request),
        )
        .route("/profile/{id}/tokens", post(profile::update_tokens_request))
        .route("/profile/{id}/tier", post(profile::update_tier_request))
        .route("/profile/{id}/group", post(profile::update_group_request))
        .route("/profile/{id}/coins", post(profile::update_coins_request))
        .route(
            "/profile/{id}/password",
            post(profile::update_password_request),
        )
        .route(
            "/profile/{id}/username",
            post(profile::update_username_request),
        )
        .route(
            "/profile/{id}/metadata",
            post(profile::update_metdata_request),
        )
        .route(
            "/profile/{id}/metadata",
            put(profile::patch_metdata_request),
        )
        .route("/profile/{id}/badges", post(profile::update_badges_request))
        .route("/profile/{id}/labels", post(profile::update_labels_request))
        .route("/profile/{id}/links", post(profile::update_links_request))
        .route("/profile/{id}/layout", post(profile::update_layout_request))
        .route("/profile/{id}/totp", post(profile::enable_totp_request))
        .route("/profile/{id}/totp", delete(profile::disable_totp_request))
        .route(
            "/profile/{id}/totp_recovery_codes",
            post(profile::refresh_totp_recovery_codes_request),
        )
        .route("/profile/{id}/banner", get(profile::banner_request))
        .route("/profile/{id}/avatar", get(profile::avatar_request))
        .route("/profile/{id}/custom.css", get(profile::css_request))
        .route("/profile/{id}", delete(profile::delete_request))
        .route("/profile/{id}", get(profile::get_request))
        .route("/token/{token}", get(profile::get_from_token_request))
        // items
        .route("/items", post(items::create_request))
        .route("/item/{id}", get(items::get_request))
        .route("/item/{id}", post(items::update_item_request))
        .route("/item/{id}/buy", post(items::buy_request))
        .route("/item/{id}/status", post(items::update_status_request))
        .route(
            "/item/{id}/content",
            post(items::update_item_content_request),
        )
        .route("/item/{id}", delete(items::delete_request))
        // labels
        .route("/labels", post(labels::create_request))
        .route("/label/{id}", get(labels::get_request))
        .route("/label/{id}", delete(labels::delete_request))
        // notifications
        .route("/notifications/{id}", delete(notifications::delete_request))
        .route(
            "/notifications/clear",
            delete(notifications::delete_all_request),
        )
        // warnings
        .route("/warnings", post(warnings::create_request))
        .route("/warnings/{id}", delete(warnings::delete_request))
        // ipbans
        .route("/ipbans", post(ipbans::create_request))
        .route("/ipbans/{id}", delete(ipbans::delete_request))
        // ipblocks
        .route("/ipblocks", post(ipblocks::create_request))
        .route("/ipblocks/{id}", delete(ipblocks::delete_request))
        // me
        .route("/me/tokens/generate", post(me::generate_token_request))
        .route("/me/tokens", post(me::update_tokens_request))
        .route("/me/delete", post(me::delete_request))
        .route("/me/upload_avatar", post(me::upload_avatar_request))
        .route("/me/upload_banner", post(me::upload_banner_request))
        .route("/me", get(me::get_request))
        // account
        .route("/switch", post(general::set_token_request))
        .route("/register", post(general::create_request))
        .route("/login", post(general::login_request))
        .route("/callback", get(general::callback_request))
        .route("/logout", post(general::logout_request))
        .route("/untag", post(general::remove_tag))
        // ...
        .route("/render_layout", post(profile::render_layout_request))
        .with_state(database)
}
