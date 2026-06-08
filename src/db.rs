use rusqlite::{params, Connection, Result};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::models::NoteResponse;

/// A thread-safe wrapper around a SQLite connection.
#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Opens or creates the SQLite database at `path` and ensures the schema exists.
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrent read performance
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        let db = Database {
            conn: Arc::new(Mutex::new(conn)),
        };

        db.initialize_tables()?;

        tracing::info!("Database initialized at {}", path);
        Ok(db)
    }

    /// Creates the `users` and `notes` tables if they do not exist.
    fn initialize_tables(&self) -> Result<()> {
        let conn = self.conn.blocking_lock();

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS users (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                username    TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS notes (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id     INTEGER NOT NULL,
                title       TEXT NOT NULL,
                content     TEXT NOT NULL,
                updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (user_id) REFERENCES users(id)
            );
            ",
        )?;

        tracing::info!("Database tables initialized");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // User methods
    // -----------------------------------------------------------------------

    /// Creates a new user with the given username and password hash.
    /// Returns the new user's ID.
    pub fn create_user(&self, username: &str, password_hash: &str) -> Result<i64> {
        let conn = self.conn.blocking_lock();
        conn.execute(
            "INSERT INTO users (username, password_hash) VALUES (?1, ?2)",
            params![username, password_hash],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Looks up a user by username and verifies the password against the stored hash.
    /// Returns `Some((user_id, password_hash))` if credentials are valid, `None` otherwise.
    pub fn verify_user(&self, username: &str, password: &str) -> Result<Option<(i64, String)>> {
        let conn = self.conn.blocking_lock();

        let mut stmt = conn.prepare(
            "SELECT id, password_hash FROM users WHERE username = ?1",
        )?;

        let result = stmt.query_row(params![username], |row| {
            let user_id: i64 = row.get(0)?;
            let password_hash: String = row.get(1)?;
            Ok((user_id, password_hash))
        });

        match result {
            Ok((user_id, password_hash)) => {
                // Verify the password using bcrypt
                let valid = bcrypt::verify(password, &password_hash).unwrap_or(false);
                if valid {
                    Ok(Some((user_id, password_hash)))
                } else {
                    Ok(None)
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Checks if a username is already taken.
    pub fn username_exists(&self, username: &str) -> Result<bool> {
        let conn = self.conn.blocking_lock();
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM users WHERE username = ?1")?;
        let count: i64 = stmt.query_row(params![username], |row| row.get(0))?;
        Ok(count > 0)
    }

    // -----------------------------------------------------------------------
    // Notes methods
    // -----------------------------------------------------------------------

    /// Returns all notes belonging to the given user.
    pub fn list_notes(&self, user_id: i64) -> Result<Vec<NoteResponse>> {
        let conn = self.conn.blocking_lock();

        let mut stmt = conn.prepare(
            "SELECT id, user_id, title, content, updated_at FROM notes WHERE user_id = ?1 ORDER BY updated_at DESC",
        )?;

        let notes = stmt
            .query_map(params![user_id], |row| {
                Ok(NoteResponse {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    title: row.get(2)?,
                    content: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(notes)
    }

    /// Creates a new note for the given user.
    /// Returns the new note's ID.
    pub fn create_note(&self, user_id: i64, title: &str, content: &str) -> Result<i64> {
        let conn = self.conn.blocking_lock();

        conn.execute(
            "INSERT INTO notes (user_id, title, content) VALUES (?1, ?2, ?3)",
            params![user_id, title, content],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Updates an existing note. Only succeeds if the note belongs to `user_id`.
    /// Returns `true` if a row was updated, `false` otherwise.
    pub fn update_note(&self, note_id: i64, user_id: i64, title: &str, content: &str) -> Result<bool> {
        let conn = self.conn.blocking_lock();

        let rows = conn.execute(
            "UPDATE notes SET title = ?1, content = ?2, updated_at = datetime('now') WHERE id = ?3 AND user_id = ?4",
            params![title, content, note_id, user_id],
        )?;

        Ok(rows > 0)
    }

    /// Deletes a note. Only succeeds if the note belongs to `user_id`.
    /// Returns `true` if a row was deleted, `false` otherwise.
    pub fn delete_note(&self, note_id: i64, user_id: i64) -> Result<bool> {
        let conn = self.conn.blocking_lock();

        let rows = conn.execute(
            "DELETE FROM notes WHERE id = ?1 AND user_id = ?2",
            params![note_id, user_id],
        )?;

        Ok(rows > 0)
    }
}