//! The sqlite backend just stores all data (including BLOBs) in a single
//! sqlite3 file. SQLite is great at storing lots of small blobs this way,
//! but may perform poorly for lots of large files.
//! 
//! Mostly, this makes data management trivial since it's all in one file.
//! But if performance is an issue we can implement a different backend.

// TODO: Consider running ANALYZE: https://www.sqlite.org/lang_analyze.html -- results in better default query plans.

mod upgraders;

use std::{io::{Read, Write}, path::Path, collections::HashMap};

use crate::{backend::UsageByUserRow, protos::Item, util::AsHex};
use actix_web::web::Bytes;
use backend::{FileMeta, RowCallback, SHA512};
use log::{debug, warn};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{DatabaseName, NO_PARAMS, OpenFlags, named_params};
use sodiumoxide::randombytes::randombytes;
use crate::backend::{self, UserID, Signature, ItemRow, ItemDisplayRow, Timestamp, ServerUser, QuotaDenyReason};

use anyhow::{Error, bail, Context};
use rusqlite::{params, OptionalExtension, Row};

use super::{FileStream, PruneResult, TimeSpan};

const CURRENT_VERSION: u32 = 7;

type Pool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;
type PConn = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

pub(crate) struct FactoryBuilder {
    sqlite_file: String
}

impl FactoryBuilder {
    pub fn new(sqlite_file: String) -> Self {
        Self {
            sqlite_file
        }
    }
}

impl backend::FactoryBuilder for FactoryBuilder {
    fn factory(&self) -> Result<Box<dyn backend::Factory>, Error> {
        if !self.db_exists()? {
            bail!("\
                    Error: Database file not found.\n\
                    You may need to run `diskuto db init` to create the a database.\
            ");
        }

        if self.db_needs_upgrade()? {
            bail!("\
                Error: Database needs an upgrade.\n\
                Run `diskuto db upgrade` to upgrade it.
            ");
        }

        self.set_wal()?;

        Ok(Box::new(self.build_factory()?))
    }

    fn db_exists(&self) -> Result<bool, Error> {
        let path = Path::new(self.sqlite_file.as_str());
        Ok(path.exists())
    }

    fn db_needs_upgrade(&self) -> Result<bool, Error> {
        let conn = self.connection()?;
        let db_version = conn.get_version()?;
        Ok(db_version < CURRENT_VERSION)
    }

    fn db_upgrade(&self) -> Result<(), Error> {
        if !self.db_exists()? {
            bail!("No such database file: {}", self.sqlite_file)
        }

        let upgraders = upgraders::Upgraders::new();
        let conn = self.connection()?;
        upgraders.upgrade(&conn)?;

        Ok(())
    }

    fn db_create(&self) -> Result<(), Error> {
        if self.db_exists()? {
            bail!("Database already exists")
        }

        println!("Creating database: {}", self.sqlite_file);
        let pool = self.pool_builder().build(
            self.connection_manager()
            // Let sqlite create the DB file since that is explicitly our intention here:
            .with_flags(OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_READ_WRITE)
        )?;

        let conn = Connection{ 
            conn: pool.get()?,
            pool: pool.clone(),
        };
        conn.initialize()?;
        println!("Database created.");


        // This allows me to be lazy, I can specify new DB additions as version upgrades and not have to keep updating the
        // main initialize() code. BUT, I probably should if the upgrade path gets too long.
        drop(conn);
        drop(pool);
        if self.db_needs_upgrade()? {
            self.db_upgrade()?;
        }

        Ok(())
    }
}

impl FactoryBuilder {
    // Shortcut the FactoryBuilder::new() checks and open a connection to a DB that may be in a bad state.
    // e.g.:
    // * needs tables created
    // * needs to be upgraded.
    fn connection(&self) -> Result<Connection, Error> {
        let pool = self.pool()?;
        Ok(
            Connection { 
                conn: pool.get()?,
                pool,
            }
        )
    }

    fn pool(&self) -> Result<r2d2::Pool<SqliteConnectionManager>, r2d2::Error> {
        self.pool_builder().build(self.connection_manager())
    }

    fn pool_builder(&self) -> r2d2::Builder<SqliteConnectionManager> {
        r2d2::Pool::builder()
        .min_idle(Some(0)) // defaults to max_size. (Which defaults to 10.)
    }

    fn build_factory(&self) -> Result<Factory, Error> {
        Ok(Factory{ pool: self.pool()? })
    }

    fn connection_manager(&self) -> r2d2_sqlite::SqliteConnectionManager {
        r2d2_sqlite::SqliteConnectionManager
            ::file(self.sqlite_file.as_str())
            // Note: explicitly NOT SQLITE_OPEN_CREATE
            .with_flags(OpenFlags::SQLITE_OPEN_READ_WRITE)
    }

    /// Enable write-ahead-logging mode for SQLite.
    /// This greatly improves write performance, which helps in general, but in particular
    /// when syncing your feed.
    /// See: https://sqlite.org/wal.html
    fn set_wal(&self) -> Result<(), Error> {
        let conn = self.connection()?;
        let wal_mode = "wal";
        let new_mode: String = conn.conn.pragma_update_and_check(
            None,
            "journal_mode",
            &wal_mode,
            |row| { row.get(0) },
        )?;
        if wal_mode != &new_mode {
            warn!("Could not set journal_mode to WAL mode. Using {}", new_mode);
        } else {
            debug!("WAL mode set.");
        }

        Ok(())
    }
}

pub(crate) struct Factory
{
    pool: Pool,
}

impl backend::Factory for Factory
{
    fn open(&self) -> Result<Box<dyn backend::Backend>, Error>
    {
        let conn = Connection{
            conn: self.pool.get()?,
            pool: self.pool.clone(),
        };
        Ok(Box::new(conn))
    }

