# Spec conformance

`nx-remote-cache-openapi.json` is the OpenAPI document published by Nx at
<https://nx.dev/docs/kb/self-hosted-caching> (copied verbatim, 2026-10-29).

`tests/spec_conformance.rs` reads that file and, for every operation and every response code it
declares, sends a real request to the server and checks the status and the declared content type.
It fails when the spec declares something the server does not produce, and when the server
produces a status the spec does not list for that operation (apart from the documented extras
below). Updating the spec file therefore updates the contract the tests enforce.

| Operation | Spec response | Server |
|---|---|---|
| PUT | 200 uploaded | ✅ |
| PUT | 401 missing/invalid token, `text/plain` | ✅ |
| PUT | 403 read-only token, `text/plain` | ✅ |
| PUT | 409 already exists | ✅ |
| PUT | `Content-Length` required | ✅ honoured; a chunked body is also accepted and counted |
| GET | 200 `application/octet-stream` | ✅ |
| GET | 403 forbidden, `text/plain` | declared by the spec, never emitted: every valid token may read |
| GET | 404 not found | ✅ |
| both | bearer auth | ✅ `Authorization: Bearer <token>` |

Extras the spec does not list (all outside the Nx client's happy path):
`400` invalid hash or incomplete body, `413` above `NX_CACHE_MAX_ARTIFACT_BYTES`, `502` store error.
