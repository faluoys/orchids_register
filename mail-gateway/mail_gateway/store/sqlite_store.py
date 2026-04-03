import sqlite3
from pathlib import Path

from mail_gateway.schemas.inbox import InboxSessionRecord


class SQLiteStore:
    def __init__(self, db_path: str | Path) -> None:
        self.db_path = str(db_path)
        self._memory_conn: sqlite3.Connection | None = None
        if self.db_path == ':memory:':
            self._memory_conn = sqlite3.connect(':memory:', check_same_thread=False)

    def _connect(self) -> sqlite3.Connection:
        if self._memory_conn is not None:
            return self._memory_conn
        db_path = Path(self.db_path)
        db_path.parent.mkdir(parents=True, exist_ok=True)
        return sqlite3.connect(self.db_path)

    def init_schema(self) -> None:
        with self._connect() as conn:
            conn.execute(
                """
                CREATE TABLE IF NOT EXISTS inbox_sessions (
                    session_id TEXT PRIMARY KEY,
                    provider TEXT NOT NULL,
                    mode TEXT NOT NULL,
                    address TEXT NOT NULL,
                    upstream_token TEXT NOT NULL,
                    upstream_ref TEXT NOT NULL,
                    project_code TEXT,
                    status TEXT NOT NULL,
                    last_message_id TEXT,
                    created_at TEXT NOT NULL,
                    expires_at TEXT
                )
                """
            )

    def save_session(self, record: InboxSessionRecord) -> None:
        with self._connect() as conn:
            conn.execute(
                """
                INSERT OR REPLACE INTO inbox_sessions (
                    session_id, provider, mode, address, upstream_token,
                    upstream_ref, project_code, status, last_message_id,
                    created_at, expires_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    record.session_id,
                    record.provider,
                    record.mode,
                    record.address,
                    record.upstream_token,
                    record.upstream_ref,
                    record.project_code,
                    record.status,
                    record.last_message_id,
                    record.created_at,
                    record.expires_at,
                ),
            )

    def get_session(self, session_id: str) -> InboxSessionRecord | None:
        with self._connect() as conn:
            row = conn.execute(
                'SELECT session_id, provider, mode, address, upstream_token, upstream_ref, project_code, status, last_message_id, created_at, expires_at FROM inbox_sessions WHERE session_id = ?',
                (session_id,),
            ).fetchone()
        if row is None:
            return None
        return InboxSessionRecord(
            session_id=row[0],
            provider=row[1],
            mode=row[2],
            address=row[3],
            upstream_token=row[4],
            upstream_ref=row[5],
            project_code=row[6],
            status=row[7],
            last_message_id=row[8],
            created_at=row[9],
            expires_at=row[10],
        )

    def update_last_message_id(self, session_id: str, message_id: str) -> None:
        with self._connect() as conn:
            conn.execute(
                'UPDATE inbox_sessions SET last_message_id = ? WHERE session_id = ?',
                (message_id, session_id),
            )

    def delete_session(self, session_id: str) -> None:
        with self._connect() as conn:
            conn.execute('DELETE FROM inbox_sessions WHERE session_id = ?', (session_id,))
