Cryptography in Diskuto
=======================

To minimize attack surface, Diskuto API server never stores any
user's private key. It only ever has users' public keys. These also serve as
globally unique user IDs, so you'll see them referred to as UserID in docs.
Servers learn of users' public keys in various ways:

 * A user's ID is manually added to the server via the CLI.
 * Someone already known to the server
   * "Follows" another user
   * Interacts with another user's content (ex: commenting on a post).

Additionally, the server does not use cookies to maintain any session state with
clients. To post new content to the server, a user signs a piece of [data], and
`HTTP PUT`s it to a URL containing the user ID and signature. With this
information, the server can verify that the request comes from the user, or
someone working on their behalf (who has not modified the original content).

Diskuto uses [NaCl] signing keys, which are encoded as base58 strings in URLs.

[data]: ./data_format.md 
[NaCl]: https://en.wikipedia.org/wiki/NaCl_(software)
[Base58]: https://en.wikipedia.org/wiki/Base58


One User ID, One Key
--------------------

Modern cryptography systems often allow for a single identity to have multiple
associated "device" keys. (ex: [Keybase], [Wire]) This means that if any one
device is lost or compromised, only that (sub)key needs to be revoked.

[Keybase]: https://keybase.io/blog/keybase-new-key-model 
[Wire]: https://wire-docs.wire.com/download/Wire+Security+Whitepaper.pdf

However, that model requires a centralized source of authority for which keys
are and or are not (any longer) valid for a user ID. Both Keybase and Wire
assign human-readable, globally-unique user IDs which can be used to look up a
user's currently valid keys. But that's not viable if you want a truly P2P
system.

Other systems (ex: PGP) solve this by having a "main" key and "sub" keys which
can be created and revoked by the main key. There are two reasons why this
approach was not chosen:

1. Mainly, it complicates the interface for the user and the software developer.
   Adding a layer of indirection, and N keys, and cache invalidation to the
   problem adds more edge cases than I care to think about at this point.

   One of the complicated edge cases is the question of what to do with content
   that was signed by a key that has now been revoked.
   
   Does it get deleted? Now portions of the user's blog history are just
   missing.
   
   And if we don't delete that content, what's to stop someone from posting more
   content signed with the revoked key? (Servers have an eventual consistency
   model, so they *should* allow back-filling any missing history in
   general.)

2. More importantly, being able to create and delete subkeys (and their
   content) essentially allows users to abuse subkeys to treat their blog as if
   content is able to be deleted, when that is not a use case I want to support
   in Diskuto.

   If a user posts some content that they receive criticism for, they should not
   be able to just revoke a subkey to remove the content. If they continue to
   use their userID (which may have accumulated many followers), the content
   should remain verifiable as authored by that ID.

   Users have two other options to deal with the above case: They may post a
   follow-up comment clarifying/denouncing their controversial post, or they can
   [revoke] their entire UserID and start fresh.

[revoke]: https://github.com/diskuto/diskuto-api/issues/5

Note that this does not prevent users from manually creating "sub-blogs" and
manually performing such key management. One likely scenario I can imagine:  A
user has a "main" blog, and in their profile they link to other blogs (user IDs)
that they also post to.