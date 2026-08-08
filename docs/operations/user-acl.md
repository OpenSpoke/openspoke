# User ACL and admin role

*Introduced in v2.0.*

OpenSpoke v2.0 splits the "am I an operator?" and "am I an admin?"
questions into two well-defined mechanisms:

- **Admin role** — a Keycloak realm role (`openspoke_admin`) drives every
  hub-side `is_admin()` check. There is no hard-coded email allowlist in
  the frontend or MCP server.
- **Per-user ACL** — an OpenSearch index (`user_acl`) holds one document
  per user with fine-grained scopes (which topics they can read/write,
  which knowledge namespaces they can query). The frontend and the MCP
  server share the same index.

Both the browser frontend and Claude-connected MCP clients look at the
same identifier for a given user, so an admin adjustment applies to both
interfaces in one place.

## Adding an admin

1. In Keycloak, open the OpenSpoke realm.
2. Assign the `openspoke_admin` realm role to the target user.
3. That's it. On the user's next request, oauth2-proxy will forward the
   role in `X-Auth-Request-Groups`, and every `is_admin()` call will
   return true. No hub-side restart needed.

## Emergency fallback: `_ADMIN_EMAILS`

The frontend's dashboard code retains a small hard-coded
`_ADMIN_EMAILS = {"admin@example.com"}` set as a Keycloak-outage
fallback. In normal operation this set is empty (or contains only
placeholder addresses that will never authenticate) and the realm role
is authoritative.

Do not use this fallback as your primary admin channel. Rotate the
placeholder addresses out as soon as Keycloak is available again.

## Per-user ACL

Every authenticated user has one document in the OpenSearch `user_acl`
index. On first sign-in the document is auto-created with an empty ACL
(default-deny). Admins edit the document either through the frontend's
"Users" tab or by editing OpenSearch directly.

The ACL controls:

- Which knowledge collections the user may read from
  (`search_rag`, `retrieve_chunks`).
- Which knowledge collections the user may write to
  (`upsert_knowledge`, `delete_knowledge`).
- Which topics from `system_topics()` are visible.

A user whose ACL is empty can still authenticate and see their own name;
they simply cannot read or write any collection until an admin grants a
scope.

## Auditing

The frontend logs every admin action (ACL edit, role assignment lookup)
with the acting user's `preferred_username`, timestamp, and diff. Point
your log stack at those events for compliance evidence.

## Migrating from v1

In v1.0, admin status was inferred from a hard-coded email list embedded
in the dashboard code. To migrate:

1. Deploy the v2.0 dashboard code (Keycloak role check active).
2. Assign the `openspoke_admin` role in Keycloak to every user who was
   previously in the email list.
3. Verify by having each admin sign in and confirm they see the admin UI.
4. Empty the `_ADMIN_EMAILS` fallback set in the dashboard ConfigMap.

No index changes are required; the `user_acl` index is created lazily on
first write.
