Full-Stack Example
==================

This `fullstack-example` directory aims to be a quick way to try out [Diskuto].

In everyday use, Diskuto has two main parts:

 * [diskuto-api] - The API server which lets you read/write content in the Diskuto network.
 * [diskuto-web] - A web UI that lets you do that in a browser.

You *can* host both of those things on different servers, but I suspect many people
might want to have them on the same server, so you can put them behind a web proxy
like Nginx.

This directory puts all three of those together in a [compose.yaml] file to simplify
testing.

You can also use the configuration files here as a starting point
to bootstrap your "production" setup.


Initial Setup
-------------

1. Install [Podman]

2. Install [podman-compose].

   Note: Docker and docker-compose might work here, but I'm testing this with Podman, so YMMV.
   TODO: Try Colima on mac to use standard Docker commands.

3. Build/download all images:  
   `./compose build`

4. Initialize the database.  
   `./compose run api diskuto db init`

   This just runs `diskuto db init` inside of the `api` container.
   It'll will create a `diskuto.sqlite3` database in the `./data/` directory.

5. Start services.  
   `./compose up`

First Post
----------

You should now be able to access an empty Diskuto instance at:
<http://localhost:9090>.

There's nothing interesting there yet, though. Just an empty "Home" page
and a "Log In" page.

### Create a UserID ###

The "Log In" page has a tool called "Create ID".  Click the "New" button to create one.

The `UserID` will be your global, public ID within Diskuto. For example:
`B2wQ3yuzGaaE6U5KbduAWwTNFZsX2RfspdUAFbmLjaEM`. (Don't worry, you'll have a name, too.)

The `Private Key` should be kept in a secure location like a password manager. You **only**
need your private key when you **write** to Diskuto. Browsing only needs your UserID.

"Log In" to the web UI by with your new UserID.


### Create Your Profile ###

I good idea for your first post is to publish a profile about yourself. This lets you set a
display name associated with your new user ID, and write a bit about yourself.

Click on "My Profile" to create a new profile. Fill in the Display Name field and type something about yourself.

You can ignore the other sections ("Servers" and "Follows") for now.

### Sign Your Profile ###

Any time you write content to Diskuto, you cryptographically sign it so that the server knows:

 * It's from you
 * It hasn't been altered in transit.

So to send our post, we first need to sign it.

Eventually, there will be a [browser plugin] that will automate this for us. Until then,
you can use diskuto-web's built-in signing tool to to this.

1. Click the "Copy Signing Request" button to copy your new post to the clipboard.
2. Click the "Open Signing Tool" link to use the built-in signing tool.
3. Paste the signing request.
4. Paste your private key.
5. Copy the generated signature at the bottom of the page.
6. Paste that signature back into the "Edit Profile" page.
7. Click the "Post" button to send your signed update to Diskuto.

And... it failed! You'll likely see something like this:

> Error uploading Item: 403 Forbidden

That's because we haven't yet told the server which users are allowed to
store content on it.

Keep this browser tab open, we'll come back to it and try again, right after we...

### Add A User To The Server ###

1. Open up a new terminal window in this directory.
2. Run: `./compose run api diskuto user add $yourUserID --on-homepage`

   Of course, replace `$yourUserID` with your public user ID.
   **The server does not need to know your private key.**

3. To verify the user was added, you can run:  
   `./compose run api diskuto user list`


Now return to your web browser and click the "Post" button again. This time it succeeds,
the server accepts your profile update.

Next Steps
----------

 * Try creating a post. The process for signing is similar to signing a profile update.
 * Follow a new user
   * Create a new userID.
   * In your old userID, add the new ID to its "follows" list and post your profile update.
     This lets the server (and other people) know that you're interested in updates from that user.
   * Log in as the new userID.
   * You can automatically post profile updates and posts because you're followed by someone on the server.
     (i.e.: no need to run `diskuto user add ...`)
 * Sync content from other servers using [diskuto-sync]
 * Sync content from RSS feeds using [rss-sync]
 * Sync content from Mastodon using [coming soon!]. 
 * Try browsing the content directly from the API using [diskuto-client]
 * Join the Discord server to chat about Diskuto and find other users to follow. (see link [here])


[browser plugin]: https://github.com/diskuto/diskuto-web/issues/3

[diskuto-sync]: https://github.com/diskuto/diskuto-sync
[rss-sync]: https://github.com/diskuto/rss-sync
[diskuto-client]: https://jsr.io/@diskuto/client
[here]: https://blog.nfnitloop.com/u/42P3FTZoCmN8DRmLSu89y419XfYfHP9Py7a9vNLfD72F/profile



[Diskuto]: https://github.com/diskuto
[diskuto-api]: https://github.com/diskuto/diskuto-api
[diskuto-web]: https://github.com/diskuto/diskuto-web
[compose.yaml]: ./compose.yaml
[Podman]: https://podman.io/docs/installation
[podman-compose]: https://github.com/containers/podman-compose
