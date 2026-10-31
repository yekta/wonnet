# Wonnet

An email client that feels instant, for the Mac, iOS, Android, Windows and Linux. Its look is the old Newton Mail's: calm, plain, nothing that isn't needed. Open source, under the MIT license. [wonnet.app](https://wonnet.app)

![The inbox on the Mac](docs/screenshots/mac-inbox.png)

| | |
| --- | --- |
| ![A thread](docs/screenshots/mac-thread.png) | ![A thread in dark mode](docs/screenshots/mac-dark.png) |

<p>
  <img src="docs/screenshots/ios-inbox.png" alt="The inbox on iPhone" width="49%">
  <img src="docs/screenshots/ios-dark.png" alt="The inbox on iPhone in dark mode" width="49%">
</p>

- **Nothing waits on the network.** The apps keep a local copy of your mail in SQLite. Opening, archiving, starring and searching read and write that copy at once; a server syncs it with your mail provider in the background.
- **Gmail, Outlook and JMAP.** Gmail through Google's API, Outlook.com, Hotmail and Microsoft 365 through Microsoft Graph, and any JMAP server (Fastmail, Stalwart). IMAP is planned.
- **Newton's habits.** A unified inbox with an account colour on every thread, snooze and remind me (typed as "tomorrow 9am"), undo send, send later, and single keys on the Mac, Superhuman's by default: `j`/`k` to move, `e` archive, `s` star, `#` trash, `u` unread, `h` snooze, `r` reply, `a` reply all, `f` forward, `c` compose, `/` search, `z` undo, `g` then `i` for the inbox, and `⌘K` for everything else. `?` lists them all, and any of them can be changed in Settings.
- **The rest of a full mail client.** Split Inbox (Important, your own splits, Other for newsletters), labels, multi-select, mute, block, one-click unsubscribe, Get Me To Zero, search with Gmail's operators, drafts on every device, signatures, aliases, snippets, attachments in and out, senders' photos (their Gravatar or their company's logo), a contact pane, swipe actions on phones, notifications and printing.
- **Private by default.** Remote images stay blocked until you ask for them, and message bodies are sanitized before they are drawn. The server keeps your mail credentials encrypted, and new mail is pushed to your phone encrypted with a key only the phone has, so neither Apple nor Google sees it.

These are its programs:

| Program | Where it runs | What it does |
| --- | --- | --- |
| Wonnet.app (`apps/macos`) | Your Mac | The interface |
| Wonnet for iOS (`apps/ios`) | Your iPhone and iPad | The interface |
| Wonnet for Android (`apps/android`) | Your Android phone and tablet | The interface |
| Wonnet for Windows and Linux (`apps/gpui`) | Your PC | The interface |
| `mail-server` (`apps/server`) | A server, beside Postgres | Signs you in, syncs your accounts with their providers, sends, snoozes and searches |

Every app is built on one Rust core (`crates/core`): the local copy, the sync, and everything they draw, so clients for other systems can share it.

## Getting started

You run the server yourself. You need Docker, Rust, Node 24, and for the Apple apps a Mac with Xcode (and [XcodeGen](https://github.com/yonaskolb/XcodeGen) for iOS). The Android app needs JDK 21, the Android SDK and NDK and [cargo-ndk](https://github.com/bbqsrc/cargo-ndk).

### 1. Start Postgres and the server

```sh
docker compose up -d                 # Postgres on localhost:5441, and Stalwart for trying JMAP
cp .env.example .env                 # then fill it in, see below
set -a; . ./.env; set +a
cargo run --release -p mail-server   # on http://localhost:3000
```

| Variable | What it is |
| --- | --- |
| `PUBLIC_URL` | Where the server can be reached, without a trailing slash. Google sends you back to `{PUBLIC_URL}/auth/google/callback`, Microsoft to `{PUBLIC_URL}/auth/microsoft/callback`. Over https, Outlook pushes changes to `{PUBLIC_URL}/hooks/outlook` |
| `DATABASE_URL` | Postgres. The server runs its migrations when it starts |
| `SECRET_KEY` | Seals the mail credentials the server keeps. Any long random text (`openssl rand -hex 32`); changing it loses access to every account |
| `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` | The Google OAuth client, below. Without them Gmail can't be added |
| `MICROSOFT_CLIENT_ID`, `MICROSOFT_CLIENT_SECRET` | The Microsoft app registration, below. Without them Outlook can't be added |
| `APNS_KEY`, `APNS_KEY_ID`, `APNS_TEAM_ID` | Optional: an APNs auth key (the text of its .p8), its ID and your Apple team ID. Without them nothing is pushed to iPhones |
| `FCM_SERVICE_ACCOUNT` | Optional: the JSON key of a Firebase service account that may send with Cloud Messaging, as text. Without it nothing is pushed to Android phones |
| `GMAIL_PUBSUB_TOPIC`, `GMAIL_HOOK_TOKEN` | Optional: Gmail pushes changes to a Pub/Sub topic whose push subscription calls `{PUBLIC_URL}/hooks/gmail?token={GMAIL_HOOK_TOKEN}`. Without them new Gmail mail waits for the next poll |
| `POLL_SECONDS` | How often an account is synced when the provider pushed nothing. JMAP servers push over their event source. 10 by default |
| `PORT` | The port the server listens on. 3000 by default |
| `INITIAL_SYNC_LIMIT` | How many messages an account's first sync fetches before older mail is filled in. 2000 by default |
| `DEV_LOGIN` | `1` gives sessions without an account, for tests. Never set it in production |

The `Dockerfile` builds the server for deploying.

### 2. Create the Google OAuth client

1. In the [Google Cloud console](https://console.cloud.google.com), create a project and enable the **Gmail API** (APIs & Services → Library).
2. Under **Google Auth Platform → Branding**, fill in the app's name and your support email.
3. Under **Audience**, choose **External** and leave the publishing status on **Testing**. Add every Google account that will use the app under **Test users**: up to 100.
4. Under **Data Access**, add the scopes `openid`, `email` and `https://mail.google.com/` (all of Gmail, as IMAP clients ask for).
5. Under **Clients**, create a client of the type **Web application**, with the authorized redirect URI `{PUBLIC_URL}/auth/google/callback` (for example `http://localhost:3000/auth/google/callback`).
6. Put its client ID and secret in `GOOGLE_CLIENT_ID` and `GOOGLE_CLIENT_SECRET`.

In Testing mode, Google ends a refresh token after **7 days**: the account then shows "Sign in again" and you remove and add it.

**Before a public launch**, the app has to leave Testing mode. `https://mail.google.com/` is a restricted scope and the server stores the mail it fetches, so Google requires its verification and a yearly **CASA security assessment** (Cloud Application Security Assessment) by an authorized lab. Plan weeks for it, and its cost.

### 3. Register the app with Microsoft

1. In the [Microsoft Entra admin center](https://entra.microsoft.com), open **App registrations → New registration**. A Microsoft account without a directory needs one first: signing up for Azure or for a Microsoft 365 business trial makes one.
2. Supported account types: **any Entra ID tenant and personal Microsoft accounts**. Redirect URI: platform **Web**, `{PUBLIC_URL}/auth/microsoft/callback`; add one for each server you run.
3. Under **API permissions → Microsoft Graph → Delegated**, add `offline_access`, `openid`, `email`, `User.Read`, `Mail.ReadWrite`, `Mail.Send` and `MailboxSettings.ReadWrite`.
4. Under **Certificates & secrets**, make a client secret. Put the **Application (client) ID** in `MICROSOFT_CLIENT_ID` and the secret's **Value** in `MICROSOFT_CLIENT_SECRET`.

A client secret lasts two years at most: make a new one before it runs out, or every Outlook account stops syncing. Without publisher verification, Microsoft calls the app unverified when people sign in; personal accounts can still allow it, and a company may ask its admin to.

Outlook keeps a message in one folder, so labels are its categories, and Outlook's own folders show as archived mail. A message sent through Microsoft Graph can be 3 MB at most, files included.

### 4. Build the apps

```sh
apps/macos/scripts/build-app.sh --open
```

The apps start on our server, `https://api.wonnet.app`. `MAIL_SERVER_URL=http://localhost:3000 apps/macos/scripts/build-app.sh` builds one that starts on your own, and the server can be changed under **Advanced** on the first screen.

The Mac app from a [release](https://github.com/yekta/wonnet/releases/latest) updates itself: a banner at the bottom of the window offers each new release, downloads and installs it, then offers to restart into it. A copy you built yourself doesn't, as it isn't signed by the developer.

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
cd apps/ios && xcodegen generate && open Mail.xcodeproj
```

Xcode builds the core for the iOS app itself. Set your team under Signing to run it on a device, and `MAIL_SERVER_URL` in `project.yml` for another server.

```sh
rustup target add aarch64-linux-android x86_64-linux-android
cd apps/android && ./gradlew installDebug     # on a running emulator or a phone
apps/gpui/scripts/patch-crates.sh && cargo run --release --manifest-path apps/gpui/Cargo.toml   # Windows, Linux (and macOS, for development)
```

Gradle builds the core for the Android app itself. `MAIL_SERVER_URL` set while building points the Android and desktop apps at another server too. Push to Android needs a Firebase project: put its `google-services.json` in `apps/android/app`.

The Android app from a release (`Wonnet.apk`) and the Windows and Linux app (`Wonnet-windows-x86_64.zip`, `Wonnet-linux-<arch>.AppImage`) update themselves the same way the Mac app does. Linux updates only when it runs as the AppImage. The AppImage carries the WebKitGTK it shows mail with, so it needs nothing installed.

To look at the apps without a server or an account, start them with `--demo`: they show made-up mail, kept apart from your own and made fresh at every start. On the Mac, `open apps/macos/build/Wonnet.app --args --demo`; on iOS, add `--demo` to the scheme's arguments in Xcode; on Windows and Linux, `wonnet --demo`; on Android, `adb shell am start -n app.wonnet.wonnet/app.wonnet.MainActivity --ez demo true`.

### 5. Add an account

Open the app and choose **Continue with Google** or **Continue with Microsoft**, or add a JMAP server with its address, your email and a password (for Fastmail, an app password and `https://api.fastmail.com`). The first account makes your user; accounts added later in Settings join it.

To try it without a real account, give the compose Stalwart a demo mailbox and add it as a JMAP account:

```sh
cargo run -p mail-server -- seed     # demo@example.com, password quiet-harbor-lantern-42
```

Server `http://localhost:8441`, email `demo@example.com`.

## Development

[AGENTS.md](AGENTS.md) describes the code, its rules and its checks. In short:

```sh
export DATABASE_URL=postgres://mail:mail@localhost:5441/mail STALWART_URL=http://localhost:8441
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
node packages/theme/build.mjs --check
cargo run -p mail-core --example drive   # drive the core from a terminal
cd apps/marketing && pnpm install && pnpm dev   # wonnet.app
```

## License

MIT
