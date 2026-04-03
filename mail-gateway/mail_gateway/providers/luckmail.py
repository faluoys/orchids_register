from __future__ import annotations

import re
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from mail_gateway.providers.base import AcquiredInbox, InboxProvider, PollResult


SDK_VENDOR_DIR = Path(__file__).resolve().parents[2] / 'vendor' / 'LuckMailSdk-Python'
DOMAIN_FILTER_LOOKUP_COUNT = 100


def _ensure_sdk_import_path() -> None:
    if not SDK_VENDOR_DIR.exists():
        raise RuntimeError(f'LuckMail SDK not found: {SDK_VENDOR_DIR}')
    sdk_path = str(SDK_VENDOR_DIR)
    if sdk_path not in sys.path:
        sys.path.insert(0, sdk_path)


def create_luckmail_client(base_url: str, api_key: str):
    _ensure_sdk_import_path()
    from luckmail import LuckMailClient

    return LuckMailClient(base_url=base_url, api_key=api_key)


class LuckMailProvider(InboxProvider):
    def __init__(self, base_url: str, api_key: str, client: Any | None = None) -> None:
        self.base_url = base_url.rstrip('/')
        self.api_key = api_key
        self.client = client or create_luckmail_client(self.base_url, self.api_key)

    def acquire_inbox(
        self,
        project_code: str | None,
        domain: str | None,
        metadata: dict[str, str],
    ) -> AcquiredInbox:
        purchase = self._acquire_purchase(domain, metadata)
        email_address = _get_value(purchase, 'email_address')
        token = _get_value(purchase, 'token')
        purchase_id = _get_value(purchase, 'id')

        if not isinstance(email_address, str) or not email_address.strip():
            raise RuntimeError(f'LuckMail acquire failed: {purchase!r}')
        if not isinstance(token, str) or not token.strip():
            raise RuntimeError(f'LuckMail acquire failed: {purchase!r}')
        if purchase_id is None:
            raise RuntimeError(f'LuckMail acquire failed: {purchase!r}')

        return AcquiredInbox(
            address=email_address,
            upstream_token=token,
            upstream_ref=f'purchase:{purchase_id}',
            expires_at=None,
        )

    def _acquire_purchase(self, domain: str | None, metadata: dict[str, str]) -> Any:
        normalized_domain = _normalize_domain(domain)
        if not normalized_domain:
            return self._acquire_first_purchase(metadata)
        return self._acquire_domain_filtered_purchase(normalized_domain, metadata)

    def _acquire_first_purchase(self, metadata: dict[str, str]) -> Any:
        try:
            items = self.client.user.api_get_purchases(
                1,
                tag_name=metadata.get('tag_name'),
                mark_tag_name=metadata.get('mark_tag_name'),
            )
        except Exception as exc:
            raise RuntimeError(f'LuckMail acquire failed: {exc}') from exc

        if not items:
            raise RuntimeError('LuckMail acquire failed: empty purchase list')
        return items[0]

    def _acquire_domain_filtered_purchase(self, domain: str, metadata: dict[str, str]) -> Any:
        try:
            items = self.client.user.api_get_purchases(
                DOMAIN_FILTER_LOOKUP_COUNT,
                tag_name=metadata.get('tag_name'),
                mark_tag_name=None,
            )
        except Exception as exc:
            raise RuntimeError(f'LuckMail acquire failed: {exc}') from exc

        if not items:
            raise RuntimeError('LuckMail acquire failed: empty purchase list')

        matched_purchase = next(
            (
                item
                for item in items
                if _email_matches_domain(_get_value(item, 'email_address'), domain)
            ),
            None,
        )
        if matched_purchase is None:
            raise RuntimeError(f'LuckMail acquire failed: no purchase matches domain {domain}')

        self._mark_purchase_if_needed(matched_purchase, metadata)
        return matched_purchase

    def _mark_purchase_if_needed(self, purchase: Any, metadata: dict[str, str]) -> None:
        mark_tag_name = _as_string(metadata.get('mark_tag_name'))
        if not mark_tag_name:
            return

        purchase_id = _get_value(purchase, 'id')
        if purchase_id is None:
            raise RuntimeError(f'LuckMail acquire failed: {purchase!r}')

        try:
            self.client.user.set_purchase_tag(purchase_id, tag_name=mark_tag_name)
        except Exception as exc:
            raise RuntimeError(f'LuckMail acquire failed: {exc}') from exc

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
            try:
                token_result = self.client.user.get_token_code(upstream_token)
            except Exception as exc:
                raise RuntimeError(f'LuckMail poll failed: {exc}') from exc

            normalized = self._normalize_token_result(upstream_token, token_result, regex, after_ts_ms)
            if normalized is not None:
                return normalized

            time.sleep(max(interval_seconds, 0.5))

        return PollResult(
            status='timeout',
            code=None,
            message_id=None,
            received_at=None,
            summary={},
        )

    def release_inbox(self, upstream_ref: str, upstream_token: str) -> None:
        return None

    def _normalize_token_result(
        self,
        upstream_token: str,
        token_result: Any,
        regex: re.Pattern[str],
        after_ts_ms: int | None,
    ) -> PollResult | None:
        if not bool(_get_value(token_result, 'has_new_mail')):
            return None

        mail = _get_mapping(_get_value(token_result, 'mail'))
        message_id = _as_string(mail.get('message_id'))
        received_at = _as_string(mail.get('received_at'))
        from_addr = _as_string(mail.get('from'))
        subject = _as_string(mail.get('subject'))
        body_text = _as_string(mail.get('body_text'))
        body_html = _as_string(mail.get('body_html'))
        verification_code = _as_string(_get_value(token_result, 'verification_code'))

        needs_detail = bool(message_id) and (verification_code is None or body_text is None)
        if needs_detail:
            detail = self._get_mail_detail(upstream_token, message_id)
            received_at = _coalesce_str(received_at, _as_string(_get_value(detail, 'received_at')))
            from_addr = _coalesce_str(from_addr, _as_string(_get_value(detail, 'from_addr')))
            subject = _coalesce_str(subject, _as_string(_get_value(detail, 'subject')))
            body_text = _coalesce_str(body_text, _as_string(_get_value(detail, 'body_text')))
            body_html = _coalesce_str(body_html, _as_string(_get_value(detail, 'body_html')))
            verification_code = _coalesce_str(verification_code, _as_string(_get_value(detail, 'verification_code')))

        if after_ts_ms is not None:
            received_at_ms = _to_timestamp_ms(received_at)
            if received_at_ms is None or received_at_ms < after_ts_ms:
                return None

        code = verification_code or _extract_code(
            regex,
            {
                'subject': subject or '',
                'body_text': body_text or '',
                'body_html': body_html or '',
            },
        )
        if not code:
            return None

        return PollResult(
            status='success',
            code=code,
            message_id=message_id,
            received_at=received_at,
            summary={
                'from': from_addr or '',
                'subject': subject or '',
            },
        )

    def _get_mail_detail(self, upstream_token: str, message_id: str) -> Any:
        try:
            return self.client.user.get_token_mail_detail(upstream_token, message_id)
        except Exception as exc:
            raise RuntimeError(f'LuckMail poll failed: {exc}') from exc


def _extract_code(regex: re.Pattern[str], mail: dict[str, str]) -> str | None:
    text = '\n'.join(
        [
            mail.get('subject') or '',
            mail.get('body_text') or '',
            mail.get('body_html') or '',
        ]
    )
    match = regex.search(text)
    return match.group(1) if match else None


def _get_value(source: Any, name: str) -> Any:
    if isinstance(source, dict):
        return source.get(name)
    return getattr(source, name, None)


def _get_mapping(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


def _as_string(value: Any) -> str | None:
    if not isinstance(value, str):
        return None
    text = value.strip()
    return text or None


def _coalesce_str(primary: str | None, fallback: str | None) -> str | None:
    return primary if primary is not None else fallback


def _normalize_domain(value: str | None) -> str | None:
    if not isinstance(value, str):
        return None
    text = value.strip().lower()
    if text.startswith('@'):
        text = text[1:]
    return text or None


def _email_matches_domain(email_address: Any, domain: str) -> bool:
    if not isinstance(email_address, str):
        return False
    normalized_email = email_address.strip().lower()
    return normalized_email.endswith(f'@{domain}')


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
            iso_text = text[:-1] + '+00:00' if text.endswith('Z') else text
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
