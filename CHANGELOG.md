Changelog
=========

Version 1.0.0
=============

Date: Feb. 3, 2025

 * Rename to [diskuto-api].

 * Moved to a new GitHub organization: [Diskuto]
 
 * UI has been rewritten and moved into its own repository as [diskuto-web].  
   Expect most noteworthy changes to happen in this repository moving forward, since
   the API changes much less often.
 
 * New [full-stack] example for local testing.
 
 * New OpenAPI schema for the REST API. See: [docs/rest_api/]

Breaking API Changes
--------------------

 * The REST API has been relocated to `/diskuto/`.  
   This makes it much easier to serve both [diskuto-api] and [diskuto-web] from one web host.
   See: [nginx/default.conf] for an example.

   Note: The old URLs are still available for backward compatibility but will soon be removed.

 * REST API endpoints have had `/proto3` suffixes removed.  
   These used to be there to distinguish between the HTTP and REST API URLs.

 * Some REST API paths have been renamed to be more user-friendly.  
   Ex: `/u/` -> `/users/`, `/i/` -> `/items/`.

 * See [docs/rest_api/] for details.


[diskuto-api]: https://github.com/diskuto/diskuto-api
[diskuto-web]: https://github.com/diskuto/diskuto-web
[Diskuto]: https://github.com/diskuto
[full-stack]: ./examples/full-stack/
[docs/rest_api/]: ./docs/rest_api/
[nginx/default.conf]: ./examples/full-stack/nginx/default.conf

Version 0.6.0
=============

<https://github.com/NfNitLoop/feoblog/releases/tag/v0.6.0>

New Features
------------

 * Support for the [Open Graph Protocol].  
   Now when you share links to other web sites, they'll be able to generate previews if they support OGP.

 * Quick access to share links.  
   Click the arrow at the top-right corner of a post to access share links.

 * `db prune` to remove data that's no longer being used.

 * `db usage` to see who's hogging all your disk space.  
   (See also: The [tablestream] crate I created to help with this output.)

[Open Graph Protocol]: https://ogp.me/
[tablestream]: https://crates.io/crates/tablestream

