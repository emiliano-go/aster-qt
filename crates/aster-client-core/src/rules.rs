// SPDX-License-Identifier: AGPL-3.0-or-later
//! Client-side mailbox routing rules.
//!
//! Aster Bridge has no server-side Sieve/filter engine, so routing is
//! evaluated locally after sync. This module only stores rules and matches
//! cached message metadata; moving messages happens in the service layer.

use aster_bridge_core::db::{CachedMessage, Database};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuleMatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_attachment: Option<bool>,
    /// false = any condition matches, true = all conditions must match
    #[serde(default)]
    pub all: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuleAction {
    /// Target folder label (`mbx_...` token resolved by the service layer).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub move_to: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub add_tags: Vec<String>,
    #[serde(default)]
    pub mark_read: bool,
    #[serde(default)]
    pub star: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: i64,
    #[serde(rename = "match")]
    pub conditions: RuleMatch,
    pub action: RuleAction,
    #[serde(default)]
    pub stop_processing: bool,
}

impl Rule {
    pub fn matches(&self, message: &CachedMessage) -> bool {
        if !self.enabled {
            return false;
        }
        let mut checks = Vec::new();
        if let Some(needle) = non_empty(&self.conditions.from) {
            checks.push(contains_ci(message.sender.as_deref(), needle));
        }
        if let Some(needle) = non_empty(&self.conditions.to) {
            checks.push(contains_ci(message.recipients.as_deref(), needle));
        }
        if let Some(needle) = non_empty(&self.conditions.subject) {
            checks.push(contains_ci(message.subject.as_deref(), needle));
        }
        if let Some(needle) = non_empty(&self.conditions.body) {
            checks.push(contains_ci(message.body_text.as_deref(), needle));
        }
        if let Some(want) = self.conditions.has_attachment {
            checks.push(has_attachment(message) == want);
        }
        if checks.is_empty() {
            return false;
        }
        if self.conditions.all {
            checks.iter().all(|matched| *matched)
        } else {
            checks.iter().any(|matched| *matched)
        }
    }
}

fn non_empty(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

fn contains_ci(haystack: Option<&str>, needle: &str) -> bool {
    haystack
        .map(|text| text.to_lowercase().contains(&needle.to_lowercase()))
        .unwrap_or(false)
}

fn has_attachment(message: &CachedMessage) -> bool {
    message
        .raw_headers
        .as_deref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .map(|meta| {
            meta.get("attachments")
                .and_then(|v| v.as_array())
                .is_some_and(|parts| !parts.is_empty())
                || meta.get("has_attachments").and_then(|v| v.as_bool()) == Some(true)
        })
        .unwrap_or(false)
}

pub fn ensure_schema(db: &Database) -> Result<(), String> {
    db.with_conn(|conn| {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS client_rule (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                priority INTEGER NOT NULL DEFAULT 100,
                match_json TEXT NOT NULL,
                action_json TEXT NOT NULL,
                stop_processing INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS client_rule_applied (
                rule_id TEXT NOT NULL,
                message_id TEXT NOT NULL,
                applied_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (rule_id, message_id)
            );",
        )
    })
}

pub fn list(db: &Database) -> Result<Vec<Rule>, String> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, name, enabled, priority, match_json, action_json, stop_processing
             FROM client_rule ORDER BY priority, created_at",
        )?;
        let rows = stmt.query_map([], |row| {
            let match_json: String = row.get(4)?;
            let action_json: String = row.get(5)?;
            Ok(Rule {
                id: row.get(0)?,
                name: row.get(1)?,
                enabled: row.get::<_, i64>(2)? != 0,
                priority: row.get(3)?,
                conditions: serde_json::from_str(&match_json).unwrap_or_default(),
                action: serde_json::from_str(&action_json).unwrap_or_default(),
                stop_processing: row.get::<_, i64>(6)? != 0,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
    })
}

pub fn upsert(db: &Database, rule: &Rule) -> Result<(), String> {
    let match_json = serde_json::to_string(&rule.conditions).map_err(|e| e.to_string())?;
    let action_json = serde_json::to_string(&rule.action).map_err(|e| e.to_string())?;
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO client_rule (id, name, enabled, priority, match_json, action_json, stop_processing)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                enabled = excluded.enabled,
                priority = excluded.priority,
                match_json = excluded.match_json,
                action_json = excluded.action_json,
                stop_processing = excluded.stop_processing,
                updated_at = datetime('now')",
            rusqlite::params![
                rule.id,
                rule.name,
                rule.enabled as i64,
                rule.priority,
                match_json,
                action_json,
                rule.stop_processing as i64,
            ],
        )?;
        Ok(())
    })
}

