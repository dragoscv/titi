//! SQLite + FTS5 backend for native hosts (Android, Wear, Google TV, iOS,
//! desktop). One file per identity in the app sandbox; plain SQLite (the OS
//! encrypts the sandbox), PQ sealing comes with CH-A6.
//!
//! Search: contentless-delete FTS5 (the messages table is the source of
//! truth), `unicode61 remove_diacritics 2` so "stiinta" finds "știință",
//! `detail=column` + `prefix='2 3'` for small, search-as-you-type indexes, and
//! secure-delete so a tombstoned message leaves no words behind.

use prost::Message;
use rusqlite::{params, Connection, OptionalExtension};

use super::{id16, sort_ms, tokens, ChatBackend, MsgId};
use crate::engine::GroupId;
use crate::proto;
use crate::time::Ms;

const SCHEMA_VERSION: i32 = 1;

pub struct SqliteBackend {
    db: Connection,
}

impl SqliteBackend {
    /// Opens (or creates) the store; `path` = ":memory:" for tests.
    pub fn open(path: &str) -> rusqlite::Result<Self> {
        let db = if path == ":memory:" {
            Connection::open_in_memory()?
        } else {
            Connection::open(path)?
        };
        db.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA secure_delete=ON;
             PRAGMA foreign_keys=OFF;",
        )?;
        let v: i32 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if v < 1 {
            db.execute_batch(
                "BEGIN;
                 CREATE TABLE IF NOT EXISTS message(
                   rid INTEGER PRIMARY KEY,
                   id BLOB NOT NULL UNIQUE,
                   conv BLOB NOT NULL,
                   sort_ms INTEGER NOT NULL,
                   placeholder INTEGER NOT NULL DEFAULT 0,
                   row BLOB NOT NULL);
                 CREATE INDEX IF NOT EXISTS message_conv_sort ON message(conv, sort_ms);
                 CREATE VIRTUAL TABLE IF NOT EXISTS message_fts USING fts5(
                   body, content='', contentless_delete=1, detail=column,
                   tokenize='unicode61 remove_diacritics 2', prefix='2 3');
                 INSERT INTO message_fts(message_fts, rank) VALUES('secure-delete', 1);
                 CREATE TABLE IF NOT EXISTS pending(
                   target BLOB NOT NULL, id BLOB NOT NULL, conv BLOB NOT NULL,
                   received_ms INTEGER NOT NULL, ev BLOB NOT NULL,
                   PRIMARY KEY(target, id));
                 CREATE TABLE IF NOT EXISTS draft(conv BLOB PRIMARY KEY, text TEXT NOT NULL);
                 COMMIT;",
            )?;
        }
        db.execute_batch(&format!("PRAGMA user_version={SCHEMA_VERSION}"))?;
        Ok(SqliteBackend { db })
    }

    fn rows(&self, sql: &str, p: &[&dyn rusqlite::ToSql]) -> Vec<proto::StoredMessage> {
        let Ok(mut st) = self.db.prepare_cached(sql) else {
            return vec![];
        };
        let Ok(it) = st.query_map(p, |r| r.get::<_, Vec<u8>>(0)) else {
            return vec![];
        };
        it.filter_map(|b| proto::StoredMessage::decode(b.ok()?.as_slice()).ok())
            .collect()
    }
}

