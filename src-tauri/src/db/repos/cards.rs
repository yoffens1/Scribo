use rusqlite::{Connection, OptionalExtension};
use crate::error::AppError;
use crate::domain::card::{Card, CardId, CardType, NewCard};
use crate::domain::section::SectionId;

fn row_to_card(row: &rusqlite::Row) -> rusqlite::Result<Card> {
    let card_type_str: String = row.get(3)?;
    let status_str: String = row.get(7)?;
    let chunk_id_opt: Option<i64> = row.get(2)?;
    
    Ok(Card {
        id: CardId(row.get(0)?),
        note_id: crate::domain::NoteId(row.get(1)?),
        section_id: chunk_id_opt.map(SectionId),
        card_type: CardType::parse(&card_type_str).unwrap_or(CardType::Heading),
        custom_front: row.get(4)?,
        custom_back: row.get(5)?,
        cloze_mask: row.get(6)?,
        status: crate::domain::card::CardLifecycle::parse(&status_str).unwrap_or(crate::domain::card::CardLifecycle::Fresh),
        last_section_snapshot: row.get(8)?,
        generated_by: row.get(9)?,
        source_raw_hash_at_creation: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

pub fn insert_with_schedule(conn: &Connection, new: NewCard) -> Result<CardId, AppError> {
    let now = crate::db::time::now_seconds();

    conn.execute(
        "INSERT INTO cards (
            note_id, chunk_id, card_type, custom_front, custom_back, cloze_mask,
            status, last_section_snapshot, generated_by, source_raw_hash_at_creation,
            created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, 'fresh', NULL, ?, ?, ?, ?)",
        rusqlite::params![
            new.note_id.0,
            new.section_id.0,
            new.card_type.as_str(),
            new.custom_front,
            new.custom_back,
            new.cloze_mask,
            new.generated_by,
            new.source_raw_hash_at_creation,
            now,
            now,
        ],
    )?;

    let card_id = conn.last_insert_rowid();

    // Auto-create a schedule for this card
    conn.execute(
        "INSERT INTO schedules (target_type, target_id, state)
         VALUES ('card', ?, 'new')",
        rusqlite::params![card_id],
    )?;

    Ok(CardId(card_id))
}

pub fn find_by_id(conn: &Connection, id: CardId) -> Result<Option<Card>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT card_id, note_id, chunk_id, card_type, custom_front, custom_back, cloze_mask,
                status, last_section_snapshot, generated_by, source_raw_hash_at_creation, created_at, updated_at
         FROM cards WHERE card_id = ?"
    )?;
    let card = stmt.query_row([id.0], row_to_card).optional()?;
    Ok(card)
}

pub fn list_by_note(conn: &Connection, note_id: i64) -> Result<Vec<Card>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT card_id, note_id, chunk_id, card_type, custom_front, custom_back, cloze_mask,
                status, last_section_snapshot, generated_by, source_raw_hash_at_creation, created_at, updated_at
         FROM cards
         WHERE note_id = ?
         ORDER BY card_id ASC"
    )?;
    let rows = stmt.query_map([note_id], row_to_card)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn update(
    conn: &Connection,
    card_id: i64,
    custom_front: Option<&str>,
    custom_back: Option<&str>,
    status: crate::domain::card::CardLifecycle,
    last_section_snapshot: Option<&str>,
) -> Result<(), AppError> {
    let now = crate::db::time::now_seconds();
    conn.execute(
        "UPDATE cards 
         SET custom_front = ?, custom_back = ?, status = ?, last_section_snapshot = ?, updated_at = ? 
         WHERE card_id = ?",
        rusqlite::params![
            custom_front,
            custom_back,
            status.as_str(),
            last_section_snapshot,
            now,
            card_id
        ],
    )?;
    Ok(())
}

pub fn mark_stale_for_section(conn: &Connection, section_id: i64) -> Result<i64, AppError> {
    let updated = conn.execute(
        "UPDATE cards SET status = 'stale' WHERE chunk_id = ? AND status != 'suspended'",
        rusqlite::params![section_id],
    )?;
    Ok(updated as i64)
}

pub fn delete_by_id(conn: &Connection, id: i64) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM cards WHERE card_id = ?",
        rusqlite::params![id],
    )?;
    Ok(())
}
