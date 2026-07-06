use rusqlite::{params, Connection, Result as SqliteResult};
use std::sync::{Arc, Mutex};

use crate::models::NoteResponse;

/// A thread-safe wrapper around a SQLite connection.
/// Uses `std::sync::Mutex` (not tokio::sync::Mutex) because all DB operations
/// are offloaded to blocking threads via `tokio::task::spawn_blocking`.
#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Opens or creates the SQLite database at `path` and ensures the schema exists.
    pub fn new(path: &str) -> SqliteResult<Self> {
        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrent read performance
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        let db = Database {
            conn: Arc::new(Mutex::new(conn)),
        };

        db.initialize_tables_sync()?;

        tracing::info!("Database initialized at {}", path);
        Ok(db)
    }

    /// Synchronous helper to create tables – called only during `new()` on the main thread.
    fn initialize_tables_sync(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();

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
                category    TEXT NOT NULL DEFAULT 'Allgemein',
                updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (user_id) REFERENCES users(id)
            );
            ",
        )?;

        // Add category column for databases created before the category feature
        // Only run migration if the column doesn't already exist
        let has_category = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('notes') WHERE name = 'category'")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
            .unwrap_or(0);
        if has_category == 0 {
            conn.execute_batch(
                "ALTER TABLE notes ADD COLUMN category TEXT NOT NULL DEFAULT 'Allgemein';"
            )?;
        }

        tracing::info!("Database tables initialized");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // User methods (async, offloaded to blocking threads)
    // -----------------------------------------------------------------------

    /// Creates a new user with the given username and password hash.
    /// Returns the new user's ID.
    pub async fn create_user(&self, username: String, password_hash: String) -> SqliteResult<i64> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT INTO users (username, password_hash) VALUES (?1, ?2)",
                params![username, password_hash],
            )?;
            Ok(conn.last_insert_rowid())
        })
        .await
        .expect("Blocking task panicked")
    }

    /// Looks up a user by username and verifies the password against the stored hash.
    /// Returns `Some(user_id)` if credentials are valid, `None` otherwise.
    pub async fn verify_user(
        &self,
        username: String,
        password: String,
    ) -> SqliteResult<Option<i64>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();

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
                    let valid = bcrypt::verify(&password, &password_hash).unwrap_or(false);
                    if valid {
                        Ok(Some(user_id))
                    } else {
                        Ok(None)
                    }
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
        .await
        .expect("Blocking task panicked")
    }

    /// Checks if a username is already taken.
    pub async fn username_exists(&self, username: String) -> SqliteResult<bool> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt =
                conn.prepare("SELECT 1 FROM users WHERE username = ?1 LIMIT 1")?;
            let exists = stmt.exists(params![username])?;
            Ok(exists)
        })
        .await
        .expect("Blocking task panicked")
    }

    // -----------------------------------------------------------------------
    // Notes methods (async, offloaded to blocking threads)
    // -----------------------------------------------------------------------

    /// Returns all notes belonging to the given user.
    pub async fn list_notes(&self, user_id: i64) -> SqliteResult<Vec<NoteResponse>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();

            let mut stmt = conn.prepare(
                "SELECT id, user_id, title, content, category, updated_at FROM notes WHERE user_id = ?1 ORDER BY updated_at DESC",
            )?;

            let notes = stmt
                .query_map(params![user_id], |row| {
                    Ok(NoteResponse {
                        id: row.get(0)?,
                        user_id: row.get(1)?,
                        title: row.get(2)?,
                        content: row.get(3)?,
                        category: row.get(4)?,
                        updated_at: row.get(5)?,
                    })
                })?
                .collect::<SqliteResult<Vec<_>>>()?;

            Ok(notes)
        })
        .await
        .expect("Blocking task panicked")
    }

    /// Creates a new note for the given user.
    /// Returns the new note's ID.
    pub async fn create_note(&self, user_id: i64, title: String, content: String, category: String) -> SqliteResult<i64> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT INTO notes (user_id, title, content, category) VALUES (?1, ?2, ?3, ?4)",
                params![user_id, title, content, category],
            )?;
            Ok(conn.last_insert_rowid())
        })
        .await
        .expect("Blocking task panicked")
    }

    /// Updates an existing note. Only succeeds if the note belongs to `user_id`.
    /// Returns `true` if a row was updated, `false` otherwise.
    pub async fn update_note(
        &self,
        note_id: i64,
        user_id: i64,
        title: String,
        content: String,
        category: String,
    ) -> SqliteResult<bool> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let rows = conn.execute(
                "UPDATE notes SET title = ?1, content = ?2, category = ?3, updated_at = datetime('now') WHERE id = ?4 AND user_id = ?5",
                params![title, content, category, note_id, user_id],
            )?;
            Ok(rows > 0)
        })
        .await
        .expect("Blocking task panicked")
    }

    /// Deletes a note. Only succeeds if the note belongs to `user_id`.
    /// Returns `true` if a row was deleted, `false` otherwise.
    pub async fn delete_note(&self, note_id: i64, user_id: i64) -> SqliteResult<bool> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let rows = conn.execute(
                "DELETE FROM notes WHERE id = ?1 AND user_id = ?2",
                params![note_id, user_id],
            )?;
            Ok(rows > 0)
        })
        .await
        .expect("Blocking task panicked")
    }
}