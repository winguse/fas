# FAS — Forward Auth Service

FAS (Forward Auth Service) is a lightweight visitor access control service designed specifically for Traefik's ForwardAuth middleware. Written in Rust using the `axum` web framework, it provides visitor registration, granular **ACL control (using regular expressions)**, **cookie domain scope control**, rate-limiting, and an administrative dashboard to manage visitors and configuration.

## Features

- **Forward Auth Middleware Compatible**: Inspects and intercepts traffic via the standard `/_auth` route.
- **Granular ACL Control**: Define access control rules by HTTP Method, Domain, and URL Path using regular expressions (Regex).
  - **Deny-First Priority**: Deny rules take precedence over allow rules.
  - **Default Fallback Rules**: If no rules are defined in `acl.yaml`, `✅ allow_all` (allow everything) and `🚫 deny_all` (deny everything) are automatically initialized. New visitors receive an empty rule (`""`), defaulting to deny.
- **Cookie Domain Scope Control**:
  - **Default**: Omit the `Domain` attribute in `Set-Cookie` headers, restricting cookies strictly to exact host matches (excluding subdomains).
  - **Configurable Scope Mapping**: Map domain regex patterns to parent domain levels (e.g., `^.*\.b\.a\.com$: 1` writes `Domain=a.com`) or explicit domain strings.
- **YAML Configuration & Live Admin Editor**: Stored in standard YAML (`acl.yaml`). The Admin Panel includes a tab with a live editor featuring YAML syntax & regex linting and validation.
- **User Expiration (`expire_at`)**: Tracks visitor expiration timestamp `expire_at`. If missing, it is automatically computed based on default TTLs. Expired users are automatically purged by the background maintenance job.
- **In-Memory Store with Debounced Persistence**: Fast lookups with asynchronous JSONL persistence, debouncing disk writes (at most once every 30s) to maximize throughput and minimize disk wear.
- **IP-Based Rate Limiting**: Limit unapproved requests to 1 request per 5 seconds per IP, returning a `429 Too Many Requests` page with an interactive countdown timer.
- **Multi-lingual Support**: Automatically detects language preferences (`Accept-Language` headers) and serves pages in English (`en`) or Chinese (`zh-CN`).
- **Secure Dashboard**: Tabbed administrative interface to manage visitor ACL rules, search/filter users, and edit ACL/Cookie YAML configuration.
- **Shared Secret Protection**: Optionally enforce a `X-Shared-Secret` header (set via `FAS_SHARED_SECRET`) on all endpoints except `/_auth`, ensuring the admin portal and APIs are only reachable through the upstream proxy (e.g. Traefik) and not directly from other pods in the cluster.
- **Secure Runtime Container**: Multi-arch Docker images built on secure, ultra-minimal `gcr.io/distroless/cc-debian12`.

---

## Architecture and Code Modules

The application is structured cleanly:
- `src/main.rs`: Configures logging, spawns background tasks (saving, purging, rate-limit cleanup), and sets up Axum router.
- `src/config.rs`: Defines environment configuration settings.
- `src/acl.rs`: Core ACL engine, regex pattern compilation, YAML validation, and cookie domain scope resolution.
- `src/store.rs`: Manages in-memory user data, `expire_at` calculation, JSONL persistence, and TTL purging.
- `src/handlers.rs`: Axum handlers for `/_auth`, config validation/saving, stats, user management, and admin UI.
- `src/templates.rs`: UI templates for visitor cards, rate-limiting pages, and the tabbed admin dashboard.
- `src/i18n.rs`: Handles localization dictionaries (English & Chinese).

---

## Configuration

### Environment Variables

| Environment Variable | Description | Default Value |
| :--- | :--- | :--- |
| `FAS_PORT` | Port the web server binds to | `8080` |
| `FAS_DATA_FILE` | Path where user data is stored in JSONL format | `/data/fas.jsonl` |
| `FAS_ACL_FILE` | Path where ACL & Cookie YAML configuration is stored | `/data/acl.yaml` |
| `FAS_COOKIE_MAX_AGE` | Duration of the session cookie `fas_sid` in seconds (also defines record retention for allowed users) | `7776000` (90 days) |
| `FAS_UNAPPROVED_TTL_SECS` | Soft expiration for unapproved/denied visitor IDs in seconds | `3600` (1 hour) |
| `FAS_PURGE_INTERVAL_SECS` | Interval at which database TTL purges run | `3600` (1 hour) |
| `FAS_RATE_LIMIT_WINDOW_SECS`| Minimum interval between requests for unapproved visitors | `5` (5 seconds) |
| `FAS_SAVE_INTERVAL_SECS` | Throttle time before saving dirty state to disk | `30` (30 seconds) |
| `FAS_SHARED_SECRET` | If set, all requests (except `/_auth` and health probes) must include `X-Shared-Secret: <value>` — use this to prevent direct pod-to-pod access in Kubernetes; set the header in proxy and FAS will reject requests without it | *(unset — no check)* |
| `FAS_LOG_LEVEL` / `LOG_LEVEL` | Logging level (`debug`, `info`, `warn`, `error`, `trace`) | `info` |
| `FAS_LOG_FORMAT` / `LOG_FORMAT` / `FAS_LOG_JSON` | Log format (`json` or `text`) — structured JSON logging to stdout | `json` |

---

