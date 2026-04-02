from __future__ import annotations

import re
import time
from datetime import datetime, timezone

import httpx

from mail_gateway.providers.base import AcquiredInbox, InboxProvider, PollResult


class LuckMailProvider(InboxProvider):
    def __init__(self, base_url: str, api_key: str, client: httpx.Client | None = None) -> None:
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.client = client or httpx.Client(base_url=self.base_url, timeout=30.0)

    def acquire_inbox(self, project_code: str | None, metadata: dict[str, str]) -> AcquiredInbox:
        response = self.client.post(
            "/api/v1/openapi/email/purchases/api-get",
            headers={"X-API-Key": self.api_key},
            json={
                "count": 1,
                "tag_name": metadata.get("tag_name", "orchids-ready"),
                "mark_tag_name": metadata.get("mark_tag_name", "orchids-used"),
            },
        )
        response.raise_for_status()
        payload = response.json()
        if payload.get("code") != 0:
            raise RuntimeError(f"LuckMail acquire failed: {payload}")

        data = payload.get("data")
        if not isinstance(data, list) or not data or not isinstance(data[0], dict):
            raise RuntimeError(f"LuckMail acquire failed: {payload}")

        first = data[0]
        try:
            email_address = first["email_address"]
            token = first["token"]
            purchase_id = first["id"]
        except (KeyError, TypeError) as exc:
            raise RuntimeError(f"LuckMail acquire failed: {payload}") from exc

        if not isinstance(email_address, str) or not email_address.strip():
            raise RuntimeError(f"LuckMail acquire failed: {payload}")
        if not isinstance(token, str) or not token.strip():
            raise RuntimeError(f"LuckMail acquire failed: {payload}")
        if purchase_id is None:
            raise RuntimeError(f"LuckMail acquire failed: {payload}")

        return AcquiredInbox(
            address=email_address,
            upstream_token=token,
            upstream_ref=f"purchase:{purchase_id}",
            expires_at=None,
        )

    def poll_code(
        self,
        upstream_token: str,
        timeout_seconds: int,
        interval_seconds: float,
        code_pattern: str,
        after_ts: int | None,
    ) -> PollResult:
        deadline = time.time() + timeout_seconds
        regex = re.compile(code_pattern)
        after_ts_ms = _to_timestamp_ms(after_ts) if after_ts is not None else None
        while time.time() < deadline:
            response = self.client.get(f"/api/v1/openapi/email/token/{upstream_token}/code")
            response.raise_for_status()
            payload = response.json()
            if payload.get("code") != 0:
                raise RuntimeError(f"LuckMail poll failed: {payload}")
            data = payload.get("data") or {}
            if data.get("has_new_mail"):
                mail = data.get("mail") or {}
                if after_ts_ms is not None:
                    received_at_ms = _to_timestamp_ms(mail.get("received_at"))
                    if received_at_ms is None or received_at_ms < after_ts_ms:
                        time.sleep(max(interval_seconds, 0.5))
                        continue

                code = data.get("verification_code") or _extract_code(regex, mail)
                if code:
                    return PollResult(
                        status="success",
                        code=code,
                        message_id=mail.get("message_id"),
                        received_at=mail.get("received_at"),
                        summary={
                            "from": mail.get("from", ""),
                            "subject": mail.get("subject", ""),
                        },
                    )
            time.sleep(max(interval_seconds, 0.5))
        return PollResult(
            status="timeout",
            code=None,
            message_id=None,
            received_at=None,
            summary={},
        )

    def release_inbox(self, upstream_ref: str, upstream_token: str) -> None:
        return None


def _extract_code(regex: re.Pattern[str], mail: dict[str, str]) -> str | None:
    text = "\n".join(
        [
            mail.get("subject", ""),
            mail.get("body_text", ""),
            mail.get("body_html", ""),
        ]
    )
    match = regex.search(text)
    return match.group(1) if match else None


def _to_timestamp_ms(value: object) -> int | None:
    if value is None:
        return None
    if isinstance(value, (int, float)):
        return _normalize_number_timestamp_ms(float(value))
    if isinstance(value, str):
        text = value.strip()
        if not text:
            return None
        try:
            return _normalize_number_timestamp_ms(float(text))
        except ValueError:
            iso_text = text[:-1] + "+00:00" if text.endswith("Z") else text
            try:
                dt = datetime.fromisoformat(iso_text)
            except ValueError:
                return None
            if dt.tzinfo is None:
                dt = dt.replace(tzinfo=timezone.utc)
            return int(dt.timestamp() * 1000)
    return None


def _normalize_number_timestamp_ms(value: float) -> int:
    return int(value * 1000) if abs(value) < 1_000_000_000_000 else int(value)
