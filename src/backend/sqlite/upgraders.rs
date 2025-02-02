//! Types that know how to upgrade the SQLite database.

use anyhow::{Error, bail};
use protobuf::Message;
use rusqlite::params;

use crate::{backend::{ItemRow, RowCallback, Signature, UserID}, protos::Item};

use super::{AttachmentRow, CURRENT_VERSION, Connection, ReplyRow, get_attachment_rows, save_attachment_rows, save_reply_rows};

pub(crate) struct Upgraders {
    upgraders: Vec<Box<dyn Upgrader>>
}

impl Upgraders {
    pub fn new() -> Self {
        Self { upgraders: vec![
            Box::new(From3To4),
            Box::new(From4To5),
            Box::new(From5To6),
            Box::new(From6To7),
        ]}
    }

    pub fn upgrade(&self, conn: &Connection) -> Result<(), Error> {
        let mut current_version = conn.get_version()?; 
        while current_version < CURRENT_VERSION {
            let upgrader = self.upgrader_from(current_version)?;
            println!("Upgrading from db version {} to {} ...", current_version, upgrader.to_version());
            upgrader.upgrade(conn)?;
            current_version = conn.get_version()?;
            if current_version != upgrader.to_version() {
                bail!("Upgrader failed to upgrade to advertised version: {}, still {}", upgrader.to_version(), current_version);
            }
        }

        println!("Upgrade complete. Current database version: {}", current_version);

        Ok(())
    }

    fn upgrader_from(&self, version: u32) -> Result<&dyn Upgrader, Error> {
        for upgrader in &self.upgraders {
            if upgrader.from_version() == version {
                return Ok(upgrader.as_ref())
            }
        }
        bail!("Couldn't find a way to upgrade from version {}", version);
    }
}


trait Upgrader {
    fn from_version(&self) -> u32;
    fn to_version(&self) -> u32;
    fn upgrade(&self, conn: &Connection) -> Result<(), Error>;
}


/// New `reply` table to track comments.
/// (and eventually also "reply posts" and other things that may be able to reply)
struct From3To4;
impl Upgrader for From3To4 {
    fn from_version(&self) -> u32 { 3 }
    fn to_version(&self) -> u32 { 4 }

