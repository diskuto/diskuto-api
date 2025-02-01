Data Format
===========

Goals
-----

 * User data should be easy to cache on multiple distributed servers, or for
   offline use.
 * Different servers may have different policies about which or how much data
   different users may store/cache on them.
 * User data should be publicly verifiable using public cryptographic
   signatures.

Implementation
--------------

Diskuto uses Protocol Buffers (v3) to define a compact encoding for blog posts
and other data. These types are currently defined in [diskuto.proto].

[diskuto.proto]: ../protobufs/diskuto.proto

Users/software create these protobuf objects, sign them, and send them to one or
more servers to host. Unlike Scuttlebutt, or typical blockchain data structures,
the items do not depend on previous items, so can be considered a (mostly)
unordered collection. This allows servers to truncate history, deny, or even
delete items as their local policies may require.

See [crypto.md] for details of how signing works.

[crypto.md]: ./crypto.md