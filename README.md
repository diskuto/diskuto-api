Diskuto API
===========

[Diskuto] is a [P2P] distributed social networking system designed to protect
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

[Diskuto]: https://github.com/diskuto
[P2P]: https://en.wikipedia.org/wiki/Peer-to-peer
[How Does It Work?]: ./docs/how_does_it_work.md


### Benefits ###

 * UserIDs are globally unique, and can be used among many servers.
 * Multiple servers act as redundant backups of users' content.
 * You can run a server locally to download your own posts, and those of people you follow.
   * Good for backups
   * Allows you to read/post while offline.
 


Getting Started
===============

* <https://blog.nfnitloop.com/> is my personal instance of Diskuto. You can browse that to see what it looks like.
* [Full Stack] - contains configuration files and instructions for testing out Diskuto locally.
* If you want to build the Diskuto API from source, or modify it, see the [Development] documentation.
* See [docs/] for even more information.

[Full Stack]: ./examples/full-stack/
[Development]: docs/development.md
[docs/]: ./docs/