    fn upgrade(&self, conn: &Connection) -> Result<(), Error> {
        conn.run("
            CREATE TABLE reply (
                -- Tracks Items that 'reply to' other items.

                from_user_id BLOB,
                from_signature BLOB,

                to_user_id BLOB,
                to_signature BLOB
            )
        ")?;

        conn.run("
            CREATE INDEX reply_to_idx
            ON reply(to_user_id, to_signature)
        ")?;

        // TODO: Newer rusqlite supports u64 & usize:
        let item_count: u32 = conn.conn.query_row(
            "SELECT COUNT(*) FROM item",
            params![],
            |row| Ok(row.get(0)?)
        )?;

        if item_count > 1000 {
            println!("Scanning {} items for replies. This may take a some time.", item_count);
        }

        let mut pager = ItemPager::new();
        
        // If SQLite hasn't enabled WAL (not the default, & not supported
        // everywhere) then we can't read & write from the DB at the same time.
        // Batch up rows to write here, and periodically write them:
        let mut reply_tos = Vec::<ReplyRow>::new();
        let max_replies = 1000;
        
        while !pager.done {
            pager.iterate(conn, &mut |row| {
                let mut item = Item::new();
                item.merge_from_bytes(row.item_bytes.as_slice())?;

                if !item.has_comment() {
                    // Currently, only comments have replies.
                    // If/when we add new reply types in the future, we'll need
                    // to run a similar migration for those in a separate
                    // migration.  In the meantime:
                    return Ok(true) // continue
                }

                let comment = item.comment();
                
                let reply = ReplyRow {
                    from_user_id: row.user.clone(),
                    from_signature: row.signature.clone(),
                    to_user_id: UserID::from_vec(comment.reply_to.user_id.bytes.clone())?,
                    to_signature: Signature::from_vec(comment.reply_to.signature.bytes.clone())?,
                };
                reply_tos.push(reply);

                Ok(reply_tos.len() < max_replies)
            })?;

            // Write cached reply_tos to the database:
            save_reply_rows(&*conn.conn, reply_tos.as_slice())?;
            reply_tos.clear();
        }

        conn.set_version(self.to_version())?;
        Ok(())
    }
}

struct ItemPager {
    after_uid: Option<UserID>,
    after_sig: Option<Signature>,
    done: bool,
}


impl ItemPager {
    fn new() -> Self {
        Self {
            after_uid: None,
            after_sig: None,
            done: false,
        }
    }

    /// Iterate through the Connection.all_items() results, and store the iteration point to resume.
    fn iterate<'a>(&mut self, conn: &Connection, cb: RowCallback<'a, ItemRow>) -> Result<(), Error> {
        let mut new_uid = None;
        let mut new_sig = None;

        // If we get zero results, default to "done":
        let mut done = true;

        let mut my_cb = |row: ItemRow| {
            new_uid = Some(row.user.clone());
            new_sig = Some(row.signature.clone());

            let result = cb(row);
            if let Ok(should_continue) = result {
                // If we tell the function we want more, but it never calls it again, then we're done iterating:
                done = should_continue;
            }
            return result;
        };


        conn.all_items(&self.after_uid, &self.after_sig, &mut my_cb)?;

        self.after_uid = new_uid;
        self.after_sig = new_sig;
        self.done = done;

        Ok(())
    }
}

// Adds an index which makes finding whether someone is a "known user" much more efficient.
struct From4To5;
impl Upgrader for From4To5 {
    fn from_version(&self) -> u32 { 4 }
    fn to_version(&self) -> u32 { 5 }
    fn upgrade(&self, conn: &Connection) -> Result<(), Error> {

        // Note: Could have made an index on (followed_user_id, source_user_id), but:
        // * Docs say the benefit of a covering index is minimal.
        // * It's redundant w/ the UNIQUE index in the other direction.
        // * Two columns would make for a bigger index.
        conn.run("
            CREATE INDEX follow_followed_idx
            ON follow(followed_user_id)
        ")?;

        // Note: Must use "UNION ALL" here to allow for subquery flattening.
        // Unfortunately that means if we query it directly we get dupes. Oh well.
        // See: https://www.sqlite.org/optoverview.html#flattening
        conn.run("
            CREATE VIEW known_users (user_id) AS
            -- For internal use only. All 'known users' of the server.
                SELECT user_id
                FROM server_user
            UNION ALL
                SELECT followed_user_id
                FROM follow AS f
                INNER JOIN server_user AS s
                    ON (f.source_user_id=s.user_id)
            ;
        ")?;
        
        conn.set_version(self.to_version())?;
        Ok(())
    }
}

// Adds tables and indexes for file attachment.
struct From5To6;
impl Upgrader for From5To6 {
    fn from_version(&self) -> u32 { 5 }
    fn to_version(&self) -> u32 { 6 }
    fn upgrade(&self, conn: &Connection) -> Result<(), Error> {
        conn.run("
            CREATE TABLE item_attachment(
                -- maps items to their attached files.
                
                user_id BLOB,
                signature BLOB,

                -- The name of the file
                name TEXT,

                -- The size of the attachment (in bytes)
                -- (allows us to calculate quotas even if files haven't been attached yet)
                size INTEGER,

                -- the 64-byte sha-512 hash of the file (as bytes).
                -- used to look up the file contents in the 'store' table.
                hash BLOB
            )
        ")?;

        conn.run("
            CREATE INDEX item_attachment_item_idx
            ON item_attachment(user_id, signature, name)
        ")?;
        conn.run("
            CREATE INDEX item_attachment_hash_idx
            ON item_attachment(hash)
        ")?;

        conn.run("
            CREATE TABLE store(
                -- a content-addressable blob store.

                -- The 64-byte sha-512 hash of the blob's contents.
                hash BLOB,

                contents BLOB
            )
        ")?;

        conn.run("
            CREATE INDEX store_hash_idx
            ON store(hash)
        ")?;

        // Index any attachments that may have been uploaded before our upgrade:
        // TODO: Newer rusqlite supports u64 & usize:
        let item_count: u32 = conn.conn.query_row(
            "SELECT COUNT(*) FROM item",
            params![],
            |row| Ok(row.get(0)?)
        )?;

        if item_count > 1000 {
            println!("Scanning {} items for file attachments. This may take a some time.", item_count);
        }

        let mut pager = ItemPager::new();
        
        // If SQLite hasn't enabled WAL (not the default, & not supported
        // everywhere) then we can't read & write from the DB at the same time.
        // Batch up rows to write here, and periodically write them:
        let mut rows_to_insert = Vec::<AttachmentRow>::new();
        let max_rows = 1000;
        
        while !pager.done {
            pager.iterate(conn, &mut |row| {
                let mut item = Item::new();
                item.merge_from_bytes(row.item_bytes.as_slice())?;

                match get_attachment_rows(&row, &item) {
                    Err(err) => {
                        println!(
                            "Not indexing file attachments for /u/{}/i/{} due to error: {}",
                            row.user.to_base58(),
                            row.signature.to_base58(),
                            err
                        );
                    },
                    Ok(rows) => rows_to_insert.extend(rows),
                };

                Ok(rows_to_insert.len() < max_rows)
            })?;

            // Write cached reply_tos to the database:
            save_attachment_rows(&conn.conn, rows_to_insert)?;
            rows_to_insert = vec![];
                
        }

        conn.set_version(self.to_version())?;
        Ok(())
    }
}

// Changes some indexes to UNIQUE that should have been.
struct From6To7;
impl Upgrader for From6To7 {
    fn from_version(&self) -> u32 { 6 }
    fn to_version(&self) -> u32 { 7 }
    fn upgrade(&self, conn: &Connection) -> Result<(), Error> {
        conn.run("DROP INDEX IF EXISTS item_attachment_item_idx")?;
        conn.run("
            CREATE UNIQUE INDEX item_attachment_item_idx
            ON item_attachment(user_id, signature, name)
        ")?;

        conn.run("DROP INDEX IF EXISTS store_hash_idx")?;
        conn.run("
            CREATE UNIQUE INDEX store_hash_idx
            ON store(hash)
        ")?;
    
        conn.set_version(self.to_version())?;
        Ok(())
    }
}