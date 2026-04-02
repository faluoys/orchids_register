from pathlib import Path

from mail_gateway.schemas.inbox import InboxSessionRecord
from mail_gateway.store.sqlite_store import SQLiteStore


def test_sqlite_store_round_trip(tmp_path: Path) -> None:
    db_path = tmp_path / "mail_gateway.db"
    store = SQLiteStore(db_path)
    store.init_schema()

    record = InboxSessionRecord(
        session_id="ses_test_001",
        provider="luckmail",
        mode="purchased",
        address="user1@outlook.com",
        upstream_token="tok_abc123",
        upstream_ref="purchase:42",
        project_code="orchids",
        status="active",
        last_message_id=None,
        created_at="2026-04-02T10:00:00Z",
        expires_at=None,
    )

    store.save_session(record)
    loaded = store.get_session("ses_test_001")

    assert loaded is not None
    assert loaded.address == "user1@outlook.com"
    assert loaded.upstream_token == "tok_abc123"

    store.update_last_message_id("ses_test_001", "msg_001")
    updated = store.get_session("ses_test_001")
    assert updated is not None
    assert updated.last_message_id == "msg_001"

    store.delete_session("ses_test_001")
    assert store.get_session("ses_test_001") is None


def test_sqlite_store_round_trip_with_memory_db() -> None:
    store = SQLiteStore(":memory:")
    store.init_schema()

    record = InboxSessionRecord(
        session_id="ses_mem_001",
        provider="luckmail",
        mode="purchased",
        address="mem1@outlook.com",
        upstream_token="tok_mem_123",
        upstream_ref="purchase:43",
        project_code="orchids",
        status="active",
        last_message_id=None,
        created_at="2026-04-02T11:00:00Z",
        expires_at=None,
    )

    store.save_session(record)
    loaded = store.get_session("ses_mem_001")

    assert loaded is not None
    assert loaded.address == "mem1@outlook.com"
    assert loaded.upstream_token == "tok_mem_123"


def test_sqlite_store_creates_parent_directory(tmp_path: Path) -> None:
    db_path = tmp_path / "data" / "mail_gateway.db"
    store = SQLiteStore(db_path)

    assert not db_path.parent.exists()
    store.init_schema()

    assert db_path.parent.exists()
