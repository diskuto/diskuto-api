Diskuto API
=======

Diskuto is a distributed blogging/social networking system designed to protect
you as a user. Diskuto does this based on some core principles:

1. Your data should not be held hostage by a single service. (ex: Facebook,
   Twitter).  
   If you decide you don't like a service, you should be able to easily copy and
   reuse your data elsewhere. Likewise, your user ID should be able to migrate
   with your data so that your followers know you're the same user in both
   places.

2. Your data should be resilient to censorship and server outages.  

3. Your data should not be modifiable by third parties.  
   People reading your posts should be confident that it has not been altered.
   e.g.: Servers or other middlemen should not be able to insert ads into your
   data.

4. You should be able to create/use clients to view your data as you wish.  
   This is unlike platforms like Facebook and Twitter that make it difficult to
   access your social network's data.

5. As a server administrator, you should be able to block content as required by
   law for your jurisdiction.

For more information on how Diskuto accomplishes this, see: [How Does It Work?]

[How Does It Work?]: ./docs/how_does_it_work.md


### Other features ###

 * Uses a safe subset of CommonMark markdown for posts.
 * Can easily run a server locally
   * Sync content from those you follow to have offline.
   * Compose posts offline, and send them all when you're back online.
 * Comments
 * File attachments
 * Server renders a plain HTML version of content that is viewable and indexable online.  
   Ex: <https://blog.nfnitloop.com/u/A719rvsCkuN2SC5W2vz5hypDE2SpevNTUsEXrVFe9XQ7/>
 * Server also includes an in-browser client for viewing & posting.  
   Ex: <https://blog.nfnitloop.com/client/#/u/A719rvsCkuN2SC5W2vz5hypDE2SpevNTUsEXrVFe9XQ7/>


Getting Started
===============

If you don't want to set up your server right away, check out the [v0.1 Demo on YouTube](https://youtu.be/LJMhiNUuCqI)!

If you want to build the Diskuto API from source, or modify it, see the [Development] documentation.

[Development]: docs/development.md

Otherwise, you can download a prebuilt binary from the [GitHub Releases] page.

[GitHub Releases]: https://github.com/diskuto/diskuto-api/releases


Old, Out-Of-Date Getting Started
================================

⚠️⚠️⚠️ These instructions are based on the old, all-in-one "FeoBlog" binary. Since the rename to Diskuto, setup has changed a bit.

I'm leaving these here for now, but they need to be updated to the new system.

Run the server
--------------

Once you've built or downloaded feoblog, you can run it locally by just running:

```
feoblog serve --open
```

This will:
 * Start a server on localhost:8080. (You can override w/ the `--bind` option)
 * Create a database called feoblog.sqlite3 in the current directory.
 * Open a web browser window pointing to your new empty database.

Create a User ID
----------------

By default, `http://localhost:8080/` will show you the plain HTML version of the site. This is a version of the site that's indexable by search engines, and readable by software and browsers that don't have JavaScript available.

To do more than just read existing content, you'll need to use a client. FeoBlog comes with a built-in client that runs in your browser.

Click the "Client" link to open the in-browser client.

Next, click the "Log in" link.

At the bottom of the page, click the "Create new ID button". This will generate a new user ID for you. It'll look like a random string of characters. For example: `A719rvsCkuN2SC5W2vz5hypDE2SpevNTUsEXrVFe9XQ7`.

