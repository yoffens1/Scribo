use rusqlite::params;
use crate::db::state::DbState;
use crate::error::AppError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationPair {
    pub query: String,
    pub expected_note_title: String,
    pub relevance_weight: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CalibrationNote {
    pub id: String,
    pub title: String,
    pub content: String,
}

/// Adds a query-note target calibration pair to the dataset.
pub fn add_calibration_pair(
    state: &DbState,
    query: &str,
    expected_title: &str,
    relevance_weight: f32,
) -> Result<(), AppError> {
    state.with_conn(|conn| {
        conn.execute(
            "INSERT INTO retrieval_calibration (query, expected_note_title, relevance_weight)
             VALUES (?, ?, ?)
             ON CONFLICT(query, expected_note_title) DO UPDATE SET relevance_weight = excluded.relevance_weight",
            params![query.trim(), expected_title.trim(), relevance_weight],
        )?;
        Ok(())
    })
}

/// Automatically seeds/populates the calibration dataset from the active notes in the database.
/// Generates:
/// 1. Query: "[title]" -> Target: "[title]" (weight 1.0)
/// 2. Query: "что такое [title]" (or "what is [title]") -> Target: "[title]" (weight 0.88)
pub fn seed_calibration_dataset(state: &DbState) -> Result<usize, AppError> {
    state.with_conn(|conn| {
        // Fetch active note titles
        let mut stmt = conn.prepare("SELECT title FROM notes WHERE lifecycle = 'active' AND title != ''")?;
        let note_titles: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<_, _>>()?;

        let mut inserted_count = 0;
        for title in note_titles {
            let lower_title = title.to_lowercase();
            // Seed exact alias
            let res1 = conn.execute(
                "INSERT OR IGNORE INTO retrieval_calibration (query, expected_note_title, relevance_weight) VALUES (?, ?, 1.0)",
                params![&lower_title, &title],
            );
            if let Ok(c) = res1 { inserted_count += c; }

            // Seed question alias using central language detector
            let is_ru = crate::lang::detect_language(&lower_title)
                .as_deref() == Some("ru");
            let question_query = if is_ru {
                format!("что такое {}", lower_title)
            } else {
                format!("what is {}", lower_title)
            };

            let res2 = conn.execute(
                "INSERT OR IGNORE INTO retrieval_calibration (query, expected_note_title, relevance_weight) VALUES (?, ?, 0.88)",
                params![&question_query, &title],
            );
            if let Ok(c) = res2 { inserted_count += c; }
        }

        Ok(inserted_count)
    })
}