pub fn delete(db: &Database, id: &str) -> Result<(), String> {
    db.with_conn(|conn| {
        conn.execute("DELETE FROM client_rule WHERE id = ?1", [id])?;
        conn.execute("DELETE FROM client_rule_applied WHERE rule_id = ?1", [id])?;
        Ok(())
    })
}

pub fn already_applied(db: &Database, rule_id: &str, message_id: &str) -> Result<bool, String> {
    db.with_conn(|conn| {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM client_rule_applied WHERE rule_id = ?1 AND message_id = ?2",
            rusqlite::params![rule_id, message_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    })
}

pub fn record_applied(db: &Database, rule_id: &str, message_id: &str) -> Result<(), String> {
    db.with_conn(|conn| {
        conn.execute(
            "INSERT OR IGNORE INTO client_rule_applied (rule_id, message_id) VALUES (?1, ?2)",
            rusqlite::params![rule_id, message_id],
        )?;
        Ok(())
    })
}

/// First matching rule in priority order that has not already touched the message.
pub fn first_match(
    db: &Database,
    rules: &[Rule],
    message: &CachedMessage,
) -> Result<Option<Rule>, String> {
    for rule in rules {
        if !rule.matches(message) {
            continue;
        }
        if already_applied(db, &rule.id, &message.aster_id)? {
            continue;
        }
        return Ok(Some(rule.clone()));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aster_bridge_core::db::ATTACHMENTS_NONE;

    fn db() -> (Database, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open_with_key(dir.path(), &[7u8; 32]).unwrap();
        (db, dir)
    }

    fn message(sender: &str, subject: &str, raw: Option<&str>) -> CachedMessage {
        CachedMessage {
            aster_id: "m1".into(),
            folder: "inbox".into(),
            subject: Some(subject.into()),
            sender: Some(sender.into()),
            recipients: Some("me@example.test".into()),
            date: None,
            size: 0,
            flags: 0,
            body_text: Some("body".into()),
            raw_headers: raw.map(str::to_string),
            imap_uid: 1,
            thread_id: None,
            attachments_state: ATTACHMENTS_NONE,
        }
    }

    #[test]
    fn schema_and_round_trip() {
        let (db, _dir) = db();
        ensure_schema(&db).unwrap();
        let rule = Rule {
            id: "r1".into(),
            name: "Invoices".into(),
            enabled: true,
            priority: 10,
            conditions: RuleMatch {
                from: Some("billing@".into()),
                ..Default::default()
            },
            action: RuleAction {
                move_to: Some("mbx_invoices".into()),
                ..Default::default()
            },
            stop_processing: true,
        };
        upsert(&db, &rule).unwrap();
        assert_eq!(list(&db).unwrap(), vec![rule]);
    }

    #[test]
    fn matching_and_applied_ledger() {
        let (db, _dir) = db();
        ensure_schema(&db).unwrap();
        let rule = Rule {
            id: "r1".into(),
            name: "Invoices".into(),
            enabled: true,
            priority: 10,
            conditions: RuleMatch {
                from: Some("billing@".into()),
                all: true,
                ..Default::default()
            },
            action: RuleAction {
                move_to: Some("mbx_invoices".into()),
                ..Default::default()
            },
            stop_processing: true,
        };
        upsert(&db, &rule).unwrap();
        let msg = message("Billing <billing@example.test>", "Invoice", None);
        assert!(rule.matches(&msg));
        assert_eq!(first_match(&db, &list(&db).unwrap(), &msg).unwrap().unwrap().id, "r1");
        record_applied(&db, "r1", "m1").unwrap();
        assert!(first_match(&db, &list(&db).unwrap(), &msg).unwrap().is_none());
    }

    #[test]
    fn attachment_condition() {
        let msg = message("a@b.test", "x", Some(r#"{"attachments":[{"name":"a.pdf"}]}"#));
        let rule = Rule {
            id: "r".into(),
            name: "With attachments".into(),
            enabled: true,
            priority: 1,
            conditions: RuleMatch {
                has_attachment: Some(true),
                ..Default::default()
            },
            action: RuleAction::default(),
            stop_processing: false,
        };
        assert!(rule.matches(&msg));
    }
}