The page will also generate a password for you. It's important to **save this key in a secure location** like a [password manager]. You can't change or reset this password. (It's a cryptographic private key that corresponds to your public ID.) 

[password manager]: https://en.wikipedia.org/wiki/Password_manager

Add Yourself to the Server
--------------------------

Now that you've generated a userID for yourself, you need to tell the server who you are.

A FeoBlog server doesn't contain any passwords, all it knows is a list of user IDs that are allowed to post content to it. Since all content is cryptographically signed by a user, the server can verify that a post came from you without your password.

So, using the userID above, to add myself to the server I'd just run:

```
diskuto user add A719rvsCkuN2SC5W2vz5hypDE2SpevNTUsEXrVFe9XQ7 --on-homepage --comment "Official FeoBlog Blog"
```

You can do this by stopping the server with Ctrl-C first, or by running the command in a new window. (But if you stopped the server, make sure to re-start it before the next steps!)

The optional `--on-homepage` argument says that posts you post to this ID should appear on the Home page as well as in their own user page.

And the optional `--comment X` argument is just a comment to help you, the server admin, keep track of who that ID is. It's only ever shown in the output of `diskuto user list`.

Log In
------

Return to the "Log In" page in the browser.

Paste your userID (not password!) into the "Log In As:" field, and click the "Log In" button. You'll get a warning that your profile doesn't exist. (We'll write one next!) Click the "Confirm" button.

Now you're "logged in". You may be surprised that no password was required. "Logging in" to the client just tells it to present data to you as if you are that user. Don't worry, a password will be required to write any data.

The client is built with the idea that you may manage multiple identities. If you generate another ID, you can "log in" as that identity as well, and the client will remember so that you can easily switch between them.

To help distinguish your identities, you can (and should!) give them different names and colors. Just edit the "Name" and "Color" fields as you wish. Changes are saved immediately. Colors should be 3- or 6-digit hexadecimal colors like `#03c` or `#0033cc`. Here's a handy [color picker] for you.

[color picker]: https://www.w3schools.com/colors/colors_picker.asp

Create Your Profile
-------------------

User IDs are not a great way to remember people. Thankfully FeoBlog supports user profiles. There, you can set a name for yourself and provide a short description of yourself and/or the purpose of your blog.

Click on the "My Profile" link.

FeoBlog again warns you that it can't find an existing profile for you. If a profile does exist, you have the option to "Sync from another server" so that you can re-use (or modify) the existing one. But since we're creating a new ID, click "Create New Profile".

For now, the important parts to set are:

 * Profile Display Name -- this is a friendly name or nickname to display instead of your long randomly-generated userID. You *should* set this, but it's not required, if you really prefer being an anonymous number.
 * The "Your profile here..." text box serves as a description for you and/or your blog. Write whatever you want. This box accepts "CommonMark" Markdown formatting.

Once you've got those set, now paste your password into the "Private Key" field, and click "Sign".

This will automatically generate a cryptographic signature for this content. If everything still looks good, click the "Send" button to send this profile to the server.

Write Your First Post
---------------------

Click on the "New Post" link at the top/left of the page.

This interface is very much like the "My Profile" page. Fill out the title and body of your post, then sign and send the post.

You can view it in "My Feed" and (if you enabled `--on-homepage` above) on the "Home" feed.

Linking
-------

Since FeoBlog content is distributed, and may be hosted on multiple servers, you should avoid hard-linking to a particular server. If you want to link to a FeoBlog userID or post within your post, use relative links, like this:

```
Did you see the [FeoBlog] [first post]?

[FeoBlog]: /u/A719rvsCkuN2SC5W2vz5hypDE2SpevNTUsEXrVFe9XQ7/
[first post]: /u/A719rvsCkuN2SC5W2vz5hypDE2SpevNTUsEXrVFe9XQ7/i/2F6NB6PYKDTPGTc9dfaQHpmPzd3LSjVgBuC6qa2hcLUJA74LbZpV8wL5HoXDmvzyfZWaX6sLyg3DoGtqh3t2rJt5/
```

Of course, if you're linking someone *outside* of FeoBlog to a particular post, you can link them directly to a page on a particular server like this:

<https://blog.nfnitloop.com/u/A719rvsCkuN2SC5W2vz5hypDE2SpevNTUsEXrVFe9XQ7/i/2F6NB6PYKDTPGTc9dfaQHpmPzd3LSjVgBuC6qa2hcLUJA74LbZpV8wL5HoXDmvzyfZWaX6sLyg3DoGtqh3t2rJt5/>

The URLs are a bit long, but many services (like Twitter) will shorten them for you anyway. Plus, the URL contains a globally unique ID which can also be used to cryptographically verify the contents of the post. If any one server goes down, the `/u/...` relative path can be used on any other FeoBlog server that contains a copy of that item.

Advanced Topics
---------------

I should probably write more about these things? Tell me if you'd find them useful.

* Using Sync to copy your content between servers. (Hopefully the in-client info is enough for now?)
* Running a server behind Apache
* Running a server in Docker
* Writing your own server
* Writing your own client. But in the meantime see these examples:
  * Python:
    * <https://github.com/NfNitLoop/fb-rss>
  * TypeScript for Deno:
    * <https://github.com/NfNitLoop/rss2fb> 
    * <https://github.com/NfNitLoop/deno-feomasto>
