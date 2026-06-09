// ========================================================================
// Scriptorium Cloud – Frontend App (Vanilla JS)
// ========================================================================

const API_BASE = '/api';

// ─── DOM References ──────────────────────────────────────────────────────

const loginView = document.getElementById('login-view');
const notesView = document.getElementById('notes-view');
const loginForm = document.getElementById('login-form');
const registerForm = document.getElementById('register-form');
const noteForm = document.getElementById('note-form');
const logoutBtn = document.getElementById('logout-btn');
const themeToggle = document.getElementById('theme-toggle');
const showRegisterLink = document.getElementById('show-register');
const showLoginLink = document.getElementById('show-login');
const loginCard = document.querySelector('.login-card');
const registerCard = document.querySelector('.register-card');
const loginError = document.getElementById('login-error');
const registerError = document.getElementById('register-error');
const notesError = document.getElementById('notes-error');
const notesContainer = document.getElementById('notes-container');
const userDisplay = document.getElementById('user-display');

// ─── Auth Helpers ────────────────────────────────────────────────────────

function getCredentials() {
    const stored = sessionStorage.getItem('scriptorium_auth');
    if (!stored) return null;
    try {
        return JSON.parse(stored);
    } catch {
        return null;
    }
}

function setCredentials(username, password) {
    sessionStorage.setItem('scriptorium_auth', JSON.stringify({ username, password }));
}

function clearCredentials() {
    sessionStorage.removeItem('scriptorium_auth');
}

function getAuthHeader() {
    const creds = getCredentials();
    if (!creds) return null;
    const encoded = btoa(`${creds.username}:${creds.password}`);
    return `Basic ${encoded}`;
}

function getUsername() {
    const creds = getCredentials();
    return creds ? creds.username : null;
}

// ─── API Helper ──────────────────────────────────────────────────────────

async function apiRequest(path, options = {}) {
    const authHeader = getAuthHeader();
    const headers = {
        'Content-Type': 'application/json',
        ...(authHeader ? { 'Authorization': authHeader } : {}),
        ...options.headers,
    };

    const response = await fetch(`${API_BASE}${path}`, {
        ...options,
        headers,
    });

    // If unauthorized, redirect to login
    if (response.status === 401) {
        clearCredentials();
        showLoginView();
        throw new Error('Unauthorized');
    }

    return response;
}

// ─── View Switching ──────────────────────────────────────────────────────

function showLoginView() {
    loginView.style.display = '';
    notesView.style.display = 'none';
    loginError.textContent = '';
    registerError.textContent = '';
}

function showNotesView() {
    if (!getCredentials()) {
        showLoginView();
        return;
    }
    loginView.style.display = 'none';
    notesView.style.display = '';
    userDisplay.textContent = getUsername();
    loadNotes();
}

// ─── Login / Register UI Toggle ──────────────────────────────────────────

function showLoginCard() {
    loginCard.style.display = '';
    registerCard.style.display = 'none';
    loginError.textContent = '';
}

function showRegisterCard() {
    loginCard.style.display = 'none';
    registerCard.style.display = '';
    registerError.textContent = '';
}

// ─── Login ───────────────────────────────────────────────────────────────

loginForm.addEventListener('submit', async (e) => {
    e.preventDefault();
    loginError.textContent = '';

    const username = document.getElementById('username').value.trim();
    const password = document.getElementById('password').value;

    if (!username || !password) {
        loginError.textContent = 'Bitte Benutzername und Passwort eingeben.';
        return;
    }

    // Store credentials temporarily to test
    setCredentials(username, password);

    try {
        // Test credentials by fetching notes
        const response = await apiRequest('/notes');
        if (response.ok) {
            showNotesView();
        } else {
            clearCredentials();
            loginError.textContent = 'Ungültiger Benutzername oder Passwort.';
        }
    } catch (err) {
        clearCredentials();
        loginError.textContent = 'Verbindungsfehler zum Server.';
    }
});

// ─── Register ────────────────────────────────────────────────────────────

registerForm.addEventListener('submit', async (e) => {
    e.preventDefault();
    registerError.textContent = '';

    const username = document.getElementById('reg-username').value.trim();
    const password = document.getElementById('reg-password').value;

    if (!username || !password) {
        registerError.textContent = 'Bitte Benutzername und Passwort eingeben.';
        return;
    }

    if (password.length < 4) {
        registerError.textContent = 'Passwort muss mindestens 4 Zeichen lang sein.';
        return;
    }

    try {
        const response = await fetch(`${API_BASE}/register`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ username, password }),
        });

        const data = await response.json();

        if (response.ok) {
            // Auto-login after registration
            setCredentials(username, password);
            showNotesView();
        } else {
            registerError.textContent = data.error || 'Registrierung fehlgeschlagen.';
        }
    } catch (err) {
        registerError.textContent = 'Verbindungsfehler zum Server.';
    }
});

// ─── Logout ──────────────────────────────────────────────────────────────

logoutBtn.addEventListener('click', () => {
    clearCredentials();
    showLoginView();
    showLoginCard();
});

// ─── UI Navigation: Login <-> Register ──────────────────────────────────