Improvements
------------

 * [#63] Faster Sync when syncing items between servers.
 * [#43] Better browser caching for static files.
 * Logged-in profiles now have a color picker.  
   You can set custom background colors for each identity you log in as to help keep them separate.

[#63]: https://github.com/NfNitLoop/feoblog/issues/63
[#43]: https://github.com/NfNitLoop/feoblog/issues/43

Bug Fixes
---------

 * Fixed [bug][svelte-spa-issue] (re)rendering widgets when changing pages.

[svelte-spa-issue]: https://github.com/ItalyPaleAle/svelte-spa-router/issues/234


Version 0.5.1
=============

Released: July 24, 2021  
<https://github.com/NfNitLoop/feoblog/releases/tag/v0.5.1>


Improvements
------------

 * Better handling of attachment uploads.  
   For cases where it already had a copy of an attachment on the server,
   FeoBlog would close the HTTP connection early with a 202 status. This
   didn't work well in Chrome or with Deno. Now we'll try to be a bit
   more friendly to the client.

   As an user, you *may* notice slightly faster uploads of duplicate files.
   Though I don't expect that that's a common case.

Version 0.5.0
=============

Released: July 18, 2021  
<https://github.com/NfNitLoop/feoblog/releases/tag/v0.5.0>

New Features
------------

 * You can now filter and search your "My Feed" page.  
   Is someone posting a bit too much today? You can temporarily hide them from
   your feed to see what everyone *else* has to say. Looking for a post you saw
   last week? Now you can search for a keyword and view only posts/comments that
   mention that.

   ![v0.5.0 filter demo]

[v0.5.0 filter demo]: ./docs/images/v0.5.0%20filter%20demo.gif

Improvements
------------

 * Posts are no longer clickable.  
   Previously, the entire block containing a post was clickable, and would take
   you to the page for that post. But that resulted in a lot of accidental
   clicks. Also, since the cursor changed to a pointer for the whole block, it
   was difficult to see if images were clickable. Now that behavior is gone. You
   can click on the timestamp of a post to go to a page for just that post.

 * [#52] Automatically redirect to the "My Feed" page when logged in.  
   If you're logged in, you're probably repeatedly coming to FeoBlog to check
   your feed. So that's now the default view.

[#52]: https://github.com/NfNitLoop/feoblog/issues/52

Bug Fixes
---------

 * [#51] Draft timestamps could accidentally backdate posts.
 * Fix for nav links that wouldn't highlight when they were clicked.

[#51]: https://github.com/NfNitLoop/feoblog/issues/51


Version 0.4.0
=============

Released: June 25, 2021  
<https://github.com/NfNitLoop/feoblog/releases/tag/v0.4.0>

 * The web client is now the default view.  
   FeoBlog has two ways to access content. One is plain HTML (A.K.A.: Web 1.0),
   which works well for old browsers and search engines. The other is a web
   client (Web 2.0), which has a nicer interface. Now, if you visit a page in a
   browser that supports JavaScript, you'll get automatically redirected to the
   newer, nicer web client.

 * Post drafts are now saved.  
   If you navigate away from the "New Post" page and come back later, your post
   will still be there. Whew!

 * Added some helpful warnings when writing markdown posts  
   Now if you forget to link that `[reference]`, you'll get a warning reminding
   you to add a link.

 * Better support for password managers  
   You should save your private key ("password") in a password manager. But some
   password managers were filling in the wrong fields. Hopefully that's fixed.
   (If not, please open an issue!)

 * An updated [README] to explain the core principles behind FeoBlog's design

 * Support for attachments on iOS (and probably Android)  
   Oops. You can't easily drag-and-drop on a phone, so I added a button to
   attach files. Now you can take photos and easily upload them from your phone!

 * Improved automatic link generation when adding attachments  
   When you add an attachment to a post, FeoBlog will generate a `[link]` and a
   `[link]: files/reference.example` for you. Now it'll do a better job of
   placing those within an existing document.

[README]: ./README.md

Version 0.3.0
=============

Released: Feb. 25, 2021  
<https://github.com/NfNitLoop/feoblog/releases/tag/v0.3.0>

New Features
------------

 * Attachments!  
   You can now attach files to posts by dragging them onto the post editor. A
   link to the file will be automatically generated for you. If the file is an
   image, it'll be inlined in your post by default. Syncing between servers will
   also sync file attachments.
 * Release automation.  
   This is more for me than for y'all, but the result of this is that releases should be regularly available via the [releases] page.

[releases]: https://github.com/NfNitLoop/feoblog/releases

Note: There's a known issue ([Bug #16]) that is preventing Windows builds from working at the moment. I'll enable Windows builds when that's fixed.

[Bug #16]: https://github.com/NfNitLoop/feoblog/issues/16


Improvements
------------

 * Switched to comrak for server-side (plain HTML) rendering of Markdown.  
   Users shouldn't notice any changes, but this library operates in a "safe by
   default" mode which is nice.
 * Improved browser caching.   
   Protobuf "Items" (and file attachments) are now served with HTTP headers to
   allow browsers to cache them indefinitely, since they should never change.
 * SQLite's "Write Ahead Logging" ("WAL") mode is now enabled when available,
   which greatly increases write throughput when syncing. This also means that
   reads and writes do not block each other.
 * Disable in-browser signature verification during sync.  
   This further improves sync speed, since in-browser crypto is particularly slow.
   The server will still validate that the objects it receives are cryptographically
   signed. (And the in-browser client still always verifies content signatures before
   displaying them.)
  

Bug Fixes
---------

 * Fixed some minor rendering issues when viewing a server w/ no posts.

Version 0.2.2
=============

Released: Feb. 5, 2021

Bug Fixes
---------

 * You need not be logged in to view comments in the client.

Version 0.2.1
=============

Released: Feb. 5, 2021

Bug Fixes
---------

* Fix a broken build.  
  Building in dev mode doesn't necessarily expose all the broken parts. Oops!


Version 0.2.0
=============

Released: Feb. 5, 2021

New Features
------------

 * Comments!
    * You can leave comments on any post or comment.
    * Comments only show up in your feed in the client, not on the plain HTML
      version of the site, or on the home page.
 * Lots of style updates. Items (post/comments) now have a single-line header of
   metadata.
 * [Identicons]!  
   For users that don't have a profile photo (which is everyone, at the moment),
   you'll get a default identicon based on your user ID. This makes it easy to
   quickly see who posts are from in your feed.
 * Server-side support for replies.
   * List all replies to a given Item via `/u/:userID/i/:signature/replies/proto3`
   * Index reply Items so it's efficient to look them up.
 * New `feoblog db upgrade` command to keep your database up-to-date with the
   latest versions of FeoBlog.
 * Relative timestamps. (But you can mouse-over a date to see absolute times.)

[Identicons]: https://en.wikipedia.org/wiki/Identicon


Improvements
------------

 * Much faster loading on endless-scrolling pages of items.
 * Posting a new Item will take you to its page after upload.
 * Much simplified Sync process.  
   The "bootstrap" step from v0.1.0 is no longer necessary.
 * Improved sync progress logging.  
   Click on subsections to show/hide as much detail as you care to see.
 * Explicit `feoblog db init` command.  
   This will require an extra step during initial setup, but it means you can't
   accidentally create new databases.
 * Refactored database code so that it should be even easier to replace w/
   alternate implementations if/when they exist.
 * Add the `<meta ...>` tag to make iOS devices happy.  
   I still haven't done a whole lot of testing on these devices, but the scaling
   is now much better at least.

Bug Fixes
---------

 * Disallow uploading Items dated in the future.  
   This was likely due to user error. The server doesn't show future items, so it
   also resulted in unexpected behavior.

Version 0.1.0
=============

Released: Jan. 20, 2021

New Features
------------

 * First release!
 * Posts
 * Profiles
 * Syncing between servers
 * Plain HTML "Web 1.0" version
 * In-browser JavaScript client.
