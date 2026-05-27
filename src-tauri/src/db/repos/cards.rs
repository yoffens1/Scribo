use rusqlite::{Connection, OptionalExtension};
use crate::error::AppError;
use crate::domain::card::{Card, CardId, CardType, NewCard};
use crate::domain::section::SectionId;

fn row_to_card(row: &rusqlite::Row) -> rusqlite::Result<Card> {
    let card_type_str: String = row.get(2)?;
    Ok(Card {
        id: CardId(row.get(0)?),
        section_id: SectionId(row.get(1)?),
        card_type: CardType::parse(&card_type_str).unwrap_or(CardType::Heading),
        custom_front: row.get(3)?,
        custom_back: row.get(4)?,
        cloze_mask: row.get(5)?,
        is_stale: row.get::<_, i64>(6)? != 0,
        is_suspended: row.get::<_, i64>(7)? != 0,
        generated_by: row.get(8)?,
        section_hash_at_creation: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

pub fn insert_with_schedule(conn: &Connection, new: NewCard) -> Result<CardId, AppError> {
    let now = crate::db::time::now_seconds();

    conn.execute(
        "INSERT INTO cards (
            section_id, card_type, custom_front, custom_back, cloze_mask,
            is_stale, is_suspended, generated_by, section_hash_at_creation,
            created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, 0, 0, ?, ?, ?, ?)",
        rusqlite::params![
            new.section_id.0,
            new.card_type.as_str(),
            new.custom_front,
            new.custom_back,
            new.cloze_mask,
            new.generated_by,
            new.section_hash_at_creation,
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
        "SELECT card_id, section_id, card_type, custom_front, custom_back, cloze_mask,
                is_stale, is_suspended, generated_by, section_hash_at_creation, created_at, updated_at
         FROM cards WHERE card_id = ?"
    )?;
    let card = stmt.query_row([id.0], row_to_card).optional()?;
    Ok(card)
}

pub fn list_by_note(conn: &Connection, note_id: i64) -> Result<Vec<Card>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT c.card_id, c.section_id, c.card_type, c.custom_front, c.custom_back, c.cloze_mask,
                c.is_stale, c.is_suspended, c.generated_by, c.section_hash_at_creation, c.created_at, c.updated_at
         FROM cards c
         JOIN sections s ON s.section_id = c.section_id
         WHERE s.note_id = ?
         ORDER BY c.card_id ASC"
    )?;
    let rows = stmt.query_map([note_id], row_to_card)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn update(
    conn: &Connection,
    card_id: i64,
    custom_front: Option<&str>,
    custom_back: Option<&str>,
    is_suspended: bool,
    is_stale: bool,
) -> Result<(), AppError> {
    let now = crate::db::time::now_seconds();
    conn.execute(
        "UPDATE cards 
         SET custom_front = ?, custom_back = ?, is_suspended = ?, is_stale = ?, updated_at = ? 
         WHERE card_id = ?",
        rusqlite::params![
            custom_front,
            custom_back,
            is_suspended as i64,
            is_stale as i64,
            now,
            card_id
        ],
    )?;
    Ok(())
}

pub fn mark_stale_for_section(conn: &Connection, section_id: i64) -> Result<i64, AppError> {
    let updated = conn.execute(
        "UPDATE cards SET is_stale = 1 WHERE section_id = ?",
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