    fn dyn_clone(&self) -> Box<dyn backend::Factory> {
        let new_factory = Factory {
            pool: self.pool.clone()
        };
        Box::new(new_factory)
    }
}


pub(crate) struct Connection
{
    // Mostly, we'll use an open connection:
    conn: PConn,

    // But also let's get an Arc copy of the pool in case we need to open more connections.
    pool: Pool,
}


/// private methods for Conneciton
impl Connection
{
    fn initialize(&self) -> Result<(), Error>
    {
        self.run("
            CREATE TABLE version (
                -- The current version of the database schema.
                version INTEGER
            )
        ")?;
        self.run("INSERT INTO version VALUES(3)")?;

        self.run("
            CREATE TABLE item(
                -- An Item is the core data structure of Diskuto.
                -- It is a BLOB of protobuf v3 bytes defining an item in a
                -- user's collection of items
                bytes BLOB

                -- An item must be accompanied by a nacl public key (user_id)
                -- and (detached) signature so that its authenticity can be
                -- verified.
                , user_id BLOB
                , signature BLOB

                -- A copy of the signed timestamp from within `bytes`
                -- this allows for sorting queries by timestamp.
                , unix_utc_ms INTEGER

                -- The date this item was received by this server. May differ
                -- from above.
                , received_utc_ms INTEGER
            )
        ")?;
        self.run("
            CREATE UNIQUE INDEX item_primary_idx
            ON item(user_id, signature)
        ")?;
        self.run("
            CREATE INDEX item_user_chrono_idx
            ON item(user_id, unix_utc_ms)
        ")?;
        self.run("
            CREATE INDEX item_user_chrono_received_idx
            ON item(user_id, received_utc_ms)
        ")?;
        self.run("
            CREATE INDEX item_unix_utc_idx
            ON item(unix_utc_ms)
        ")?;
        self.run("
            CREATE INDEX item_received_utc_idx
            ON item(received_utc_ms)
        ")?;

        self.run("
            CREATE TABLE server_user(
                -- These users have been granted direct access to the server.
                
                user_id BLOB

                -- Information about this user.
                -- Not displayed on the web UI, just here to let the server
                -- admin leave a human-readable note about who this user is.
                , notes TEXT

                -- bool 0/1 -- should this user's posts appear on the home page
                -- of this server?
                , on_homepage INTEGER

                -- How many bytes will the server cache for this user?
                -- 0 = unlimited.
                , max_bytes INTEGER 
            )
        ")?;

        self.run("
            CREATE UNIQUE INDEX server_user_primary_idx
            ON server_user(user_id)
        ")?;

        self.run("
            CREATE INDEX server_user_homepage_idx
            ON server_user(on_homepage, user_id)
        ")?;


        self.run("
            CREATE TABLE follow(
                -- Lists which users follow which other users.
                -- Always represents the latest Profile saved by a user.
                source_user_id BLOB,
                followed_user_id BLOB,
                display_name TEXT
            )
        ")?;

        self.run("
            CREATE UNIQUE INDEX follow_primary_idx
            ON follow(source_user_id, followed_user_id)
        ")?;

        self.run("
            CREATE TABLE profile(
                -- Always contains a reference to the latest profile uploaded by a user
                user_id BLOB,
                signature BLOB,
                display_name TEXT
            )
        ")?;

        self.run("
            CREATE UNIQUE INDEX profile_primary_idx
            ON profile(user_id)
        ")?;

        // See upgraders.rs for newer DB additions.

        Ok(())
    }

    fn run(&self, sql: &str) -> Result<(), Error>
    {
        self.conn.execute(sql, params![])?;
        Ok(())
    }

    fn get_version(&self) -> Result<u32, Error>
    {
        let table_count: u32  = self.conn.prepare(
            "SELECT count()
            FROM sqlite_master
            WHERE type = 'table'
            AND name = 'version'
            "
        )?.query_row(
            params![],
            |row|  Ok(row.get(0)?)
        )?;

        if table_count == 0 {
            bail!("No version table found. This may not be a valid diskuto database.")
        }

        let mut stmt = self.conn.prepare(
            "SELECT version from version"
        )?; 
        let versions = stmt.query_map(
            params![],
            |row| -> rusqlite::Result<u32> { Ok(row.get(0)?) }
        )?;

        let versions: Vec<u32> = versions.take(2).collect::<rusqlite::Result<Vec<u32>>>()?;

        if versions.len() == 0 {
            bail!("Found no version in the database. This may not be a valid diskuto database.");
        }
        if versions.len() > 1 {
            bail!("Found more than one version in the database. This database may have been corrupted.");
        }

        Ok(versions[0])
    }

    fn set_version(&self, version: u32) -> Result<(), Error> {
        self.conn.execute("UPDATE version SET version = ?", params![version])?;

        Ok(())
    }

    fn all_items<'a>(&self, after_uid: &Option<UserID>, after_sig: &Option<Signature>, callback: RowCallback<'a, ItemRow>) -> Result<(), Error>{
        let mut stmt;
        let mut rows;
        if let (Some(uid), Some(sig)) = (after_uid, after_sig) {
            stmt = self.conn.prepare("
                SELECT
                    user_id,
                    signature,
                    unix_utc_ms,
                    received_utc_ms,
                    bytes
                FROM item
                WHERE (user_id > :uid)
                OR (user_id = :uid AND signature > :sig)
                ORDER BY user_id, signature
            ")?;
            rows = stmt.query_named(named_params! {
                "uid": uid.bytes(),
                "sig": sig.bytes(),
            })?;
        } else {
            // Start from the beginning:
            stmt = self.conn.prepare("
                SELECT
                    user_id,
                    signature,
                    unix_utc_ms,
                    received_utc_ms,
                    bytes
                FROM item
                ORDER BY user_id, signature
            ")?;
            rows = stmt.query(params![])?;
        }

        let mut fetch_more = true;
        while fetch_more {
            let row = match rows.next()? {
                None => return Ok(()), // No more results.
                Some(row) => row,
            };

            let ir = ItemRow{
                user: UserID::from_vec(row.get(0)?)?,
                signature: Signature::from_vec(row.get(1)?)?,
                timestamp: Timestamp {  unix_utc_ms: row.get(2)? },
                received: Timestamp {  unix_utc_ms: row.get(3)? },
                item_bytes: row.get(4)?,
            };
            fetch_more = callback(ir)?;
        }

        Ok(())
    }

}

/// We're saving a profile. If it's new, update the profile and follow tables.
fn update_profile(conn: &rusqlite::Savepoint, item_row: &ItemRow, item: &Item) -> Result<(), Error> {

    let prev_timestamp: Option<i64> =  
        conn.prepare("
            SELECT i.unix_utc_ms
            FROM profile AS p
            INNER JOIN item AS i USING (user_id, signature)
            WHERE user_id = ?
        ")?
        .query(params![ item_row.user.bytes() ])?
        .next()?
        .map(|row| row.get(0))
        .transpose()?
    ;

    // Never replace a newer profile's metadata:
    if let Some(previous) = prev_timestamp {
        if previous >= item.timestamp_ms_utc {
            return Ok(())
        }
    }

    // Replace all follows with new ones listed in the profile:
    conn.execute("DELETE FROM follow WHERE source_user_id = ?", params![item_row.user.bytes()])?;

    // Behavior is undefined if duplicate follows exist in a Profile. So we just replace:
    let mut add_follow = conn.prepare("
        INSERT OR REPLACE INTO follow (source_user_id, followed_user_id, display_name)
        VALUES (?, ?, ?)
    ")?;

    for follow in &item.profile().follows {
        add_follow.execute(params![
            item_row.user.bytes(),
            follow.user.bytes,
            follow.display_name,
        ])?;
    }

    let mut add_profile = conn.prepare("
        INSERT OR REPLACE INTO profile(user_id, signature, display_name)
        VALUES (?,?,?)
    ")?;
    add_profile.execute(params![
        item_row.user.bytes(),
        item_row.signature.bytes(),
        item.profile().display_name
    ])?;

    Ok(())
}

fn save_comment_reply(conn: &rusqlite::Connection, row: &ItemRow, item: &Item) -> Result<(), Error> {
    if !item.has_comment() {
        return Ok(())
    }

    let comment = item.comment();
    let reply = ReplyRow {
        from_user_id: row.user.clone(),
        from_signature: row.signature.clone(),
        to_user_id: UserID::from_vec(comment.reply_to.user_id.bytes.clone())?,
        to_signature: Signature::from_vec(comment.reply_to.signature.bytes.clone())?,
    };

    save_reply_rows(conn, &[reply])
}

fn save_reply_rows(conn: &rusqlite::Connection, replies: &[ReplyRow]) -> Result<(), Error> {
    let mut stmt = conn.prepare("
        INSERT INTO reply (from_user_id, from_signature, to_user_id, to_signature)
        VALUES (?,?,?,?)
    ")?;
    for reply in replies {
        stmt.execute(params![
            reply.from_user_id.bytes(),
            reply.from_signature.bytes(),
            reply.to_user_id.bytes(),
            reply.to_signature.bytes(),
        ])?;
    }
    
    Ok(())
}


impl backend::Backend for Connection
{
    fn homepage_items<'a>(
        &self,
        time_span: TimeSpan,
        callback: &'a mut dyn FnMut(ItemDisplayRow) -> Result<bool,Error>
    ) -> Result<(), Error> {

        let mut params = vec![];
        let query = match time_span {
            TimeSpan::Before(before) => {
                params.push(before.unix_utc_ms);
                "
                    SELECT
                        user_id
                        , i.signature
                        , unix_utc_ms
                        , received_utc_ms
                        , bytes
                        , p.display_name
                    FROM item AS i
                    LEFT OUTER JOIN profile AS p USING (user_id)
                    WHERE unix_utc_ms < ?
                    AND user_id IN (
                        SELECT user_id
                        FROM server_user
                        WHERE on_homepage = 1
                    )
                    ORDER BY unix_utc_ms DESC, i.signature DESC
                "
            },
            TimeSpan::After(after) => {
                params.push(after.unix_utc_ms);
                "
                    SELECT
                        user_id
                        , i.signature
                        , unix_utc_ms
                        , received_utc_ms
                        , bytes
                        , p.display_name
                    FROM item AS i
                    LEFT OUTER JOIN profile AS p USING (user_id)
                    WHERE unix_utc_ms > ?
                    AND user_id IN (
                        SELECT user_id
                        FROM server_user
                        WHERE on_homepage = 1
                    )
                    ORDER BY unix_utc_ms ASC, i.signature DESC
                "
            },
        };

        let mut stmt = self.conn.prepare(query)?;
        let mut rows = stmt.query(params)?;


        let to_item_profile_row = |row: &Row<'_>| -> Result<ItemDisplayRow, Error> {

            let item = ItemRow{
                user: UserID::from_vec(row.get(0)?)?,
                signature: Signature::from_vec(row.get(1)?)?,
                timestamp: Timestamp{ unix_utc_ms: row.get(2)? },
                received: Timestamp{ unix_utc_ms: row.get(3)? },
                item_bytes: row.get(4)?,
            };

            Ok(ItemDisplayRow{
                item,
                display_name: row.get(5)?
            })
        };

        while let Some(row) = rows.next()? {
            let item = to_item_profile_row(row)?;
            let result = callback(item)?;
            if !result { break; }
        }

        Ok( () )
    }

    fn user_items<'a>(
        &self,
        user: &UserID,
        time_span: TimeSpan,
        callback: &'a mut dyn FnMut(ItemRow) -> Result<bool,Error>
    ) -> Result<(), Error> {

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];
        let query = match time_span {
            TimeSpan::Before(before) => {
                params.push(Box::new(before.unix_utc_ms));
                params.push(Box::new(user.bytes().to_vec()));
                "
                    SELECT
                        i.user_id
                        , i.signature
                        , unix_utc_ms
                        , received_utc_ms
                        , bytes
                    FROM item AS i
                    WHERE
                        unix_utc_ms < ?
                        AND user_id = ?
                        AND EXISTS(SELECT user_id FROM known_users WHERE user_id = i.user_id)
                    ORDER BY unix_utc_ms DESC, i.signature DESC
                "
            },
            TimeSpan::After(after) => {
                params.push(Box::new(after.unix_utc_ms));
                params.push(Box::new(user.bytes().to_vec()));
                "
                    SELECT
                        i.user_id
                        , i.signature
                        , unix_utc_ms
                        , received_utc_ms
                        , bytes
                    FROM item AS i
                    WHERE
                        unix_utc_ms > ?
                        AND user_id = ?
                        AND EXISTS(SELECT user_id FROM known_users WHERE user_id = i.user_id)
                    ORDER BY unix_utc_ms ASC, i.signature ASC
                "          
            }
        };
        
        let mut stmt = self.conn.prepare(query)?;
        let mut rows = stmt.query(params)?;

        let convert = |row: &Row<'_>| -> Result<ItemRow, Error> {
            let item = ItemRow{
                user: UserID::from_vec(row.get(0)?)?,
                signature: Signature::from_vec(row.get(1)?)?,
                timestamp: Timestamp{ unix_utc_ms: row.get(2)? },
                received: Timestamp{ unix_utc_ms: row.get(3)? },
                item_bytes: row.get(4)?,
            };

            Ok(item)
        };

        while let Some(row) = rows.next()? {
            let item = convert(row)?;
            let result = callback(item)?;
            if !result { break; }
        }

        Ok( () )
    }

    fn reply_items<'a>(
        &self,
        user: &UserID,
        signature: &Signature,
        before: Timestamp,
        callback: RowCallback<'a, ItemRow>,
    ) -> Result<(), Error> {
        let mut stmt = self.conn.prepare("
            SELECT
                i.user_id
                , i.signature
                , unix_utc_ms
                , received_utc_ms
                , bytes
            FROM item AS i
            INNER JOIN reply AS r ON (
                r.from_user_id = i.user_id
                AND r.from_signature = i.signature
            )
            WHERE
                unix_utc_ms < ?
                AND r.to_user_id = ?
                AND r.to_signature = ?
                AND EXISTS(SELECT user_id FROM known_users WHERE user_id = i.user_id)
            ORDER BY unix_utc_ms DESC, i.signature DESC
        ")?;

        let mut rows = stmt.query(params![
            before.unix_utc_ms,
            user.bytes(),
            signature.bytes(),
        ])?;

        let convert = |row: &Row<'_>| -> Result<ItemRow, Error> {
            let item = ItemRow{
                user: UserID::from_vec(row.get(0)?)?,
                signature: Signature::from_vec(row.get(1)?)?,
                timestamp: Timestamp{ unix_utc_ms: row.get(2)? },
                received: Timestamp{ unix_utc_ms: row.get(3)? },
                item_bytes: row.get(4)?,
            };

            Ok(item)
        };

        while let Some(row) = rows.next()? {
            let item = convert(row)?;
            let result = callback(item)?;
            if !result { break; }
        }

        Ok( () )
    }

    fn user_feed_items<'a>(
        &self,
        user_id: &UserID,
        time_span: TimeSpan,
        callback: RowCallback<'a, ItemDisplayRow>,
    ) -> Result<(), Error> {

  


        let timestamp;
        let ts_order;
        let filter_ts;
        match time_span {
            TimeSpan::Before(ts) => {
                timestamp = ts;
                filter_ts = "unix_utc_ms < :timestamp";
                ts_order = "DESC";
            },
            TimeSpan::After(ts) => {
                timestamp = ts;
                filter_ts = "unix_utc_ms > :timestamp";
                ts_order = "ASC";
            }
        };

        // Because we follow N users, and the indexes for (user_id, timestamp) are fast, make N separate queries
        // against those indexes and merge them with a UNION ALL. This forces SQLite to walk & merge them like should
        // scale well, vs... whatever it was trying to do.

        let follows = get_follows(&self, user_id)?;
        let subselects: Vec<String> = follows.keys().map(|uid| {
            format!(
                "
                    SELECT * from iv 
                    WHERE user_id = x'{uid}'
                ",
                uid=uid.bytes().as_hex(),
            )
        }).collect();

        if subselects.is_empty() {
            return Ok(());
        }

        let query = format!(
            "
                WITH iv AS (
                    SELECT 
                        user_id
                        , signature
                        , unix_utc_ms
                        , received_utc_ms
                        , bytes
                    FROM item
                    WHERE {filter_ts}
                )
                {subselects}
                ORDER BY unix_utc_ms {ts_order}, signature {ts_order}
            ", 
            filter_ts=filter_ts,
            ts_order=ts_order,
            subselects=subselects.join("\n\nUNION ALL\n")
        );

        let mut stmt = self.conn.prepare(&query)?;

        let mut rows = stmt.query_named(&[
            (":timestamp", &timestamp.unix_utc_ms),
        ])?;

        let to_item_profile_row = |row: &Row<'_>| -> Result<ItemDisplayRow, Error> {

            let item = ItemRow{
                user: UserID::from_vec(row.get(0)?)?,
                signature: Signature::from_vec(row.get(1)?)?,
                timestamp: Timestamp{ unix_utc_ms: row.get(2)? },
                received: Timestamp{ unix_utc_ms: row.get(3)? },
                item_bytes: row.get(4)?,
            };

            Ok(ItemDisplayRow{
                display_name: follows.get(&item.user).map(|info| info.display_name.clone()).flatten(),
                item,
            })
        };

        while let Some(row) = rows.next()? {
            let item = to_item_profile_row(row)?;
            let result = callback(item)?;
            if !result { break; }
        }

        Ok( () )
    }

    fn server_user(&self, user: &UserID)
    -> Result<Option<backend::ServerUser>, Error> 
    { 
        let mut stmt = self.conn.prepare("
            SELECT notes, on_homepage
            FROM server_user
            WHERE user_id = ?
        ")?;

        let to_server_user = |row: &Row<'_>| {
            let on_homepage: isize = row.get(1)?;
             Ok(
                 ServerUser {
                    user: user.clone(),
                    notes: row.get(0)?,
                    on_homepage: on_homepage != 0,
                }
            )
        };

        let item = stmt.query_row(
            params![user.bytes()],
            to_server_user,
        ).optional()?;

        Ok(item)

    }

    fn server_users<'a>(&self, cb: RowCallback<'a, ServerUser>) -> Result<(), Error> {
        let mut stmt = self.conn.prepare("
            SELECT 
                user_id
                , notes
                , on_homepage
            FROM server_user
            ORDER BY on_homepage, user_id
        ")?;

        let mut rows = stmt.query(NO_PARAMS)?;

        while let Some(row) = rows.next()? {
            let on_homepage: isize = row.get(2)?;
            let on_homepage = on_homepage != 0;

            let user = ServerUser {
                user: UserID::from_vec(row.get(0)?)?,
                notes: row.get(1)?,
                on_homepage,
            };
            let more = cb(user)?;
            if !more {break;}
        }

        Ok(())
    }
    
    
    fn user_item_exists(&self, user: &UserID, signature: &Signature) -> Result<bool, Error> { 
        let mut stmt = self.conn.prepare("
            SELECT COUNT(*)
            FROM item
            WHERE user_id = ?
            AND signature = ?
        ")?;

        let count: u32 = stmt.query_row(
            params![
                user.bytes(),
                signature.bytes(),
            ],
            |row| { Ok(row.get(0)?) }
        )?;

        if count > 1 {
            bail!("Found {} matches!? (user_id,signature) should be unique!", count);
        }

        Ok(count > 0)
    }

    fn user_item(&self, user: &UserID, signature: &Signature) -> Result<Option<ItemRow>, Error> { 
        let mut stmt = self.conn.prepare("
            SELECT
                user_id
                , signature
                , unix_utc_ms
                , received_utc_ms
                , bytes
            FROM item AS i
            WHERE user_id = ?
            AND signature = ?
            AND EXISTS(SELECT user_id FROM known_users WHERE user_id = i.user_id)
        ")?;

        let mut rows = stmt.query(params![
            user.bytes(),
            signature.bytes(),
        ])?;

        let row = match rows.next()? {
            None => return Ok(None),
            Some(row) => row,
        };

        let item = ItemRow{
            user: UserID::from_vec(row.get(0)?)?,
            signature: Signature::from_vec(row.get(1)?)?,
            timestamp: Timestamp{ unix_utc_ms: row.get(2)? },
            received: Timestamp{ unix_utc_ms: row.get(3)? },
            item_bytes: row.get(4)?,
        };

        if rows.next()?.is_some() {
            bail!("Found multiple matching rows!? (user_id,signature) should be unique!");
        }

        Ok(Some(item))
    }

    fn save_user_item(&mut self, row: &ItemRow, item: &Item) -> Result<(), Error>
    {
        let tx = self.conn.savepoint().context("getting a transaction")?;

        let stmt = "
            INSERT INTO item (
                user_id
                , signature
                , unix_utc_ms
                , received_utc_ms
                , bytes
            ) VALUES (?, ?, ?, ?, ?);
       ";

        tx.execute(stmt, params![
            row.user.bytes(),
            row.signature.bytes(),
            row.timestamp.unix_utc_ms,
            row.received.unix_utc_ms,
            row.item_bytes.as_slice(),
        ])?;

        if item.has_profile() {
            update_profile(&tx, row, item)?;
        }

        if item.has_comment() {
            save_comment_reply(&tx, row, item)?;
        }

        index_attachments(&tx, row, item)?;

        tx.commit().context("committing")?;
        Ok(())
    }

    fn add_server_user(&self, server_user: &ServerUser) -> Result<(), Error> {

        let stmt = "
            INSERT INTO server_user(user_id, notes, on_homepage)
            VALUES (?,?,?)
        ";

        let on_homepage = if server_user.on_homepage { 1 } else { 0 };

        self.conn.execute(stmt, params![
            server_user.user.bytes(),
            server_user.notes.as_str(),
            on_homepage
        ])?;

        Ok(())
    }

    fn user_profile(&self, user: &UserID) -> Result<Option<ItemRow>, Error> {

        // TODO: I'm not crazy about making 2 queries here instead of a join, but it lets me
        // re-use the user_item() loading logic.
        let mut find_profile = self.conn.prepare("
            SELECT user_id, signature
            FROM profile
            WHERE user_id = ?
        ")?;

        let mut rows = find_profile.query(params![user.bytes()])?;
        let row = match rows.next()? {
            None => return Ok(None),
            Some(row) => row,
        };

        let user_id: Vec<u8> = row.get(0)?;
        let signature: Vec<u8> = row.get(1)?;

        let user_id = UserID::from_vec(user_id)?;
        let signature = Signature::from_vec(signature)?;

        self.user_item(&user_id, &signature)
    }

    fn user_known(&self, user_id: &UserID) -> Result<bool, Error> {
        let mut query = self.conn.prepare("
            SELECT
                EXISTS(SELECT user_id FROM server_user WHERE user_id = :user_id)
                OR EXISTS(
                    SELECT followed_user_id
                    FROM follow AS f
                    INNER JOIN server_user AS su ON (f.source_user_id = su.user_id)
                    WHERE followed_user_id = :user_id
                )
        ")?;

        let mut result = query.query_named(&[
            (":user_id", &user_id.bytes())
        ])?;

        let row = match result.next()? {
            Some(row) => row,
            None => bail!("Expected at least 1 row from SQLite."),
        };

        Ok(row.get(0)?)
    }

    fn quota_check_item(&self, user_id: &UserID, _bytes: &[u8], _item: &Item) -> Result<Option<QuotaDenyReason>, Error> {
        
        if self.server_user(user_id)?.is_some() {
            // TODO: Implement optional quotas for "server users".
            // For now, there is no quota for them:
            return Ok(None);
        };

        // Check those followed by "server users":
        let mut statement = self.conn.prepare("
            SELECT
                f.followed_user_id
            FROM
                follow AS f
                INNER JOIN server_user AS su ON su.user_id = f.source_user_id
            WHERE
                f.followed_user_id = ?
        ")?;
        let mut rows = statement.query(params![user_id.bytes()])?;
        if rows.next()?.is_some() {
            // TODO Implement quotas in follows. For now, presence of a follow gives unlimited quota.
            // TODO: Exclude server users whose profiles/IDs have been revoked.
            return Ok(None);
        }

        // TODO: When "pinning" is implemented, allow posting items which are pinned by server users and their follows.
        // TODO: I've since decided that "pinning" might be prone to abuse. I should write up my thoughts there.

        Ok(Some(QuotaDenyReason::UnknownUser))
    }
   
    fn get_contents(&self, user_id: UserID, signature: Signature, file_name: &str) 
    -> Result< Option<FileStream> , Error> 
    {
        let mut stmt = self.conn.prepare("
            SELECT store.rowid, length(store.contents), a.size
            FROM store 
            INNER JOIN item_attachment AS a USING(hash)
            WHERE 
                a.user_id = ?
                AND a.signature = ?
                AND a.name = ?
                AND EXISTS(SELECT user_id FROM known_users WHERE user_id = a.user_id)
        ")?;

        let mut rows = stmt.query(params![
            user_id.bytes(),
            signature.bytes(),
            file_name,
        ])?;

        let row = match rows.next()? {
            None => return Ok(None),
            Some(row) => row,
        };

        let rowid: i64 = row.get(0)?;
        let size = row.get::<_, i64>(1)? as u64;
        let expected_size = row.get::<_, i64>(2)? as u64;

        if size != expected_size {
            bail!("Item expected {} bytes but found {}", expected_size, size);
        }

        if rows.next()?.is_some() {
            bail!("UNIQUE constraint failure, found 2 results for file");
        }

        drop(rows);
        drop(stmt);


        // Open a new pooled connection that will be owned just by our Iterator/Stream:
        // TODO: Maybe we should just re-open the connection every time if we have to for the BLOB too?
        let conn = self.pool.get()?;
        let mut buf = [0 as u8; 32 * 1024];
        let mut read_pos = 0;

        let iter = std::iter::from_fn(move || -> Option<Result<Bytes,crate::server::SendError>> {
            // Have to re-open the BLOB every time because it's not Send (due to its lifetime on &Connection?).
            let blob = conn.blob_open(
                DatabaseName::Main, 
                "store",
                "contents",
                rowid,
                true // read-only
            );

            let blob = match blob {
                Ok(b) => b,
                Err(err) => return Some(Err(err.into())),
            };
    
            let bytes_read = match blob.read_at(&mut buf, read_pos) {
                Err(io_err) => return Some(Err(io_err.into())),
                Ok(x) => x,
            };
            read_pos += bytes_read;

            if bytes_read == 0 {
                return None;
            }

            let bytes = Bytes::copy_from_slice(&buf[..bytes_read]);
            return Some(Ok(bytes));
        });

        let stream = blocking::Unblock::with_capacity(2, iter);
        let stream = Box::new(stream);
        Ok(Some(FileStream{stream, size}))
    }

    fn get_attachment_meta(&self, user_id: &UserID, signature: &Signature, file_name: &str) -> Result<Option<backend::FileMeta>, Error> {
        
        let mut stmt = self.conn.prepare("
            SELECT 
                a.size,
                a.hash,
                s.hash IS NOT NULL AS contents_exist
            FROM item_attachment AS a
            LEFT OUTER JOIN store AS s USING (hash)
            WHERE 
                a.user_id = ?
                AND a.signature = ?
                AND a.name = ?
                AND EXISTS(SELECT user_id FROM known_users WHERE user_id = a.user_id)
        ")?;

        let mut rows = stmt.query(params![
            user_id.bytes(),
            signature.bytes(),
            file_name
        ])?;

        let row = match rows.next()? {
            Some(row) => row,
            None => return Ok(None),
        };

        let size = row.get::<_, i64>(0)? as u64;
        let hash_bytes: Vec<u8> = row.get(1)?;
        let hash = SHA512::from_hash_bytes(&hash_bytes)?;
        let exists = row.get(2)?;

        let meta = FileMeta{
            exists,
            hash,
            size,
            quota_exceeded: false, // TODO
        };

        Ok(Some(meta))
    }

    fn save_attachment(&self, size: u64, hash: &SHA512, file: &mut dyn Read) -> Result<(), Error> {
        // Save to a temporary hash while we stream the data into the database.
        // Note, this is 31 bytes, which is easily distinguishable from SHA-512's 64-bytes:
        let temp_hash = randombytes(31);

        // In practice, SQLite's max BLOB size defaults to <1GiB. 
        // See: https://sqlite.org/limits.html
        // We'll just rely on this insert failing to tell us what it is:
        debug!("Inserting zeroblob into 'store'");
        self.conn.execute(
            "INSERT INTO store (hash, contents) VALUES(?, zeroblob(?))",
            params![
                &temp_hash,
                size as i64
            ],
        )?;

        let row_id: i64 = self.conn.query_row(
            "SELECT rowid FROM store WHERE hash = ?",
            params![ &temp_hash ], 
            |row| row.get(0)
        )?;

        let mut blob = self.conn.blob_open(
            DatabaseName::Main,
            "store",
            "contents",
            row_id,
            false // read_only=false
        )?; 

        debug!("Copying temp file into sqlite");
        std::io::copy(file, &mut blob)?;
        blob.flush()?;
        debug!("Finished copy.");

        // Check blob hash:
        // I know the docs say we expect the caller to have performed the hash, but 
        // getting the wrong content here is annoying so I'm going to do it again anyway:
        let hash_check = SHA512::from_file(&mut blob)?;
        debug!("Verified BLOB hash: {}", hash);
        
        if &hash_check != hash {
            bail!("SQLite expected {} but got {}", hash, hash_check);
        }

        drop(blob);

        // Now that the copy has finished, move the blob into its final location atomically:
        let updated = self.conn.execute(
            "UPDATE store SET hash = ? WHERE hash = ?",
            params![hash.bytes(), &temp_hash],
        )?;

        if updated != 1 {
            bail!("Error updating content hash from {:?} to {}", temp_hash, hash);
        }
        debug!("save_attachment() done.");

        Ok(())
    }

    fn prune(&self, opts: backend::PruneOpts) -> Result<backend::PruneResult, Error> {
        
        let mut result = PruneResult{
            dry_run: opts.dry_run,
            attachments_bytes: 0,
            attachments_count: 0,
            items_bytes: 0,
            items_count: 0,
        };

        if opts.items {
            let query = "
                SELECT 
                    COUNT(*) AS `count`
                    , COALESCE(SUM(LENGTH(bytes) + LENGTH(user_id) + LENGTH(signature)), 0) AS size
                FROM item AS i
                WHERE NOT EXISTS (
                    SELECT 1
                    FROM known_users
                    WHERE user_id = i.user_id
                )
            ";

            let (count, bytes) = self.conn.query_row(
                query,
                params![],
                |row| Ok((row.get::<usize,i64>(0)? as u64, row.get::<usize,i64>(1)? as u64))
            )?;
            result.items_count = count;
            result.items_bytes = bytes;
        }

        if opts.attachments {
            let query = if opts.items {
                "
                SELECT COUNT(*) AS `count`, COALESCE(SUM(LENGTH(contents)), 0) AS size
                FROM store AS s
                WHERE NOT EXISTS (
                    SELECT 1
                    FROM item_attachment
                    INNER JOIN item USING (user_id, signature)
                    WHERE hash = s.hash
                )
                "
            } else {
                // We'll delete more if we're also deleting items:
                "
                SELECT COUNT(*) AS `count`, COALESCE(SUM(LENGTH(contents)), 0) AS size
                FROM store AS s
                WHERE NOT EXISTS (
                    SELECT 1
                    FROM item_attachment
                    INNER JOIN item USING (user_id, signature)
                    INNER JOIN known_users USING (user_id)
                    WHERE hash = s.hash
                )
                "
            };


            // Find attachments that are no longer referenced.
            // NOTE: Can't just do a simple LEFT OUTER JOIN and check for NULL.
            // That could lead to false positives when one ref is dangling but another exists.
            let (count, bytes) = self.conn.query_row(
                query,
                params![],
                |row| Ok((row.get::<usize, i64>(0)? as u64, row.get::<usize,i64>(1)? as u64)),
            )?;
            result.attachments_count = count;
            result.attachments_bytes = bytes;
        }

        if opts.dry_run {
            return Ok(result)
        }

        // Note: Delete items first, which makes more things available to delete from store if we do that:
        if opts.items {
            let query = "
                DELETE FROM ITEM AS i
                WHERE NOT EXISTS(
                    SELECT 1
                    FROM known_users
                    WHERE user_id = i.user_id
                )
            ";
            self.conn.execute(query, params![])?;

            // Delete attachments now abandoned:
            let query = "
                DELETE FROM item_attachment AS ia
                WHERE NOT EXISTS (
                    SELECT 1
                    FROM item
                    WHERE user_id = ia.user_id
                    AND signature = ia.signature
                )
            ";
            self.conn.execute(query, params![])?;
        }

        if opts.attachments {
            // Note: We can get by w/ one query here because we already deleted
            // items from unknown users if that was desired.
            let query = "
            DELETE FROM store AS s
            WHERE NOT EXISTS (
                SELECT 1
                FROM item_attachment
                INNER JOIN item USING (user_id, signature)
                WHERE hash = s.hash
            )
            ";
            self.conn.execute(query, params![])?;
        }

        self.conn.execute("VACUUM", params![])?;

        Ok(result)
    }

    fn usage_by_user(&self, callback: RowCallback<'_, backend::UsageByUserRow>) -> Result<(), Error> {
        let query = "
            SELECT
                s1.user_id,
                s1.display_name,
                EXISTS(SELECT 1 FROM known_users WHERE user_id=s1.user_id) AS known_user,
                EXISTS(SELECT 1 FROM server_user WHERE user_id=s1.user_id) AS server_user,
                item_count,
                item_size,
                IFNULL(attachment_size,0) AS attachment_size,
                IFNULL(attachment_count,0) AS attachment_count,
                item_size + COALESCE(attachment_size, 0) as total_size
            FROM (
                SELECT
                    user_id,
                    p.display_name,
                    COUNT(*) as item_count,
                    SUM(length(bytes)) as item_size
                FROM item AS i
                LEFT OUTER JOIN profile AS p USING (user_id)
                GROUP BY user_id
            ) AS s1
            LEFT OUTER JOIN (
                SELECT 
                    user_id,
                    SUM(IFNULL(LENGTH(contents),0)) as attachment_size,
                    COUNT(*) AS attachment_count
                FROM (
                    SELECT DISTINCT user_id, hash
                    FROM item
                    INNER JOIN item_attachment USING (user_id, signature)
                ) AS user_hashes
                INNER JOIN store USING (hash)
                GROUP BY user_id
            ) AS s2 USING (user_id)
            ORDER BY total_size DESC
        ";

        let mut stmt = self.conn.prepare(&query)?;
        let mut rows = stmt.query(params![])?;

        let mut fetch_more = true;
        while fetch_more {
            let row = match rows.next()? {
                None => return Ok(()), // No more results.
                Some(row) => row,
            };
            
            let usage = UsageByUserRow {
                user_id: UserID::from_vec(row.get(0)?)?,
                display_name: row.get("display_name")?,
                items_count: row.get::<&str, i64>("item_count")? as u64,
                items_bytes: row.get::<&str, i64>("item_size")? as u64,
                attachments_count: row.get::<&str, i64>("attachment_count")? as u64,
                attachments_bytes: row.get::<&str, i64>("attachment_size")? as u64,
                known_user: row.get("known_user")?,
                server_user: row.get("server_user")?,
                total_bytes: row.get::<&str, i64>("total_size")? as u64,
            };

            fetch_more = callback(usage)?;
        }

        Ok(())
    }
}

struct ReplyRow {
    from_user_id: UserID,
    from_signature: Signature,
    to_user_id: UserID,
    to_signature: Signature,
}

/// A row from the item_attachment table.
struct AttachmentRow {
    user_id: UserID,
    signature: Signature,
    name: String,

    // The size of the attachment (in bytes)
    // Unfortunately must be i64 because SQLite doesn't support u64.
    size: i64,

    hash: SHA512,
}

fn index_attachments(conn: &rusqlite::Connection, row: &ItemRow, item: &Item) -> Result<(), Error> {
    save_attachment_rows(conn, get_attachment_rows(row, item)?)
}

fn get_attachment_rows(row: &ItemRow, item: &Item) -> Result<Vec<AttachmentRow>, Error> {
    let mut rows = vec![];

    // TODO: Eventually support attachments for Profiles (and other types?) too:
    let post = item.post();

    let attachments = &post.attachments.file;
    for attachment in attachments {
        let row = AttachmentRow {
            name: attachment.name.clone(),
            hash: SHA512::from_hash_bytes(attachment.hash.as_slice())?,
            user_id: row.user.clone(),
            signature: row.signature.clone(),
            size: attachment.size as i64,
        };
        if row.name.contains("/") || row.name.contains("\\") {
            bail!("File separators are not allowed in attached file names: {}", row.name);
        }
        if row.size < 0 {
            bail!("File sizes greater than {} bytes are unsupported", i64::MAX);
        }

        rows.push(row);
    }
    return Ok(rows);
}

fn save_attachment_rows(conn: &rusqlite::Connection, rows: Vec<AttachmentRow>) -> Result<(), Error> {
    if rows.is_empty() {
        return Ok(());
    }

    let mut stmt = conn.prepare("
        INSERT OR REPLACE INTO item_attachment(user_id, signature, name, hash, size)
        VALUES (?,?,?,?,?)
    ")?;

    for row in rows {
        stmt.execute(params![
            row.user_id.bytes(),
            row.signature.bytes(),
            row.name,
            row.hash.bytes(),
            row.size as i64,
        ])?;
    }

    Ok(())
}

/// Get all users that `user_id` follows (and themselves).
//
// note: gets ALL follows, could be abused/DoS. (TODO: Protect against unreasonable amount of follows?)
fn get_follows(conn: &Connection, user_id: &UserID) -> Result<HashMap<UserID, FollowInfo>, Error> {
    let mut map = HashMap::new();

    let mut stmt = conn.conn.prepare("
        SELECT
            f.followed_user_id AS user_id
            , f.display_name AS follow_display_name
            , p.display_name
        FROM follow AS f
        LEFT OUTER JOIN profile AS p ON (f.followed_user_id = p.user_id)
        WHERE f.source_user_id = :user_id

        UNION ALL
        SELECT 
            p.user_id
            , p.display_name AS follow_display_name
            , p.display_name 
        FROM profile AS p
        WHERE p.user_id = :user_id
    ")?;

    let mut rows = stmt.query_named(&[
        (":user_id", &user_id.bytes()),
    ])?;

    fn to_info(row: &Row<'_>) -> Result<FollowInfo, Error> {
        let display_name: Option<String> = row.get("display_name")?;
        let follow_display_name: Option<String> = row.get("follow_display_name")?;
        fn not_empty(it: &String) -> bool { !it.trim().is_empty() }

        Ok(FollowInfo {
            user_id: UserID::from_vec(row.get("user_id")?)?,

            // Prefer displaying the name that this user has assigned to the follow.
            // TODO: This seems maybe business-logic-y? Should we move it out of Backend?
            display_name: follow_display_name.filter(not_empty).or(display_name).filter(not_empty),
        })
    }

    while let Some(row) = rows.next()? {
        let info = to_info(row)?;
        map.insert(info.user_id.clone(), info);
    }

    Ok(map)
}

struct FollowInfo {
    user_id: UserID,
    // The display name specified by this user, or (fallback) the user they followed.
    display_name: Option<String>
}