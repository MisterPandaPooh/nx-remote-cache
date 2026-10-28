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
