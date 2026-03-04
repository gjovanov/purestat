pub mod error;
pub mod extractors;
pub mod routes;
pub mod state;

use axum::routing::{delete, get, post, put};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use state::AppState;

pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Public auth routes
    let auth_routes = Router::new()
        .route("/register", post(routes::auth::register))
        .route("/login", post(routes::auth::login))
        .route("/logout", post(routes::auth::logout))
        .route("/refresh", post(routes::auth::refresh))
        .route("/activate", post(routes::auth::activate));

    // Protected user routes
    let me_routes = Router::new()
        .route("/", get(routes::auth::me).put(routes::auth::update_me));

    // OAuth routes
    let oauth_routes = Router::new()
        .route("/{provider}", get(routes::oauth::oauth_redirect))
        .route(
            "/{provider}/callback",
            get(routes::oauth::oauth_callback),
        );

    // Org routes
    let org_routes = Router::new()
        .route("/", get(routes::org::list).post(routes::org::create))
        .route(
            "/{org_id}",
            get(routes::org::get)
                .put(routes::org::update)
                .delete(routes::org::delete),
        );

    // Site routes
    let site_routes = Router::new()
        .route("/", get(routes::site::list).post(routes::site::create))
        .route(
            "/{site_id}",
            get(routes::site::get)
                .put(routes::site::update)
                .delete(routes::site::delete),
        );

    // Goal routes
    let goal_routes = Router::new()
        .route(
            "/",
            get(routes::goal::list).post(routes::goal::create),
        )
        .route("/{goal_id}", delete(routes::goal::delete));

    // Member routes
    let member_routes = Router::new()
        .route("/", get(routes::member::list))
        .route(
            "/{member_id}",
            put(routes::member::update_role).delete(routes::member::remove),
        );

    // Invite routes (org-scoped)
    let invite_org_routes = Router::new()
        .route(
            "/",
            get(routes::invite::list).post(routes::invite::create),
        )
        .route("/{invite_id}", delete(routes::invite::revoke));

    // API key routes
    let api_key_routes = Router::new()
        .route(
            "/",
            get(routes::api_key::list).post(routes::api_key::create),
        )
        .route("/{key_id}", delete(routes::api_key::revoke));

    // Stats routes
    let stats_routes = Router::new()
        .route("/stats", post(routes::stats::query))
        .route("/realtime", get(routes::realtime::current_visitors))
        .route("/export", get(routes::export::export_csv));

    // Stripe routes
    let stripe_routes = Router::new()
        .route("/plans", get(routes::stripe::plans))
        .route("/checkout", post(routes::stripe::checkout))
        .route("/portal", post(routes::stripe::portal))
        .route("/webhook", post(routes::stripe::webhook));

    // Compose API
    let api = Router::new()
        .route("/health", get(routes::health::health))
        .nest("/auth", auth_routes)
        .nest("/me", me_routes)
        .nest("/oauth", oauth_routes)
        .nest("/org", org_routes)
        .nest("/org/{org_id}/site", site_routes)
        .nest("/org/{org_id}/site/{site_id}/goal", goal_routes)
        .nest("/org/{org_id}/member", member_routes)
        .nest("/org/{org_id}/invite", invite_org_routes)
        .nest("/org/{org_id}/site/{site_id}/api-key", api_key_routes)
        .nest("/org/{org_id}/site/{site_id}", stats_routes)
        .nest("/stripe", stripe_routes)
        // Public invite endpoints
        .route("/invite/{code}", get(routes::invite::info))
        .route("/invite/{code}/accept", post(routes::invite::accept))
        // Event ingest (public, validated by domain)
        .route("/event", post(routes::event::ingest));

    Router::new()
        .nest("/api", api)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
