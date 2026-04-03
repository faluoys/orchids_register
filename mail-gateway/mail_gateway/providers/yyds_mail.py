from __future__ import annotations

import re
import time
from datetime import datetime, timezone
from typing import Any

import httpx

from mail_gateway.providers.base import AcquiredInbox, InboxProvider, PollResult


class YYDSMailProvider(InboxProvider):
    def __init__(self, base_url: str, api_key: str, client: httpx.Client | None = None) -> None:
        self.base_url = base_url.rstrip('/')
        self.api_key = api_key
        self.client = client or httpx.Client(timeout=15.0)

    def acquire_inbox(
        self,
        project_code: str | None,
        domain: str | None,
        metadata: dict[str, str],
    ) -> AcquiredInbox:
        payload: dict[str, str] = {}
        prefix = _as_string(metadata.get('prefix'))
        normalized_domain = _normalize_domain(domain)
        if prefix is not None:
            payload['prefix'] = prefix
        if normalized_domain is not None:
            payload['domain'] = normalized_domain

        data = self._request_json('POST', '/me/inboxes', json=payload)
        inbox_id = _as_string(data.get('id'))
        address = _as_string(data.get('address'))
        if inbox_id is None or address is None:
            raise RuntimeError(f'YYDS acquire failed: {data!r}')

        return AcquiredInbox(
            address=address,
            upstream_token=address,
            upstream_ref=f'inbox:{inbox_id}',
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
            messages_data = self._request_json(
                'GET',
                '/messages',
                params={'address': upstream_token},
            )
            messages = _as_list(messages_data.get('messages'))
            for message in _filter_messages(messages, after_ts_ms):
                message_id = _as_string(message.get('id'))
                if message_id is None:
                    continue
                detail = self._request_json(
                    'GET',
                    f'/messages/{message_id}',
                    params={'address': upstream_token},
                )
                poll_result = _normalize_poll_result(detail, regex, after_ts_ms)
                if poll_result is not None:
                    return poll_result

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

    def _request_json(
        self,
        method: str,
        path: str,
        *,
        params: dict[str, str] | None = None,
        json: dict[str, str] | None = None,
    ) -> dict[str, Any]:
        try:
            response = self.client.request(
                method,
                self._url(path),
                headers={'X-API-Key': self.api_key},
                params=params,
                json=json,
            )
        except httpx.HTTPError as exc:
            raise RuntimeError(f'YYDS request failed: {exc}') from exc

        if response.status_code >= 400:
            raise RuntimeError(f'YYDS request failed: {_response_error_detail(response)}')

        try:
            payload = response.json()
        except ValueError as exc:
            raise RuntimeError(f'YYDS response is not valid JSON: {response.text}') from exc

        if not isinstance(payload, dict):
            raise RuntimeError(f'YYDS response is not a JSON object: {payload!r}')

        if payload.get('success') is False:
            detail = payload.get('error') or payload.get('message') or payload
            raise RuntimeError(f'YYDS request failed: {detail}')

        data = payload.get('data')
        return data if isinstance(data, dict) else payload

    def _url(self, path: str) -> str:
        return f'{self.base_url}/{path.lstrip("/")}'


def _filter_messages(messages: list[dict[str, Any]], after_ts_ms: int | None) -> list[dict[str, Any]]:
    filtered: list[tuple[int, dict[str, Any]]] = []
    for message in messages:
        created_at_ms = _to_timestamp_ms(message.get('createdAt'))
        if after_ts_ms is not None and (created_at_ms is None or created_at_ms < after_ts_ms):
            continue
        sort_key = created_at_ms if created_at_ms is not None else -1
        filtered.append((sort_key, message))
    filtered.sort(key=lambda item: item[0], reverse=True)
    return [message for _, message in filtered]


def _normalize_poll_result(
    detail: dict[str, Any],
    regex: re.Pattern[str],
    after_ts_ms: int | None,
) -> PollResult | None:
    message_id = _as_string(detail.get('id'))
    received_at = _as_string(detail.get('createdAt'))
    received_at_ms = _to_timestamp_ms(received_at)
    if after_ts_ms is not None and (received_at_ms is None or received_at_ms < after_ts_ms):
        return None

    subject = _as_string(detail.get('subject')) or ''
    text_body = _flatten_text(detail.get('text'))
    html_body = _flatten_text(detail.get('html'))
    code = _extract_code(regex, [subject, text_body, html_body])
    if code is None:
        return None

    from_addr = _extract_from_address(detail.get('from')) or ''
    return PollResult(
        status='success',
        code=code,
        message_id=message_id,
        received_at=received_at,
        summary={
            'from': from_addr,
            'subject': subject,
        },
    )


def _response_error_detail(response: httpx.Response) -> str:
    try:
        payload = response.json()
    except ValueError:
        text = response.text.strip()
        return text or f'HTTP {response.status_code}'

    if isinstance(payload, dict):
        detail = payload.get('detail') or payload.get('error') or payload.get('message')
        if detail is not None:
            return str(detail)
    return str(payload)


def _extract_code(regex: re.Pattern[str], parts: list[str]) -> str | None:
    match = regex.search('\n'.join(parts))
    return match.group(1) if match else None


def _extract_from_address(value: Any) -> str | None:
    if isinstance(value, dict):
        return _as_string(value.get('address')) or _as_string(value.get('name'))
    return _as_string(value)


def _flatten_text(value: Any) -> str:
    if isinstance(value, str):
        return value
    if isinstance(value, list):
        return '\n'.join(item for item in value if isinstance(item, str))
    return ''


def _as_list(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, dict)]


def _as_string(value: Any) -> str | None:
    if not isinstance(value, str):
        return None
    text = value.strip()
    return text or None


def _normalize_domain(value: str | None) -> str | None:
    if not isinstance(value, str):
        return None
    text = value.strip().lower()
    if text.startswith('@'):
        text = text[1:]
    return text or None


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