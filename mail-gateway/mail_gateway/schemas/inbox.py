from pydantic import BaseModel


class InboxSessionRecord(BaseModel):
    session_id: str
    provider: str
    mode: str
    address: str
    upstream_token: str
    upstream_ref: str
    project_code: str | None = None
    status: str
    last_message_id: str | None = None
    created_at: str
    expires_at: str | None = None
