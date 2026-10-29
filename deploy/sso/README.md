# SSO for the dashboard (Keycloak, OIDC or SAML)

The cache API is used by machines (Nx CLI sends a static bearer token), the dashboard is used by
people. SSO therefore applies to the dashboard only, and it is delegated to an authenticating
reverse proxy instead of being built into the server:

1. Deploy `oauth2-proxy-keycloak.yaml` (fill in the Keycloak realm, client secret, cookie secret, hosts).
2. Point the Ingress at the `oauth2-proxy` service instead of `nx-remote-cache`.
3. Set `NX_CACHE_UI_TRUSTED_HEADER=X-Auth-Request-Email` on the server and make sure the
   `nx-remote-cache` service is not exposed outside the cluster (a NetworkPolicy allowing only
   the proxy is the usual way). The server then opens the dashboard to any request carrying
   that header, and keeps requiring a cache token on `/v1/cache/*`.

**SAML.** Keycloak speaks SAML as an identity broker: add the corporate SAML IdP under
*Identity Providers* in the realm, and keep the OIDC client above for oauth2-proxy. Users log in
through SAML, the proxy only ever sees OIDC. This avoids shipping an XML-signature stack in the
cache server and works the same with any OIDC or SAML provider Keycloak can front.

Without a proxy, the dashboard asks for a token in the browser (`NX_CACHE_UI_TOKEN` or any cache token).

## Kong Gateway

Same model with Kong in front of the service: one route for the dashboard, one for the cache API.

| Route | Plugins | Effect |
|---|---|---|
| `/`, `/api/*` | `saml` (or `openid-connect`) + `request-transformer` adding `X-Auth-Request-Email` from the authenticated consumer | people log in through Keycloak or SAML; the server trusts the header (`NX_CACHE_UI_TRUSTED_HEADER=X-Auth-Request-Email`) |
| `/v1/cache/*` | `key-auth` + `request-transformer` replacing `Authorization` with `Bearer <server cache token>` | Nx CLI users get per-team Kong API keys; the server token never leaves Kong |

```yaml
# decK-style snippet
services:
  - name: nx-remote-cache
    url: http://nx-remote-cache.nx-remote-cache.svc
    routes:
      - name: dashboard
        paths: ["/", "/api"]
        plugins:
          - name: saml   # Kong Enterprise. Community edition: openid-connect against Keycloak (which can broker SAML).
            config:
              issuer: nx-remote-cache
              idp_sso_url: https://keycloak.example.com/realms/<realm>/protocol/saml
              idp_certificate: "<IdP signing certificate>"
              assertion_consumer_path: /saml/acs
          - name: request-transformer
            config:
              add: { headers: ["X-Auth-Request-Email:$(headers.x-consumer-username)"] }
      - name: cache-api                  # /v1/cache and /v1/kv: bearer tokens, no SSO
        paths: ["/v1"]
        plugins:
          - name: key-auth
          - name: request-transformer
            config:
              replace: { headers: ["Authorization:Bearer ${NX_CACHE_TOKEN}"] }
```

Keep the Kubernetes Service reachable only from Kong (NetworkPolicy); otherwise the trusted header is a bypass.
