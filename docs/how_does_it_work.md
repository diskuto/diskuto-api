How Does It Work?
=================

Here's how Diskuto accomplishes its core principles (listed in [README.md])

#1 Your data should not be held hostage by a single service.
-----------------------------------------------------------------

Diskuto allows your data to be stored on multiple servers simultaneously.
There are a few features that enable this:

1. The core data structure of Diskuto is an `Item`, which is defined in the
   [protobuf] file, [diskuto.proto]. Your blog is just a collection of `Item`s.
2. Your user ID is a public key used to sign all of your `Item`s. (For more
   info, see: [Crypto].)
3. The signature for a post (`Item`) serves as a unique ID for that item.
4. The server makes all of your `Item`s available via a REST endpoints
   (described in [URL Layout].)

As a result, it is trivial to walk your publicly available data on one server
and copy it to another.

Because your user ID is a randomly generated cryptographic key, it is globally
unique, not just unique to the server that you started out on. You can reuse the
same ID on multiple servers.

#2 Your data should be resilient to censorship and server outages.  
------------------------------------------------------------------

(See #1). Since your data can be served on multiple servers, if any one of them
decides to censor your content, or goes offline, your data can still be accessed
on other servers.

You can also run a local server on your computer to act as a backup. Even if all
online servers are taken offline, you still have your data and can sync it to a
new server.


#3 Your data should not be modifiable by third parties.  
-------------------------------------------------------

(See #1) Since your data is cryptographically signed, readers can verify that it
has not been modified since you signed it.

Clients can download the raw protobuf file and verify it
against its userID and signature before displaying it to the user.


#4 You should be able to create/use clients to view your data as you wish.
--------------------------------------------------------------------------

(See #1) The same REST APIs that allow you to copy your data elsewhere are
usable by anyone to create clients that can download, cache, and present data
however they best see fit. (They can also generate signed `Item` protobufs to
send to the server and post on your behalf.)

There's an example client written in Python available as an example here:
<https://github.com/NfNitLoop/fb-rss>

In fact, the built-in web client (available at `/client/`) uses that same REST
API to view and post data. Your own client can do everything the web site can.


#5 [Servers] should be able to block content
---------------------------------------------------------------------------------

One problem with blockchain-based approaches (like [Scuttlebutt]) is that if any
piece of the blockchain is missing, the data structure is broken. (Or, at least,
no longer verifiable).

Since Diskuto is organized as a collection of signed `Item`s, any one item can
be omitted from the collection by a particular server.

This could be due to legal reasons (ex: a DMCA takedown request) or just due to
the fact that the server administrators find the content objectionable. This
could also be due to a user exceeding their quota on a particular server.


[protobuf]: https://en.wikipedia.org/wiki/Protocol_Buffers
[README.md]: ../README.md
[diskuto.proto]: ../protobufs/diskuto.proto
[Crypto]: ./crypto.md
[URL Layout]: ./url_layout.md
[Scuttlebutt]: https://www.scuttlebutt.nz/