/// FTS5 query: every folded token as a quoted prefix term, ANDed.
fn fts_query(q: &str) -> Option<String> {
    let t = tokens(q);
    if t.is_empty() {
        return None;
    }
    Some(
        t.iter()
            .map(|x| format!("\"{}\"*", x.replace('"', "")))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

impl ChatBackend for SqliteBackend {
    fn get(&self, id: &MsgId) -> Option<proto::StoredMessage> {
        let b: Vec<u8> = self
            .db
            .prepare_cached("SELECT row FROM message WHERE id=?1")
            .ok()?
            .query_row(params![&id[..]], |r| r.get(0))
            .optional()
            .ok()??;
        proto::StoredMessage::decode(b.as_slice()).ok()
    }

    fn put(&mut self, conv: &GroupId, row: &proto::StoredMessage, search: Option<&str>) {
        let Some(id) = row.msg.as_ref().and_then(|m| id16(&m.msg_uuid)) else {
            return;
        };
        let Ok(tx) = self.db.transaction() else {
            return;
        };
        let res = (|| -> rusqlite::Result<()> {
            let rid: i64 = tx.query_row(
                "INSERT INTO message(id, conv, sort_ms, placeholder, row) VALUES(?1,?2,?3,?4,?5)
                 ON CONFLICT(id) DO UPDATE SET conv=excluded.conv, sort_ms=excluded.sort_ms,
                   placeholder=excluded.placeholder, row=excluded.row
                 RETURNING rid",
                params![
                    &id[..],
                    &conv[..],
                    sort_ms(row) as i64,
                    row.placeholder as i64,
                    row.encode_to_vec()
                ],
                |r| r.get(0),
            )?;
            tx.execute("DELETE FROM message_fts WHERE rowid=?1", params![rid])?;
            if let Some(s) = search {
                tx.execute(
                    "INSERT INTO message_fts(rowid, body) VALUES(?1, ?2)",
                    params![rid, super::ro_comma(s)],
                )?;
            }
            Ok(())
        })();
        if res.is_ok() {
            let _ = tx.commit();
        }
    }

    fn history(
        &self,
        conv: &GroupId,
        before_ms: Option<Ms>,
        limit: usize,
    ) -> Vec<proto::StoredMessage> {
        let before = before_ms.map(|b| b as i64).unwrap_or(i64::MAX);
        let mut v = self.rows(
            "SELECT row FROM message WHERE conv=?1 AND placeholder=0 AND sort_ms<?2
             ORDER BY sort_ms DESC, id DESC LIMIT ?3",
            &[&&conv[..], &before, &(limit as i64)],
        );
        v.reverse();
        v
    }

    fn search(
        &self,
        conv: Option<&GroupId>,
        query: &str,
        limit: usize,
    ) -> Vec<(GroupId, proto::StoredMessage)> {
        let Some(q) = fts_query(query) else {
            return vec![];
        };
        let sql = "SELECT m.conv, m.row FROM message_fts f JOIN message m ON m.rid=f.rowid
                   WHERE message_fts MATCH ?1 AND (?2 IS NULL OR m.conv=?2)
                   ORDER BY m.sort_ms DESC LIMIT ?3";
        let Ok(mut st) = self.db.prepare_cached(sql) else {
            return vec![];
        };
        let c: Option<Vec<u8>> = conv.map(|c| c.to_vec());
        let Ok(it) = st.query_map(params![q, c, limit as i64], |r| {
            Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, Vec<u8>>(1)?))
        }) else {
            return vec![];
        };
        it.filter_map(|x| {
            let (c, b) = x.ok()?;
            Some((id16(&c)?, proto::StoredMessage::decode(b.as_slice()).ok()?))
        })
        .collect()
    }

    fn wipe_conv(&mut self, conv: &GroupId) {
        let _ = self.db.execute_batch("BEGIN");
        let _ = self.db.execute(
            "DELETE FROM message_fts WHERE rowid IN (SELECT rid FROM message WHERE conv=?1)",
            params![&conv[..]],
        );
        let _ = self
            .db
            .execute("DELETE FROM message WHERE conv=?1", params![&conv[..]]);
        let _ = self
            .db
            .execute("DELETE FROM pending WHERE conv=?1", params![&conv[..]]);
        let _ = self
            .db
            .execute("DELETE FROM draft WHERE conv=?1", params![&conv[..]]);
        let _ = self.db.execute_batch("COMMIT");
    }

    fn add_pending(&mut self, target: &MsgId, ev: &proto::PendingEvent) {
        let Some(m) = ev.msg.as_ref() else {
            return;
        };
        let _ = self.db.execute(
            "INSERT OR IGNORE INTO pending(target, id, conv, received_ms, ev) VALUES(?1,?2,?3,?4,?5)",
            params![
                &target[..],
                &m.msg_uuid[..],
                &m.group_uuid[..],
                ev.received_ms as i64,
                ev.encode_to_vec()
            ],
        );
    }

    fn take_pending(&mut self, target: &MsgId) -> Vec<proto::PendingEvent> {
        let out: Vec<proto::PendingEvent> = {
            let Ok(mut st) = self
                .db
                .prepare_cached("SELECT ev FROM pending WHERE target=?1")
            else {
                return vec![];
            };
            let Ok(it) = st.query_map(params![&target[..]], |r| r.get::<_, Vec<u8>>(0)) else {
                return vec![];
            };
            it.filter_map(|b| proto::PendingEvent::decode(b.ok()?.as_slice()).ok())
                .collect()
        };
        let _ = self
            .db
            .execute("DELETE FROM pending WHERE target=?1", params![&target[..]]);
        out
    }

    fn expire_pending(&mut self, before_ms: Ms) -> Vec<(MsgId, MsgId)> {
        let _ = self.db.execute(
            "DELETE FROM pending WHERE received_ms<?1",
            params![before_ms as i64],
        );
        vec![]
    }

    fn set_draft(&mut self, conv: &GroupId, text: &str) {
        let _ = if text.is_empty() {
            self.db
                .execute("DELETE FROM draft WHERE conv=?1", params![&conv[..]])
        } else {
            self.db.execute(
                "INSERT INTO draft(conv, text) VALUES(?1,?2)
                 ON CONFLICT(conv) DO UPDATE SET text=excluded.text",
                params![&conv[..], text],
            )
        };
    }

    fn draft(&self, conv: &GroupId) -> Option<String> {
        self.db
            .query_row(
                "SELECT text FROM draft WHERE conv=?1",
                params![&conv[..]],
                |r| r.get(0),
            )
            .optional()
            .ok()?
    }
}
