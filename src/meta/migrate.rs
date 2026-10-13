//! 마이그레이션 자동 적용 (Phase 13).
//!
//! PostgreSQL 연결 시 `migrations/*.sql`을 파일명 순서로 순차 적용.
//! 멱등성: `CREATE TABLE IF NOT EXISTS`로 재실행 안전.

use tokio_postgres::Client;

use crate::meta::store::DataStoreError;

/// 마이그레이션 파일 목록 (파일명 오름차순).
pub fn list_migration_files() -> Result<Vec<std::path::PathBuf>, DataStoreError> {
    let dir = std::path::Path::new("migrations");
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| DataStoreError::DbQuery(format!("migrations 디렉토리 읽기 실패: {e}")))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |ext| ext == "sql"))
        .collect();
    files.sort();
    Ok(files)
}

/// PostgreSQL 클라이언트에 마이그레이션 적용.
///
/// `migrations/*.sql`을 파일명 오름차순으로 `batch_execute` 호출.
/// 성공: 적용된 파일명 목록. 실패: 에러.
pub async fn run_migrations(client: &Client) -> Result<Vec<String>, DataStoreError> {
    let files = list_migration_files()?;
    let mut applied = Vec::new();

    for file in &files {
        let sql = std::fs::read_to_string(file)
            .map_err(|e| DataStoreError::DbQuery(format!("마이그레이션 파일 읽기 실패: {file:?}: {e}")))?;
        client
            .batch_execute(&sql)
            .await
            .map_err(|e| DataStoreError::DbQuery(format!("마이그레이션 실패: {:?}: {e}", file.file_name().unwrap_or_default())))?;
        applied.push(file.file_name().unwrap_or_default().to_string_lossy().to_string());
    }

    Ok(applied)
}