## Health, Readiness & Liveness Probes (Kubernetes / K9s)

FAS provides probe endpoints for Kubernetes and K9s health monitoring:
- **Liveness Probes**: `/livez`, `/_livez`, `/live`, `/healthz`, `/_health`, `/health`
- **Readiness Probes**: `/readyz`, `/_readyz`, `/ready`

These endpoints return `200 OK` with body `"OK"` and automatically bypass `FAS_SHARED_SECRET` enforcement.

---

## Proxy Integrations

### Traefik / Nginx
For Traefik and Nginx, ForwardAuth points to `http://fas:8080/_auth`. The original path is extracted from `X-Forwarded-Uri` or `X-Original-URI`.

### Envoy (`ext_authz`)
For Envoy HTTP `ext_authz`, Envoy appends the target path to the auth path (e.g., visiting `/abc` results in Envoy calling `/_auth/abc`). FAS extracts the target URL path from the suffix after `/_auth` (`/abc`) and evaluates it against ACL rules.

---

## ACL & Cookie Configuration Example (`acl.yaml`)

The `acl.yaml` file defines cookie domain scope rules and user ACL rules:

```yaml
# Cookie Domain Scope Mappings
# Keys are regex patterns matching the request host.
# Default behavior (if request host does not match any regex):
# No Domain parameter is included in the Set-Cookie header (restricting cookie to exact host match).
# Values MUST be integer levels N (validated >= 1 and <= current domain levels).
cookie_domains:
  "^.*\\.b\\.a\\.com$": 1                  # Matches foo.b.a.com -> Domain=a.com
  "^.*\\.sub\\.internal\\.net$": 2         # Matches app.sub.internal.net -> Domain=internal.net

# ACL Rule Definitions (Regex Patterns)
acl_rules:
  "✅ allow_all":
    allow:
      - method: ".*"
        domain: ".*"
        path: ".*"

  "🚫 deny_all":
    deny:
      - method: ".*"
        domain: ".*"
        path: ".*"

  developer_access:
    allow:
      - method: "^(GET|HEAD)$"
        domain: "^.*\\.dev\\.example\\.com$"
        path: "^/api/.*$"
      - method: "^POST$"
        domain: "^dev\\.example\\.com$"
        path: "^/api/v1/.*$"
    deny:
      - method: ".*"
        domain: ".*"
        path: "^/admin/.*$"
      - method: "^DELETE$"
        domain: ".*"
        path: ".*"
```

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
  -e FAS_ACL_FILE=/data/acl.yaml \
  --name fas \
  ghcr.io/winguse/fas:latest
```

---

## Traefik Integration Example

Integrate FAS as a `ForwardAuth` middleware in your Traefik router setup.

### 1. Define the ForwardAuth Middleware
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
    my-app-router:
      rule: "Host(`app.example.com`)"
      service: my-app-service
      middlewares:
        - fas-auth
```

### 3. Protect the Admin Portal with a Shared Secret (Recommended for Kubernetes)

In Kubernetes, other pods can reach the FAS admin portal and APIs directly via the cluster network. Set `FAS_SHARED_SECRET` and configure Traefik to inject `X-Shared-Secret` on every request to FAS. FAS rejects any request missing the header, so only traffic routed through Traefik can reach the admin UI.

**FAS deployment environment variable:**
```yaml
env:
  - name: FAS_SHARED_SECRET
    value: "your-strong-random-secret"
```

**Traefik middleware to inject the header:**
```yaml
http:
  middlewares:
    fas-inject-secret:
      headers:
        customRequestHeaders:
          X-Shared-Secret: "your-strong-random-secret"
```

**Apply both middlewares to the FAS router:**
```yaml
http:
  routers:
    fas-admin-router:
      rule: "Host(`fas.example.com`)"
      service: fas-service
      middlewares:
        - fas-inject-secret   # injects X-Shared-Secret before FAS sees the request
```

> **Note:** `/_auth` is always exempt from the shared secret check — Traefik's ForwardAuth calls that endpoint directly and does not need the header.

---

## Admin Panel & Bootstrapping Protection

### Option A: Shared Secret (Recommended for Kubernetes)
Set `FAS_SHARED_SECRET` and configure Traefik to inject `X-Shared-Secret` on every request to FAS (see **Traefik Integration** above). This ensures the admin portal and APIs are unreachable directly from within the cluster.

### Option B: Restrict at Proxy Level
Keep the administrator interface `/` and APIs `/api/*` protected by restricting them to local networks, VPNs, or requiring mTLS certificate verification at the proxy layer.

### Bootstrapping Admin Access
If the admin dashboard is placed behind `fas-auth`, assign your session cookie an allowed rule via `curl`:

1. Access the application in your browser to generate a session cookie, and copy your visitor ID from the pending approval page.
2. Run a `curl` command from your host machine to assign the `✅ allow_all` ACL rule to your session ID:
   ```bash
   # If FAS_SHARED_SECRET is set, include the header:
   curl -X POST http://localhost:8080/api/users/<your-uuid>/rule \
     -H "Content-Type: application/json" \
     -H "X-Shared-Secret: your-strong-random-secret" \
     -d '{"acl_rule": "✅ allow_all"}'
   ```
3. Refresh your browser page. Your session is now authorized, and you can manage visitors and ACL rules from the Admin dashboard.
