from __future__ import annotations

import re
import time
from datetime import datetime, timezone
from typing import Any

import httpx

from mail_gateway.providers.base import AcquiredInbox, InboxProvider, PollResult


class MailChatGPTUKProvider(InboxProvider):
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
        prefix = _as_string(metadata.get('prefix')) or _as_string(project_code)
        normalized_domain = _normalize_domain(domain)

        if prefix is None and normalized_domain is None:
            data = self._request_json_dict('GET', '/api/generate-email')
        else:
            request_payload: dict[str, str] = {}
            if prefix is not None:
                request_payload['prefix'] = prefix
            if normalized_domain is not None:
                request_payload['domain'] = normalized_domain
            data = self._request_json_dict('POST', '/api/generate-email', json=request_payload)

        email = _as_string(data.get('email'))
        if email is None:
            raise RuntimeError(f'MailChatGPT UK acquire failed: missing data.email in {data!r}')

        return AcquiredInbox(
            address=email,
            upstream_token=email,
            upstream_ref=f'inbox:{email}',
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
            messages = self._request_json_list(
                'GET',
                '/api/emails',
                params={'email': upstream_token},
            )
            for message in _filter_messages(messages):
                message_id = _as_string(message.get('id'))
                if message_id is None:
                    continue
                message_created_at = _as_string(_prefer_created_at(message))
                detail_payload = self._request_json_dict('GET', f'/api/email/{message_id}')
                poll_result = _normalize_poll_result(
                    detail_payload,
                    regex,
                    after_ts_ms,
                    message_created_at,
                )
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
    ) -> Any:
        try:
            response = self.client.request(
                method,
                self._url(path),
                headers={'X-API-Key': self.api_key},
                params=params,
                json=json,
            )
        except httpx.HTTPError as exc:
            raise RuntimeError(f'MailChatGPT UK request failed: {exc}') from exc

        if response.status_code >= 400:
            raise RuntimeError(f'MailChatGPT UK request failed: {_response_error_detail(response)}')

        try:
            payload = response.json()
        except ValueError as exc:
            raise RuntimeError(f'MailChatGPT UK response is not valid JSON: {response.text}') from exc

        return payload.get('data') if isinstance(payload, dict) and 'data' in payload else payload

    def _request_json_dict(
        self,
        method: str,
        path: str,
        *,
        params: dict[str, str] | None = None,
        json: dict[str, str] | None = None,
    ) -> dict[str, Any]:
        payload = self._request_json(method, path, params=params, json=json)
        if not isinstance(payload, dict):
            raise RuntimeError(f'MailChatGPT UK request failed: {path} returned non-dict payload: {payload!r}')
        return payload

    def _request_json_list(
        self,
        method: str,
        path: str,
        *,
        params: dict[str, str] | None = None,
        json: dict[str, str] | None = None,
    ) -> list[dict[str, Any]]:
        payload = self._request_json(method, path, params=params, json=json)
        if not isinstance(payload, list):
            raise RuntimeError(f'MailChatGPT UK request failed: {path} returned non-list payload: {payload!r}')
        return [item for item in payload if isinstance(item, dict)]

    def _url(self, path: str) -> str:
        return f'{self.base_url}/{path.lstrip("/")}'


def _filter_messages(messages: list[dict[str, Any]]) -> list[dict[str, Any]]:
    filtered: list[tuple[int, dict[str, Any]]] = []
    for message in messages:
        created_at_ms = _to_timestamp_ms(_prefer_created_at(message))
        sort_key = created_at_ms if created_at_ms is not None else -1
        filtered.append((sort_key, message))
    filtered.sort(key=lambda item: item[0], reverse=True)
    return [message for _, message in filtered]


def _normalize_poll_result(
    detail: dict[str, Any],
    regex: re.Pattern[str],
    after_ts_ms: int | None,
    message_created_at: str | None,
) -> PollResult | None:
    message_id = _as_string(detail.get('id'))
    received_at = _as_string(_prefer_created_at(detail)) or message_created_at
    received_at_ms = _to_timestamp_ms(received_at)
    if after_ts_ms is not None and (received_at_ms is None or received_at_ms < after_ts_ms):
        return None

    subject = _as_string(detail.get('subject')) or ''
    text_body = _as_string(detail.get('text')) or ''
    html_body = _as_string(detail.get('html')) or ''
    code = _extract_code_preferred(regex, text_body, html_body, subject)
    if code is None:
        return None

    from_addr = _as_string(detail.get('from')) or ''
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


def _extract_code_preferred(
    regex: re.Pattern[str],
    text_body: str,
    html_body: str,
    subject: str,
) -> str | None:
    match = _extract_code_from_text(regex, text_body)
    if match is not None:
        return match

    html_text = _html_to_text(html_body)
    match = _extract_code_from_text(regex, html_text)
    if match is not None:
        return match

    return _extract_code_from_text(regex, subject)


def _extract_code_from_text(regex: re.Pattern[str], text: str) -> str | None:
    match = regex.search(text)
    return match.group(1) if match else None


def _html_to_text(value: str) -> str:
    if not value:
        return ''
    return re.sub(r'<[^>]+>', ' ', value)


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


def _prefer_created_at(message: dict[str, Any]) -> Any:
    if 'createdAt' in message:
        return message.get('createdAt')
    return message.get('created_at')


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
