mod auth;
mod db;
mod models;

use std::sync::Arc;

use axum::{
    extract::{FromRef, Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};

use auth::AuthUser;
use db::Database;
use models::{CreateNoteResponse, NoteRequest, NotesListResponse, RegisterRequest};

/// Application state shared across all handlers via Axum's extractor system.
#[derive(Clone)]
struct AppState {
    db: Arc<Database>,
}

// Allow extracting `Arc<Database>` from `AppState` (needed by `AuthUser` extractor)
impl FromRef<AppState> for Arc<Database> {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}

#[tokio::main]
async fn main() {
    // Initialize the tracing subscriber for structured logging
    tracing_subscriber::fmt::init();

    // Determine the database path from environment variable or use default
    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "./scriptorium.db".to_string());
    tracing::info!("Using database path: {}", db_path);

    // Initialize the database (creates tables if they don't exist)
    let database = Database::new(&db_path).expect("Failed to initialize database");

    // Build shared state
    let state = AppState {
        db: Arc::new(database),
    };

    // Build the Axum router with routes
    let app = Router::new()
        // Health check (public)
        .route("/health", get(health_check))
        // Registration (public)
        .route("/api/register", post(register_handler))
        // Notes CRUD (authenticated)
        .route("/api/notes", get(list_notes_handler))
        .route("/api/notes", post(create_note_handler))
        .route("/api/notes/:id", put(update_note_handler))
        .route("/api/notes/:id", delete(delete_note_handler))
        // Attach shared state
        .with_state(state);

    // Bind to 0.0.0.0:3000
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Starting Scriptorium Cloud API on {}", addr);

    // Start the server
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");

    axum::serve(listener, app)
        .await
        .expect("Server failed");
}

// ---------------------------------------------------------------------------
// Health check
// ---------------------------------------------------------------------------

async fn health_check() -> &'static str {
    "Scriptorium Cloud API is running!"
}

// ---------------------------------------------------------------------------
// Registration (public)
// ---------------------------------------------------------------------------

async fn register_handler(
    State(db): State<Arc<Database>>,
    Json(req): Json<RegisterRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // Validate input
    if req.username.is_empty() || req.password.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Username and password are required" })),
        ));
    }

    // Check if username already exists
    if db
        .username_exists(&req.username)
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal server error" })),
            )
        })?
    {
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": "Username already taken" })),
        ));
    }

    // Hash the password
    let password_hash = bcrypt::hash(&req.password, bcrypt::DEFAULT_COST).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to hash password" })),
        )
    })?;

    // Create the user
    let user_id = db.create_user(&req.username, &password_hash).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to create user" })),
        )
    })?;

    tracing::info!("Created user '{}' with id {}", req.username, user_id);

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": user_id, "username": req.username })),
    ))
}

// ---------------------------------------------------------------------------
// Notes CRUD (authenticated)
// ---------------------------------------------------------------------------

async fn list_notes_handler(
    auth_user: AuthUser,
    State(db): State<Arc<Database>>,
) -> Result<Json<NotesListResponse>, (StatusCode, Json<serde_json::Value>)> {
    let notes = db.list_notes(auth_user.user_id).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to retrieve notes" })),
        )
    })?;

    Ok(Json(NotesListResponse { notes }))
}

async fn create_note_handler(
    auth_user: AuthUser,
    State(db): State<Arc<Database>>,
    Json(req): Json<NoteRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if req.title.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Title is required" })),
        ));
    }

    let note_id = db
        .create_note(auth_user.user_id, &req.title, &req.content)
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Failed to create note" })),
            )
        })?;

    tracing::info!(
        "User {} created note {}: '{}'",
        auth_user.user_id,
        note_id,
        req.title
    );

    Ok((
        StatusCode::CREATED,
        Json(CreateNoteResponse { id: note_id }),
    ))
}

async fn update_note_handler(
    auth_user: AuthUser,
    State(db): State<Arc<Database>>,
    Path(note_id): Path<i64>,
    Json(req): Json<NoteRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if req.title.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Title is required" })),
        ));
    }

    let updated = db
        .update_note(note_id, auth_user.user_id, &req.title, &req.content)
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Failed to update note" })),
            )
        })?;

    if !updated {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Note not found or access denied" })),
        ));
    }

    tracing::info!(
        "User {} updated note {}: '{}'",
        auth_user.user_id,
        note_id,
        req.title
    );

    Ok(StatusCode::NO_CONTENT)
}

async fn delete_note_handler(
    auth_user: AuthUser,
    State(db): State<Arc<Database>>,
    Path(note_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let deleted = db.delete_note(note_id, auth_user.user_id).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to delete note" })),
        )
    })?;

    if !deleted {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Note not found or access denied" })),
        ));
    }

    tracing::info!("User {} deleted note {}", auth_user.user_id, note_id);

    Ok(StatusCode::NO_CONTENT)
}