showRegisterLink.addEventListener('click', (e) => {
    e.preventDefault();
    showRegisterCard();
});

showLoginLink.addEventListener('click', (e) => {
    e.preventDefault();
    showLoginCard();
});

// ─── Load Notes ──────────────────────────────────────────────────────────

async function loadNotes() {
    notesError.textContent = '';
    notesContainer.innerHTML = '<p class="empty-state">Lade Notizen…</p>';

    try {
        const response = await apiRequest('/notes');

        if (!response.ok) {
            throw new Error('Fehler beim Laden');
        }

        const data = await response.json();
        renderNotes(data.notes || []);
    } catch (err) {
        if (err.message === 'Unauthorized') {
            return;
        }
        notesError.textContent = 'Fehler beim Laden der Notizen.';
        notesContainer.innerHTML = '<p class="empty-state">Keine Notizen vorhanden.</p>';
    }
}

// ─── Render Notes ────────────────────────────────────────────────────────

function renderNotes(notes) {
    if (notes.length === 0) {
        notesContainer.innerHTML = '<p class="empty-state">Keine Notizen vorhanden. Erstelle deine erste Notiz!</p>';
        return;
    }

    notesContainer.innerHTML = notes.map(note => `
        <div class="note-card" data-id="${note.id}">
            <div class="note-card-header">
                <span class="note-card-title">${escapeHtml(note.title)}</span>
                <span class="note-card-date">${formatDate(note.updated_at)}</span>
            </div>
            <div class="note-card-content">${escapeHtml(note.content)}</div>
            <div class="note-card-actions">
                <button class="btn btn-danger delete-btn" data-id="${note.id}">Löschen</button>
            </div>
        </div>
    `).join('');

    // Attach delete handlers
    document.querySelectorAll('.delete-btn').forEach(btn => {
        btn.addEventListener('click', () => deleteNote(parseInt(btn.dataset.id)));
    });
}

// ─── Create Note ─────────────────────────────────────────────────────────

noteForm.addEventListener('submit', async (e) => {
    e.preventDefault();

    const title = document.getElementById('note-title').value.trim();
    const content = document.getElementById('note-content').value.trim();

    if (!title) {
        notesError.textContent = 'Titel ist erforderlich.';
        return;
    }

    try {
        const response = await apiRequest('/notes', {
            method: 'POST',
            body: JSON.stringify({ title, content }),
        });

        if (!response.ok) {
            const data = await response.json().catch(() => ({}));
            notesError.textContent = data.error || 'Fehler beim Erstellen der Notiz.';
            return;
        }

        // Clear form and reload notes
        document.getElementById('note-title').value = '';
        document.getElementById('note-content').value = '';
        notesError.textContent = '';
        await loadNotes();
    } catch (err) {
        if (err.message === 'Unauthorized') return;
        notesError.textContent = 'Fehler beim Erstellen der Notiz.';
    }
});

// ─── Delete Note ─────────────────────────────────────────────────────────

async function deleteNote(id) {
    if (!confirm('Möchtest du diese Notiz wirklich löschen?')) return;

    try {
        const response = await apiRequest(`/notes/${id}`, {
            method: 'DELETE',
        });

        if (!response.ok) {
            notesError.textContent = 'Fehler beim Löschen der Notiz.';
            return;
        }

        await loadNotes();
    } catch (err) {
        if (err.message === 'Unauthorized') return;
        notesError.textContent = 'Fehler beim Löschen der Notiz.';
    }
}

// ─── Utilities ───────────────────────────────────────────────────────────

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

function formatDate(dateStr) {
    if (!dateStr) return '';
    try {
        const date = new Date(dateStr + 'Z'); // treat as UTC
        return date.toLocaleDateString('de-DE', {
            day: '2-digit',
            month: '2-digit',
            year: 'numeric',
            hour: '2-digit',
            minute: '2-digit',
        });
    } catch {
        return dateStr;
    }
}

// ─── Theme Switcher ──────────────────────────────────────────────────────

const THEME_STORAGE_KEY = 'scriptorium-theme';

function getSavedTheme() {
    return localStorage.getItem(THEME_STORAGE_KEY) || 'dark';
}

function setTheme(theme) {
    document.documentElement.setAttribute('data-theme', theme === 'frutiger-aero' ? 'frutiger-aero' : '');
    localStorage.setItem(THEME_STORAGE_KEY, theme);
    // Update toggle icon
    themeToggle.textContent = theme === 'frutiger-aero' ? '🌙' : '💧';
    themeToggle.title = theme === 'frutiger-aero' ? 'Dark Mode aktivieren' : 'Frutiger Aero aktivieren';
}

// Initialize theme from localStorage on page load
setTheme(getSavedTheme());

// Theme toggle event listener
themeToggle.addEventListener('click', () => {
    const currentTheme = document.documentElement.getAttribute('data-theme') ? 'frutiger-aero' : 'dark';
    const newTheme = currentTheme === 'frutiger-aero' ? 'dark' : 'frutiger-aero';
    setTheme(newTheme);
});

// ─── Initialisation ──────────────────────────────────────────────────────

// Check if user is already authenticated
if (getCredentials()) {
    showNotesView();
} else {
    showLoginView();
    showLoginCard();
}
