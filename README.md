# AtendeMente Local

AtendeMente Local is a desktop application for independent psychology practice management. It combines a Tauri shell, a React/Vite frontend, an embedded Rust/Axum API, SQLite storage, Firebase Authentication, encrypted session records, local file storage, payments, appointments, and patient exports.

## Stack

- Desktop: Tauri v2
- Frontend: React 18, TypeScript, Vite, Tailwind CSS
- Backend: Rust, Axum, Tokio
- Database: SQLite with SQLx migrations
- Authentication: Firebase Auth JWT verification
- Encryption: AES-256-GCM with per-user derived keys
- Storage: local filesystem uploads

## Requirements

- Node.js 20 or newer
- npm
- Rust stable toolchain
- Tauri system prerequisites for your OS
- Firebase project configured for authentication

On Windows, install the Microsoft C++ Build Tools and WebView2 runtime if they are not already available.

## Setup

Install frontend dependencies:

```bash
npm install
```

Create a local environment file if needed:

```bash
FIREBASE_PROJECT_ID=your-firebase-project-id
```

Optional variables:

```bash
DATABASE_URL=sqlite:C:/Users/you/.config/atendemente/atendemente.db?mode=rwc
SERVER_PORT=3001
STORAGE_DIR=C:/Users/you/.config/atendemente/uploads
MASTER_PEPPER=base64-or-hex-32-byte-secret
VITE_FIREBASE_API_KEY=your-web-api-key
VITE_FIREBASE_AUTH_DOMAIN=your-project.firebaseapp.com
VITE_FIREBASE_PROJECT_ID=your-firebase-project-id
VITE_FIREBASE_STORAGE_BUCKET=your-project.firebasestorage.app
VITE_FIREBASE_MESSAGING_SENDER_ID=your-sender-id
VITE_FIREBASE_APP_ID=your-app-id
```

If `MASTER_PEPPER` is not provided, the app generates one and stores it in the local AtendeMente config directory.

## Development

Run the Tauri desktop app:

```bash
npm run tauri dev
```

Run only the frontend dev server:

```bash
npm run dev
```

The frontend expects the local API at:

```text
http://localhost:3001/api
```

## Build

Build the frontend:

```bash
npm run build
```

Build the desktop application:

```bash
npm run tauri build
```

## Tests

Run Rust tests:

```bash
cd src-tauri
cargo test
```

## Project Layout

```text
src/
  components/      React UI components
  pages/           App screens
  lib/             Frontend API and auth helpers

src-tauri/
  src/api/         Axum route definitions
  src/features/    Business logic
  src/db/          SQLite initialization, models, migrations
  src/crypto.rs    Session record encryption
  src/firebase.rs  Firebase JWT verification
  icons/           Tauri application icons
```

## Notes

- Session records are encrypted before being stored in SQLite.
- Uploaded files are stored on the local filesystem and referenced from SQLite.
- The app requires a valid Firebase project ID outside development mode.
- Local database and upload files are intentionally ignored by Git.
