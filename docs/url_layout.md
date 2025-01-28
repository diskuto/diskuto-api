URL Layout
==========

Documentation for the REST API has been moved into the [rest_api](./rest_api/) subdirectory.

The rest of this document makes suggestions for a common layout for web UIs for Diskuto. A common set of URL paths 
will make it easy for users to look up content on different web UIs without having to learn an entirely new set of URLs.

More importantly, since users write posts and comments using Markdown,
having a standardized URL layout means users can reliably use relative links
to resources. For example, a user might link to 
`/u/A719rvsCkuN2SC5W2vz5hypDE2SpevNTUsEXrVFe9XQ7` in order to link to the
"Official Diskuto" user.

If your implementation does not want to use that URL path for whatever reason,
it's strongly advised that it at least redirect to the appropriate path in your
implementation.

Paths
=====


`/`
---

There is no specific requirement for the root `/` path of a server, so you're welcome
to display whatever kind of interface you find appropriate here. For example, It may be a stream
of latest posts on the server, or of a single user's posts, if the server is the home of a single user.

The diskuto-web implementation uses this URL to redirect a viewer to either to
a non-standardized "home" view (`/home`) or to a user's "feed" if they have 
"logged in" (set a cookie) as that user.

`/u/<userID>/`
------------

This endpoint should generally list a user's posts in reverse chronological
order (most recent posts first). Whether those posts are shown in-line or as
links to the full posts is up to the implementation.

You might also display information about a user, such as their preferred name(s),
number/size of posts, "home server", etc., either inline or as links.

`/u/<userID>/i/<signature>/`
------------------------

URLs of this format point to a single piece of content from a user. The server
should render it for viewing.

 * `userID` is the base58-encoded NaCL public key.
 * `signature` is the base58-encoded signature of the post.

Rendering may take different forms for different types of content. I expect the
common case will be rendering a [CommonMark] post, or a reply to someone else's
post. 

[CommonMark]: https://commonmark.org/


`/u/<userID>/feed/`
-------------------

Renders a view of posts from users that this user follows, according to their
latest profile. The user's own posts may be included here as well.

`/u/<userID>/profile/`
-------------------

Renders a view of the user's latest `Profile`. Views should include:

* The userID
* The user's name (`displayName`)
* The user's `about` text.
* Who this user follows.
* The "servers" list from the user's profile. 
* Other information from the user's profile, or relevant to this user.


`/diskuto/`
----------

This path is reserved for use by the Diskuto API.

This allows serving the API and the web UI on the same host behind a web proxy.