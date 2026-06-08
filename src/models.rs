use serde::{Deserialize, Serialize};

/// Request body for `POST /api/register`.
#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

/// Request body for `POST /api/notes` and `PUT /api/notes/:id`.
#[derive(Deserialize)]
pub struct NoteRequest {
    pub title: String,
    pub content: String,
}

/// Response body for a single note.
#[derive(Serialize)]
pub struct NoteResponse {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub content: String,
    pub updated_at: String,
}

/// Response body containing a list of notes.
#[derive(Serialize)]
pub struct NotesListResponse {
    pub notes: Vec<NoteResponse>,
}

/// Response body for the newly created note ID.
#[derive(Serialize)]
pub struct CreateNoteResponse {
    pub id: i64,
}