---
tags:
  - reference
  - obsidian
  - api
aliases:
  - obsidian-local-rest-api
  - obsidian rest api
date created: Tuesday, March 31st 2026, 12:14:49 am
date modified: Tuesday, March 31st 2026, 3:25:22 am
---

# Obsidian Local REST API - Reference

> __Source:__ <https://github.com/coddingtonbear/obsidian-local-rest-api>
> __Full OpenAPI Spec:__ [[obsidian-local-rest-api-openapi.yaml]]

## Authentication

All endpoints (except `GET /`) require Bearer token authentication:

```ts
Authorization: Bearer <your-api-key>
```

Find the API key in Obsidian Settings > Plugins > Local REST API.

## Base URL

- __HTTPS:__ `https://127.0.0.1:27124` (self-signed cert)
- __HTTP:__ `http://127.0.0.1:27123`

The plugin generates a self-signed certificate on first run. The cert can be fetched from `/obsidian-local-rest-api.crt`.

---

## Endpoints

### System

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Server status & auth check (no auth required) |
| GET | `/obsidian-local-rest-api.crt` | Returns the TLS certificate |
| GET | `/openapi.yaml` | Returns OpenAPI spec |

__GET /__ response:

```json
{
  "authenticated": true,
  "ok": "OK",
  "service": "Obsidian Local REST API",
  "versions": { "obsidian": "...", "self": "..." }
}
```

---

### Vault File Operations (`/vault/{filename}`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/vault/{filename}` | Read file content |
| PUT | `/vault/{filename}` | Create or overwrite file |
| POST | `/vault/{filename}` | Append content to file |
| PATCH | `/vault/{filename}` | Partially update content (heading/block/frontmatter targeting) |
| DELETE | `/vault/{filename}` | Delete file |
| GET | `/vault/` | List files in vault root |
| GET | `/vault/{dirpath}/` | List files in directory (trailing slash = directory) |

__GET (read file):__
- Default `Accept`: returns raw markdown (`text/markdown`)
- `Accept: application/vnd.olrapi.note+json`: returns JSON with parsed frontmatter, tags, stats
- `Accept: application/vnd.olrapi.document-map+json`: returns headings, block refs, frontmatter fields

__NoteJson schema:__

```json
{
  "content": "string (markdown)",
  "frontmatter": {},
  "path": "string",
  "stat": { "ctime": 0, "mtime": 0, "size": 0 },
  "tags": ["string"]
}
```

__GET (directory listing):__
- Returns `{"files": [{"path": "…", "stat": {…}}]}`

__PUT (create/overwrite):__
- Body: file content
- `Content-Type: text/markdown` for notes
- `If-None-Match: *` header: only create if file doesn't exist (returns 412 if it does)

__POST (append):__
- Body: content to append
- `Content-Type: text/markdown`
- Content is appended to end of file

__DELETE:__
- Returns 204 on success, 404 if not found

---

### PATCH Operations (Surgical Edits)

The PATCH method enables targeted content modifications within a note.

__Required Headers:__

| Header | Values | Description |
|--------|--------|-------------|
| `Operation` | `append`, `prepend`, `replace` | What to do |
| `Target-Type` | `heading`, `block`, `frontmatter` | What kind of target |
| `Target` | string | The specific target (heading path, block ref ID, frontmatter field name) |

__Optional Headers:__

| Header | Default | Description |
|--------|---------|-------------|
| `Target-Delimiter` | `::` | Separator for nested headings (e.g., `Heading 1::Subheading`) |
| `Trim-Target-Whitespace` | `false` | Trim whitespace from target |
| `Create-Target-If-Missing` | `false` | Create the target if it doesn't exist |

__Examples:__

Append under a heading:

```ts
PATCH /vault/path/to/note.md
Operation: append
Target-Type: heading
Target: Heading 1::Subheading 1:1:1
Content-Type: text/markdown

Hello
```

Replace a frontmatter field:

```ts
PATCH /vault/path/to/note.md
Operation: replace
Target-Type: frontmatter
Target: status
Content-Type: application/json

"done"
```

Append to block reference:

```ts
PATCH /vault/path/to/note.md
Operation: append
Target-Type: block
Target: 2d9b4a
Content-Type: text/markdown

New content here
```

---

### Active File Operations (`/active/`)

Same methods as vault files (GET, PUT, POST, PATCH, DELETE) but operates on the currently open file in Obsidian. No `{filename}` parameter needed.

---

### Periodic Notes (`/periodic/{period}/`)

| Method | Path | Description |
|--------|------|-------------|
| GET/PUT/POST/PATCH/DELETE | `/periodic/{period}/` | Current period's note |
| GET/PUT/POST/PATCH/DELETE | `/periodic/{period}/{year}/{month}/{day}/` | Specific date's note |

__`{period}` values:__ `daily`, `weekly`, `monthly`, `quarterly`, `yearly`

Same request/response semantics as vault file operations.

---

### Search

__Simple Search:__

```ts
POST /search/simple/?query=search+terms
```

Returns matching filenames with context snippets.

Response:

```json
[
  {
    "filename": "path/to/note.md",
    "score": 0.95,
    "matches": [{ "match": { "start": 0, "end": 10 }, "context": "..." }]
  }
]
```

__Structured Search:__

```ts
POST /search/
Content-Type: application/vnd.olrapi.dataview.dql+txt

TABLE file.name, file.mtime FROM "folder" WHERE contains(tags, "#coffee")
```

Or with JsonLogic:

```ts
POST /search/
Content-Type: application/vnd.olrapi.jsonlogic+json

{"and": [{"glob": [{"var": "path"}, "daily/*"]}]}
```

---

### Commands

| Method | Path | Description |
|--------|------|-------------|
| GET | `/commands/` | List all available commands |
| POST | `/commands/{commandId}/` | Execute a command |

__GET /commands/__ response:

```json
{
  "commands": [
    { "id": "global-search:open", "name": "Search: Search in all files" },
    { "id": "graph:open", "name": "Graph view: Open graph view" }
  ]
}
```

---

### Tags

| Method | Path | Description |
|--------|------|-------------|
| GET | `/tags/` | List all tags with usage counts |

---

### Open File in Obsidian UI

```ts
POST /open/{filename}?newLeaf=true
```

Opens the specified file in Obsidian's interface. Creates the file if it doesn't exist.

---

## Error Responses

All errors return:

```json
{
  "errorCode": 40149,
  "message": "A brief description of the error."
}
```

Error codes are 5-digit numbers unique to each error type.

---

## Key Endpoints for Pour

Based on the [[pour-design-spec|project spec]], these are the most relevant endpoints:

1. __`GET /`__ - Check if API is available (connectivity test)
2. __`PUT /vault/{filename}`__ - Create new notes (coffee logs, music sets)
3. __`PATCH /vault/{filename}`__ - Append under headers (journal entries)
4. __`GET /vault/{dirpath}/`__ - List files in directory (populate dynamic dropdowns)
5. __`POST /search/`__ - Dataview queries for dynamic selects by tag


