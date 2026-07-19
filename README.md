# FAS — Forward Auth Service

FAS (Forward Auth Service) is a lightweight visitor access control service designed specifically for Traefik's ForwardAuth middleware. Written in Rust using the `axum` web framework, it provides visitor registration, rate-limiting, and an administrative dashboard to manage visitors.

## Features

- **Forward Auth Middleware Compatible**: Inspects and intercepts traffic via the standard `/_auth` route.
- **In-Memory Store with Debounced Persistence**: Fast lookups with asynchronous JSONL persistence, debouncing disk writes (at most once every 30s) to maximize throughput and minimize disk wear.
- **Graceful Shutdown**: Intercepts `SIGTERM` / `SIGINT` signals to flush any unsaved database records before exiting.
- **Automatic Expiration (TTL)**:
  - **Soft TTL**: Automatically purges unapproved visitor records older than 1 hour.
  - **Hard TTL**: Automatically purges all records older than 30 days.
- **IP-Based Rate Limiting**: Limit unapproved requests to 1 request per 5 seconds per IP, returning a `429 Too Many Requests` page with an interactive countdown timer.
- **Multi-lingual Support**: Automatically detects language preferences (`Accept-Language` headers) and serves pages in English (`en`) or Chinese (`zh-CN`).
- **Secure Dashboard**: Manage visitor approval status, see real-time requests counter, last IP, and user-agent strings on `/`.
- **Secure Runtime Container**: Multi-arch Docker images built on secure, ultra-minimal `gcr.io/distroless/cc-debian12`.

---

## Architecture and Code Modules

The application is structured cleanly:
- `src/main.rs`: Configures logging, spawns scheduler loops (saving, purging, and rate-limit cleanups), and sets up the server.
- `src/config.rs`: Defines environment configuration settings.
- `src/store.rs`: Manages in-memory storage, file synchronization, record purging, and rate limit rules.
- `src/handlers.rs`: Contains Axum handlers for HTTP APIs, auth validation, and dashboard routing.
- `src/templates.rs`: Standard styling templates for visitor cards, rate-limiting counters, and administrative lists.
- `src/i18n.rs`: Handles localization dictionaries and header parses.

---

## Configuration

You can configure FAS by setting the following environment variables:

| Environment Variable | Description | Default Value |
| :--- | :--- | :--- |
| `FAS_PORT` | Port the web server binds to | `8080` |
| `FAS_DATA_FILE` | Path where data is stored in JSONL format | `/data/fas.jsonl` |
| `FAS_COOKIE_MAX_AGE` | Duration of the session cookie `fas_sid` in seconds | `7776000` (90 days) |
| `FAS_RECORD_TTL_SECS` | Hard expiration for any record in seconds | `2592000` (30 days) |
| `FAS_UNAPPROVED_TTL_SECS` | Soft expiration for unapproved visitor IDs in seconds | `3600` (1 hour) |
| `FAS_PURGE_INTERVAL_SECS` | Interval at which database TTL purges run | `3600` (1 hour) |
| `FAS_RATE_LIMIT_WINDOW_SECS`| Minimum interval between requests for unapproved visitors | `5` (5 seconds) |
| `FAS_SAVE_INTERVAL_SECS` | Throttle time before saving dirty state to disk | `30` (30 seconds) |

---

## Development and Building

### Prerequisites
- Rust & Cargo (1.71.0 or higher)

### Run locally
```bash
# Start the server locally
cargo run
```

### Run Tests
```bash
# Run unit and integration test suite
cargo test
```

### Build release binary
```bash
cargo build --release
```

---

## Docker Deployment

You can pull and run the pre-built multi-arch Docker image directly from GHCR:
```bash
docker run -d \
  -p 8080:8080 \
  -v /var/lib/fas-data:/data \
  -e FAS_DATA_FILE=/data/fas.jsonl \
  --name fas \
  ghcr.io/winguse/fas:latest
```

Alternatively, to build the Docker image locally:
```bash
docker build -t ghcr.io/winguse/fas:latest .
```

---

## Traefik Integration Example

Integrate FAS as a `ForwardAuth` middleware in your Traefik router setup.

### 1. Define the Middleware
```yaml
# YAML dynamic configuration
http:
  middlewares:
    fas-auth:
      forwardAuth:
        address: http://fas:8080/_auth
        trustForwardHeader: true
        authResponseHeaders:
          - "Set-Cookie"
```

### 2. Apply Middleware to Services
Attach the middleware to any router that requires visitor approval.

```yaml
http:
  routers:
    # Protect your main service
    my-app-router:
      rule: "Host(`app.example.com`)"
      service: my-app-service
      middlewares:
        - fas-auth
```

---

## Admin Panel & Bootstrapping Protection

### Option A: Restrict Admin Access at Proxy Level (Recommended)
Keep the administrator interface `/` and APIs `/api/*` protected by restricting them to local networks, VPNs, or requiring mTLS certificate verification.

```yaml
http:
  routers:
    # Admin Panel router protected by mTLS
    fas-admin-router:
      rule: "Host(`fas-admin.example.com`)"
      service: fas-admin-service
      tls:
        options: mtls-only # reference your Traefik mTLS configuration here
```

### Option B: Protect the Admin Panel using FAS itself
You can choose to place the admin dashboard `/` behind the `fas-auth` middleware as well. However, this creates a "chicken-and-egg" bootstrap problem: you cannot access the dashboard to approve your own session.

To approve the first administrator session (or approve visitors directly from the command line):

1. Access the application in your browser to generate a session cookie, and copy your visitor UUID from the pending approval page.
2. Run a `curl` command from your host machine (assuming port 8080 is mapped to your host) to approve that UUID:
   ```bash
   curl -X POST http://localhost:8080/api/users/<your-uuid>/approve
   ```
3. Refresh your browser page. Your session is now approved, and you can access the admin dashboard to approve other visitors.
